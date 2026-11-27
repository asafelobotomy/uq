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
        None,
    ));
    let mut out = BufReader::new(out_r);

    let hello = next(&mut out).await;
    assert_eq!(
        hello,
        json!({"v":1,"event":"Hello","data":{"v":1,"uqd_up":false,"locked":false,
            "welcome_pending":null,"welcome_deferred":false}})
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
    let task = tokio::spawn(run_with_bus(
        in_r,
        out_w,
        Some(NO_BUS),
        Some(NO_BUS),
        locks,
        None,
    ));
    let mut out = BufReader::new(out_r);
    assert_eq!(
        next(&mut out).await,
        json!({"v":1,"event":"Hello","data":{"v":1,"uqd_up":false,"locked":true,
            "welcome_pending":null,"welcome_deferred":false}})
    );
    let ev = json!({"v":1,"event":"LockState","data":{"locked":true}});
    assert_eq!(next(&mut out).await, ev);

    // A repeat is not a change; a change is sent.
    lock_tx.send(true).unwrap();
    lock_tx.send(false).unwrap();
    let ev = json!({"v":1,"event":"LockState","data":{"locked":false}});
    assert_eq!(next(&mut out).await, ev);
    drop(in_w);
    let _ = task.await.unwrap();

    // Nothing reported yet: Hello says unlocked and no LockState is sent.
    let (lock_tx, locks) = mpsc::unbounded_channel();
    let (mut in_w, in_r) = duplex(4096);
    let (out_w, out_r) = duplex(4096);
    let task = tokio::spawn(run_with_bus(
        in_r,
        out_w,
        Some(NO_BUS),
        Some(NO_BUS),
        locks,
        None,
    ));
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
    let _ = task.await.unwrap();
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
        None,
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
    let _ = task.await.unwrap();
}

#[tokio::test]
async fn idle_hint_answers_without_logind() {
    let (mut in_w, in_r) = duplex(4096);
    let (out_w, out_r) = duplex(4096);
    let task = tokio::spawn(run_with_bus(
        in_r,
        out_w,
        Some(NO_BUS),
        Some(NO_BUS),
        no_locks(),
        None,
    ));
    let mut out = BufReader::new(out_r);
    let _hello = next(&mut out).await;
    in_w.write_all(b"{\"v\":1,\"id\":1,\"call\":\"IdleHint\",\"args\":[true]}\n")
        .await
        .unwrap();
    assert_eq!(next(&mut out).await, json!({"v":1,"id":1,"ok":null}));
    in_w.write_all(b"{\"v\":1,\"id\":2,\"call\":\"IdleHint\",\"args\":[\"yes\"]}\n")
        .await
        .unwrap();
    assert_eq!(
        next(&mut out).await,
        json!({"v":1,"id":2,"err":"IdleHint takes one boolean argument"})
    );
    drop(in_w);
    let _ = task.await.unwrap();
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
        None,
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
    let _ = task.await.unwrap();
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
        None,
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
    let _ = task.await.unwrap();

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
        None,
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(in_w);
    let code = tokio::time::timeout(Duration::from_millis(3500), task)
        .await
        .expect("run returns despite a stalled stdout")
        .unwrap();
    assert_eq!(code, ExitCode::SUCCESS);
}
