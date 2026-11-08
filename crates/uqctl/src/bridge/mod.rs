//! `uqctl bridge`: the shell's long-lived JSON-lines child process (spec §2.3).
//!
//! stdout is protocol only; everything else goes to stderr. One task owns stdout and receives
//! `Outbound` over an mpsc channel, so concurrent replies and events never interleave mid-line.

pub mod protocol;

use protocol::{FrameError, Inbound, MAX_LINE, Outbound, encode, read_lines};
use serde_json::{Value, json};
use std::process::ExitCode;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

/// Handle one request. This is the single dispatch point later tasks extend with new calls;
/// anything not matched is an error reply.
async fn handle(call: &str, _args: &[Value]) -> Result<Value, String> {
    match call {
        "Ping" => Ok(json!("pong")),
        other => Err(format!("unknown call {other}")),
    }
}

/// Owns stdout: writes each message as one line, dropping any over `MAX_LINE`.
async fn write_loop<W: AsyncWrite + Unpin>(mut w: W, mut rx: mpsc::Receiver<Outbound>) {
    while let Some(msg) = rx.recv().await {
        let line = encode(&msg);
        if line.len() > MAX_LINE + 1 {
            eprintln!("uqctl bridge: dropping over-long outbound message");
            continue;
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
    let (out_tx, out_rx) = mpsc::channel::<Outbound>(256);
    let writer = tokio::spawn(write_loop(stdout, out_rx));
    let (in_tx, mut in_rx) = mpsc::channel::<Result<Inbound, FrameError>>(64);
    let reader = tokio::spawn(read_lines(stdin, in_tx));

    let hello = Outbound::Event {
        event: "Hello".into(),
        data: json!({"v": 1, "uqd_up": false, "locked": false}),
    };
    let _ = out_tx.send(hello).await;

    while let Some(item) = in_rx.recv().await {
        match item {
            Ok(Inbound::Request { id, call, args }) => {
                let reply = match handle(&call, &args).await {
                    Ok(ok) => Outbound::Ok { id, ok },
                    Err(err) => Outbound::Err { id, err },
                };
                if out_tx.send(reply).await.is_err() {
                    break;
                }
            }
            Err(e) => eprintln!("uqctl bridge: dropped input line: {e}"),
        }
    }

    // stdin EOF: let queued output drain, then exit.
    drop(out_tx);
    let _ = reader.await;
    let _ = writer.await;
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

    async fn next(r: &mut (impl AsyncBufReadExt + Unpin)) -> Value {
        let mut l = String::new();
        r.read_line(&mut l).await.unwrap();
        serde_json::from_str(&l).unwrap()
    }

    #[tokio::test]
    async fn run_hello_ping_eof() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run(in_r, out_w));
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

        // Junk is logged and skipped; the bridge keeps serving.
        in_w.write_all(b"garbage\n{\"v\":2,\"id\":5,\"call\":\"Ping\"}\n")
            .await
            .unwrap();
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
        let task = tokio::spawn(run(in_r, out_w));
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
}
