//! `uqctl bridge`: the shell's long-lived JSON-lines child process (spec §2.3).
//!
//! stdout is protocol only; everything else goes to stderr. One task owns stdout and receives
//! `Outbound` over an mpsc channel, so concurrent replies and events never interleave mid-line.

mod calc;
mod dispatch;
mod launch;
mod locknotify;
mod logind;
pub(crate) mod net;
mod notifowner;
mod power;
pub mod protocol;
mod providers;
mod run;
mod uqd;

use protocol::{Inbound, MAX_LINE, Outbound, Rejected, encode, read_lines};
use serde_json::{Value, json};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, mpsc, watch};
use tokio_util::sync::CancellationToken;

/// Requests in flight at once; the reader waits for a slot, so a flood cannot grow tasks forever.
const MAX_IN_FLIGHT: usize = 64;
/// How long the writer may take to drain queued output after stdin EOF.
const DRAIN: Duration = Duration::from_secs(2);

/// Handle one request. This is the single dispatch point later tasks extend with new calls;
/// anything not matched is an error reply.
async fn handle(
    link: &uqd::Link,
    net: &Arc<net::Net>,
    runs: &Mutex<run::RunState>,
    providers: &Arc<providers::Providers>,
    logind: &Arc<logind::Logind>,
    call: &str,
    args: &[Value],
) -> Result<Value, String> {
    match call {
        "Ping" => Ok(json!("pong")),
        "GetTheme" | "SetTheme" | "ListThemes" | "GetLayouts" | "SetLayout" | "GetSchedule"
        | "SetSchedule" | "Reload" | "GetSettings" | "SetSetting" | "GetLaunchCounts"
        | "RecordLaunch" => link.call(call, args).await,
        "Launch" => launch::launch(link, args, runs).await,
        "Search" => providers.search(link, runs, args).await,
        "Activate" => providers.activate(link, runs, args).await,
        "Dispatch" => dispatch::dispatch(args).await,
        "Power" => power::power(args).await,
        "Lock" => logind::lock(args).await,
        "SleepLockReady" if args.is_empty() => {
            logind.sleep_lock_ready();
            Ok(Value::Null)
        }
        "SleepLockReady" => Err("SleepLockReady takes no arguments".into()),
        "GeocodeCity" => match args {
            [Value::String(name)] => net.geocode(name).await,
            _ => Err("GeocodeCity takes one string argument (the city name)".into()),
        },
        "NetworkChanged" => match args {
            [Value::Object(o)] if o.get("vpn_active").is_some_and(Value::is_boolean) => {
                net.network_changed(o["vpn_active"] == true).await;
                Ok(Value::Null)
            }
            _ => Err("NetworkChanged takes {vpn_active: bool}".into()),
        },
        other => Err(format!("unknown call {other}")),
    }
}

/// Run the notification-owner watch on the bridge's bus connection, waiting for the bus to be
/// reachable and starting over if the connection goes away.
async fn watch_notif_owner(
    link: Arc<uqd::Link>,
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
) {
    while !stop.is_cancelled() {
        if let Some(conn) = link.connection().await {
            notifowner::watch(conn, tx.clone(), stop.clone()).await;
        }
        tokio::select! { _ = stop.cancelled() => return, _ = tokio::time::sleep(Duration::from_secs(5)) => {} }
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
    let locks = locknotify::spawn(std::env::var_os("WAYLAND_DISPLAY"));
    run_with_bus(stdin, stdout, None, None, locks).await
}

/// Relay lock-notify states: each change (and the first report) becomes a `LockState` event, then
/// goes to `hints` for logind's `SetLockedHint`. Handing the hint to `Logind::hint_loop` through a
/// latest-value channel means a logind that stops answering never delays a `LockState`. `known`
/// is a state already reported before Hello.
async fn relay_locks(
    mut locks: mpsc::UnboundedReceiver<bool>,
    mut known: Option<bool>,
    hints: watch::Sender<Option<bool>>,
    tx: mpsc::Sender<Outbound>,
) {
    let mut sent = None;
    loop {
        if let Some(locked) = known.take()
            && sent != Some(locked)
        {
            sent = Some(locked);
            let ev = Outbound::Event {
                event: "LockState".into(),
                data: json!({"locked": locked}),
            };
            if tx.send(ev).await.is_err() {
                return;
            }
            hints.send_replace(Some(locked));
        }
        match locks.recv().await {
            Some(locked) => known = Some(locked),
            None => return,
        }
    }
}

/// As `run`, against the session bus at `addr` and the system bus at `system` (None: the real
/// ones), with lock states from `locks`. Tests pass private buses and their own lock source, so
/// they never reach a Wayland display.
pub async fn run_with_bus<R, W>(
    stdin: R,
    stdout: W,
    addr: Option<&str>,
    system: Option<&str>,
    mut locks: mpsc::UnboundedReceiver<bool>,
) -> ExitCode
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
    // The latest lock state lock-notify has reported by now, if any.
    let mut known_lock = None;
    while let Ok(locked) = locks.try_recv() {
        known_lock = Some(locked);
    }
    let hello = Outbound::Event {
        event: "Hello".into(),
        data: json!({"v": 1, "uqd_up": link.is_up(), "locked": known_lock.unwrap_or(false)}),
    };
    let _ = out_tx.send(hello).await;

    // Background tasks hold sender clones, so shutdown is by cancellation, not by sender drop.
    let stop = CancellationToken::new();
    let watcher = tokio::spawn({
        let (link, tx, stop) = (link.clone(), out_tx.clone(), stop.clone());
        async move { link.watch(tx, stop).await }
    });

    let net = net::Net::new(
        net::NetClient::new(),
        out_tx.clone(),
        link.settings_rx(),
        stop.clone(),
        net::SETTLE,
    );
    let weather = tokio::spawn(net.clone().weather_loop(net::WEATHER_EVERY));
    let privacy = tokio::spawn(net.clone().privacy_loop());
    let notif = tokio::spawn(watch_notif_owner(
        link.clone(),
        out_tx.clone(),
        stop.clone(),
    ));
    let logind = logind::Logind::new(out_tx.clone());
    let sleep_watch = tokio::spawn(
        logind
            .clone()
            .connect_and_run(system.map(str::to_owned), stop.clone()),
    );
    let (hint_tx, hint_rx) = watch::channel(None);
    let hint_task = tokio::spawn(logind.clone().hint_loop(hint_rx));
    let lock_relay = tokio::spawn(relay_locks(locks, known_lock, hint_tx, out_tx.clone()));

    let runs = Arc::new(Mutex::new(run::RunState::default()));
    let providers = providers::Providers::new(
        out_tx.clone(),
        stop.clone(),
        uq_core::launcher_providers::default_dirs(),
    );
    let slots = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let mut requests = tokio::task::JoinSet::new();
    while let Some(item) = in_rx.recv().await {
        match item {
            Ok(Inbound::Request { id, call, args }) => {
                let Ok(permit) = slots.clone().acquire_owned().await else {
                    break;
                };
                let (link, net, runs, providers, logind, tx) = (
                    link.clone(),
                    net.clone(),
                    runs.clone(),
                    providers.clone(),
                    logind.clone(),
                    out_tx.clone(),
                );
                requests.spawn(async move {
                    let reply =
                        match handle(&link, &net, &runs, &providers, &logind, &call, &args).await {
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
    weather.abort();
    privacy.abort();
    notif.abort();
    sleep_watch.abort();
    // Dropping the receiver ends the lock-notify thread at its next event; it is never joined.
    lock_relay.abort();
    // It may be stuck in a SetLockedHint call that logind never answers.
    hint_task.abort();
    drop(net);
    drop(providers);
    drop(logind);
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

    /// A lock source that never reports: tests never reach a Wayland display.
    fn no_locks() -> mpsc::UnboundedReceiver<bool> {
        mpsc::unbounded_channel().1
    }

    async fn next(r: &mut (impl AsyncBufReadExt + Unpin)) -> Value {
        let mut l = String::new();
        r.read_line(&mut l).await.unwrap();
        serde_json::from_str(&l).unwrap()
    }

    #[tokio::test]
    async fn run_hello_ping_eof() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(
            in_r,
            out_w,
            Some(NO_BUS),
            Some(NO_BUS),
            no_locks(),
        ));
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
    async fn hello_reports_known_state() {
        // Lock-notify already reported "locked" before Hello: Hello says so, and LockState follows.
        let (lock_tx, locks) = mpsc::unbounded_channel();
        lock_tx.send(true).unwrap();
        let (in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS), Some(NO_BUS), locks));
        let mut out = BufReader::new(out_r);
        assert_eq!(
            next(&mut out).await,
            json!({"v":1,"event":"Hello","data":{"v":1,"uqd_up":false,"locked":true}})
        );
        let ev = json!({"v":1,"event":"LockState","data":{"locked":true}});
        assert_eq!(next(&mut out).await, ev);

        // A repeat is not a change; a change is sent.
        lock_tx.send(true).unwrap();
        lock_tx.send(false).unwrap();
        let ev = json!({"v":1,"event":"LockState","data":{"locked":false}});
        assert_eq!(next(&mut out).await, ev);
        drop(in_w);
        task.await.unwrap();

        // Nothing reported yet: Hello says unlocked and no LockState is sent.
        let (lock_tx, locks) = mpsc::unbounded_channel();
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(in_r, out_w, Some(NO_BUS), Some(NO_BUS), locks));
        let mut out = BufReader::new(out_r);
        assert_eq!(next(&mut out).await["data"]["locked"], false);
        in_w.write_all(b"{\"v\":1,\"id\":1,\"call\":\"Ping\"}\n")
            .await
            .unwrap();
        assert_eq!(next(&mut out).await, json!({"v":1,"id":1,"ok":"pong"}));
        lock_tx.send(true).unwrap();
        let ev = json!({"v":1,"event":"LockState","data":{"locked":true}});
        assert_eq!(next(&mut out).await, ev);
        drop(in_w);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn unknown_call_errs() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(
            in_r,
            out_w,
            Some(NO_BUS),
            Some(NO_BUS),
            no_locks(),
        ));
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
    async fn sleep_lock_ready_answers_without_logind() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(
            in_r,
            out_w,
            Some(NO_BUS),
            Some(NO_BUS),
            no_locks(),
        ));
        let mut out = BufReader::new(out_r);
        let _hello = next(&mut out).await;
        in_w.write_all(b"{\"v\":1,\"id\":1,\"call\":\"SleepLockReady\",\"args\":[]}\n")
            .await
            .unwrap();
        assert_eq!(next(&mut out).await, json!({"v":1,"id":1,"ok":null}));
        in_w.write_all(b"{\"v\":1,\"id\":2,\"call\":\"SleepLockReady\",\"args\":[1]}\n")
            .await
            .unwrap();
        assert_eq!(
            next(&mut out).await,
            json!({"v":1,"id":2,"err":"SleepLockReady takes no arguments"})
        );
        drop(in_w);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn calls_while_uqd_down_and_oversize_reply() {
        let (mut in_w, in_r) = duplex(4096);
        let (out_w, out_r) = duplex(4096);
        let task = tokio::spawn(run_with_bus(
            in_r,
            out_w,
            Some(NO_BUS),
            Some(NO_BUS),
            no_locks(),
        ));
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
        let task = tokio::spawn(run_with_bus(
            in_r,
            out_w,
            Some(NO_BUS),
            Some(NO_BUS),
            no_locks(),
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(in_w);
        let code = tokio::time::timeout(Duration::from_millis(3500), task)
            .await
            .expect("run returns despite a stalled stdout")
            .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
