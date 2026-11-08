//! `uqctl bridge`: the shell's long-lived JSON-lines child process (spec §2.3).
//!
//! stdout is protocol only; everything else goes to stderr. One task owns stdout and receives
//! `Outbound` over an mpsc channel, so concurrent replies and events never interleave mid-line.

mod dispatch;
mod launch;
pub mod protocol;
mod uqd;

use protocol::{Inbound, MAX_LINE, Outbound, Rejected, encode, read_lines};
use serde_json::{Value, json};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

/// Requests in flight at once; the reader waits for a slot, so a flood cannot grow tasks forever.
const MAX_IN_FLIGHT: usize = 64;
/// How long the writer may take to drain queued output after stdin EOF.
const DRAIN: Duration = Duration::from_secs(2);

/// Handle one request. This is the single dispatch point later tasks extend with new calls;
/// anything not matched is an error reply.
async fn handle(link: &uqd::Link, call: &str, args: &[Value]) -> Result<Value, String> {
    match call {
        "Ping" => Ok(json!("pong")),
        "GetTheme" | "SetTheme" | "ListThemes" | "GetLayouts" | "SetLayout" | "SetSchedule"
        | "Reload" | "GetSettings" | "SetSetting" | "GetLaunchCounts" | "RecordLaunch" => {
            link.call(call, args).await
        }
        "Launch" => launch::launch(link, args).await,
        "Dispatch" => dispatch::dispatch(args).await,
        other => Err(format!("unknown call {other}")),
    }
}

/// Owns stdout: writes each message as one line. A reply too large for one line is replaced by
/// an error for the same id; an over-long event can only be dropped.
async fn write_loop<W: AsyncWrite + Unpin>(mut w: W, mut rx: mpsc::Receiver<Outbound>) {
    while let Some(msg) = rx.recv().await {
        let mut line = encode(&msg);
        if line.len() > MAX_LINE + 1 {
            match &msg {
                Outbound::Ok { id, .. } | Outbound::Err { id, .. } => {
                    line = encode(&Outbound::Err {
                        id: *id,
                        err: "response too large".into(),
                    });
                }
                Outbound::Event { event, .. } => {
                    eprintln!("uqctl bridge: dropping over-long {event} event");
                    continue;
                }
            }
        }
        if w.write_all(&line).await.is_err() || w.flush().await.is_err() {
            eprintln!("uqctl bridge: stdout closed");
            break;
        }
    }
}

pub async fn run<R, W>(stdin: R, stdout: W) -> ExitCode
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    run_with_bus(stdin, stdout, None).await
}

/// As `run`, against the bus at `addr` (None: the session bus). Tests pass a private bus.
pub async fn run_with_bus<R, W>(stdin: R, stdout: W, addr: Option<&str>) -> ExitCode
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, out_rx) = mpsc::channel::<Outbound>(256);
    let writer = tokio::spawn(write_loop(stdout, out_rx));
    let (in_tx, mut in_rx) = mpsc::channel(64);
    let reader = tokio::spawn(read_lines(stdin, in_tx));

    // Learn uqd's state first so Hello (always the first line out) is truthful.
    let link = Arc::new(uqd::Link::open(addr).await);
    let hello = Outbound::Event {
        event: "Hello".into(),
        data: json!({"v": 1, "uqd_up": link.is_up(), "locked": false}),
    };
    let _ = out_tx.send(hello).await;

    // Background tasks hold sender clones, so shutdown is by cancellation, not by sender drop.
    let stop = CancellationToken::new();
    let watcher = tokio::spawn({
        let (link, tx, stop) = (link.clone(), out_tx.clone(), stop.clone());
        async move { link.watch(tx, stop).await }
    });

    let slots = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let mut requests = tokio::task::JoinSet::new();
    while let Some(item) = in_rx.recv().await {
        match item {
            Ok(Inbound::Request { id, call, args }) => {
                let Ok(permit) = slots.clone().acquire_owned().await else {
                    break;
                };
                let (link, tx) = (link.clone(), out_tx.clone());
                requests.spawn(async move {
                    let reply = match handle(&link, &call, &args).await {
                        Ok(ok) => Outbound::Ok { id, ok },
                        Err(err) => Outbound::Err { id, err },
                    };
                    let _ = tx.send(reply).await;
                    drop(permit);
                });
            }
            Err(Rejected {
                id: Some(id),
                error,
            }) => {
                let _ = out_tx
                    .send(Outbound::Err {
                        id,
                        err: error.to_string(),
                    })
                    .await;
            }
            Err(Rejected { id: None, error }) => {
                eprintln!("uqctl bridge: dropped input line: {error}");
            }
        }
        // Reap finished request tasks so the set stays small.
        while requests.try_join_next().is_some() {}
    }

    // stdin EOF: stop background work, then let the writer drain what is queued (bounded).
    stop.cancel();
    requests.abort_all();
    // Abort rather than await: the watcher may be blocked on a full channel or the bus.
    watcher.abort();
    reader.abort();
    drop(out_tx);
    if tokio::time::timeout(DRAIN, writer).await.is_err() {
        eprintln!("uqctl bridge: output did not drain in time");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

    /// Nothing listens here, so the bridge starts with uqd down and never touches a real bus.
    const NO_BUS: &str = "unix:path=/tmp/uq-no-such-bus";

    async fn next(r: &mut (impl AsyncBufReadExt + Unpin)) -> Value {
        let mut l = String::new();
        r.read_line(&mut l).await.unwrap();
        serde_json::from_str(&l).unwrap()
    }

    #[tokio::test]
    async fn run_hello_ping_eof() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS)));
        let mut out = BufReader::new(out_r);

        let hello = next(&mut out).await;
        assert_eq!(
            hello,
            json!({"v":1,"event":"Hello","data":{"v":1,"uqd_up":false,"locked":false}})
        );
        in_w.write_all(b"{\"v\":1,\"id\":1,\"call\":\"Ping\",\"args\":[]}\n")
            .await
            .unwrap();
        assert_eq!(next(&mut out).await, json!({"v":1,"id":1,"ok":"pong"}));

        // Id-less junk is logged and skipped; a bad request with an id gets an error reply.
        in_w.write_all(b"garbage\n{\"v\":2,\"id\":5,\"call\":\"Ping\"}\n")
            .await
            .unwrap();
        assert_eq!(
            next(&mut out).await,
            json!({"v":1,"id":5,"err":"unsupported protocol version"})
        );
        in_w.write_all(b"{\"v\":1,\"id\":2,\"call\":\"Ping\"}\n")
            .await
            .unwrap();
        assert_eq!(next(&mut out).await, json!({"v":1,"id":2,"ok":"pong"}));

        drop(in_w);
        let code = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .expect("run returns on stdin EOF")
            .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[tokio::test]
    async fn unknown_call_errs() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS)));
        let mut out = BufReader::new(out_r);
        let _hello = next(&mut out).await;
        in_w.write_all(b"{\"v\":1,\"id\":4,\"call\":\"Nope\",\"args\":[]}\n")
            .await
            .unwrap();
        assert_eq!(
            next(&mut out).await,
            json!({"v":1,"id":4,"err":"unknown call Nope"})
        );
        drop(in_w);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn calls_while_uqd_down_and_oversize_reply() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS)));
        let mut out = BufReader::new(out_r);
        let _hello = next(&mut out).await;
        in_w.write_all(b"{\"v\":1,\"id\":1,\"call\":\"GetTheme\"}\n")
            .await
            .unwrap();
        assert_eq!(
            next(&mut out).await,
            json!({"v":1,"id":1,"err":"uqd is not running"})
        );
        drop(in_w);
        task.await.unwrap();

        // A reply over MAX_LINE becomes an error for the same id.
        let (w, r) = duplex(1 << 20);
        let (tx, rx) = mpsc::channel(4);
        let writer = tokio::spawn(write_loop(w, rx));
        tx.send(Outbound::Ok {
            id: 3,
            ok: json!("x".repeat(MAX_LINE)),
        })
        .await
        .unwrap();
        drop(tx);
        writer.await.unwrap();
        let mut r = BufReader::new(r);
        assert_eq!(
            next(&mut r).await,
            json!({"v":1,"id":3,"err":"response too large"})
        );
    }

    #[tokio::test]
    async fn eof_returns_within_budget_when_stdout_is_stalled() {
        let (in_w, in_r) = duplex(4096);
        // Tiny pipe that nobody reads: the writer blocks on Hello.
        let (out_w, _out_r) = duplex(8);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS)));
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(in_w);
        let code = tokio::time::timeout(Duration::from_millis(3500), task)
            .await
            .expect("run returns despite a stalled stdout")
            .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
