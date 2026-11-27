//! Locking, hints and finding the session.

use super::*;

#[test]
fn lock_argv_is_fixed() {
    assert_eq!(
        lock_argv(),
        [
            "systemctl",
            "--user",
            "start",
            "--no-block",
            "uq-lock.service"
        ]
    );
}

#[tokio::test]
async fn lock_rejects_args() {
    assert!(lock(&[json!("x")]).await.is_err());
}

#[test]
fn fallback_follows_logind_delay() {
    // logind's default 5 s: released half a second before logind gives up.
    assert_eq!(fallback_ms(Some(5_000_000)), 4500);
    assert_eq!(fallback_ms(Some(30_000_000)), 29_500);
    // Never below 2 s, however short logind's limit.
    assert_eq!(fallback_ms(Some(2_200_000)), MIN_FALLBACK_MS);
    assert_eq!(fallback_ms(Some(0)), MIN_FALLBACK_MS);
    // Unreadable: logind's default less the margin.
    assert_eq!(fallback_ms(None), DEFAULT_FALLBACK_MS);
    assert_eq!(DEFAULT_FALLBACK_MS, 4500);
}

#[tokio::test]
async fn hint_loop_sends_latest_once_session_found() {
    let (tx, rx) = mpsc::channel(16);
    let logind = Logind::new(tx);
    // Two states before the session is known: only the latest is sent once it is found.
    let (hint_tx, hint_rx) = tokio::sync::watch::channel(None);
    hint_tx.send_replace(Some(true));
    hint_tx.send_replace(Some(false));
    let hints = tokio::spawn(logind.clone().hint_loop(hint_rx));
    let r = rig_with(logind, rx, Fake::default()).await;
    r.wait_hints(1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(r.fake.lock().unwrap().hints, vec![false]);
    hint_tx.send_replace(Some(true));
    r.wait_hints(2).await;
    assert_eq!(r.fake.lock().unwrap().hints, vec![false, true]);
    hints.abort();
}

#[tokio::test]
async fn lock_state_not_blocked_by_hung_hint() {
    let (tx, rx) = mpsc::channel(16);
    let logind = Logind::new(tx);
    let fake = Fake {
        hint_hangs: true,
        ..Fake::default()
    };
    let r = rig_with(logind.clone(), rx, fake).await;
    let (hint_tx, hint_rx) = tokio::sync::watch::channel(None);
    let hints = tokio::spawn(logind.hint_loop(hint_rx));
    let (out_tx, mut out_rx) = mpsc::channel(16);
    let (lock_tx, lock_rx) = mpsc::unbounded_channel();
    let relay = tokio::spawn(super::super::super::relay_locks(
        lock_rx, None, hint_tx, out_tx,
    ));
    let mut next = async || match tokio::time::timeout(Duration::from_secs(1), out_rx.recv())
        .await
        .expect("LockState promptly")
        .unwrap()
    {
        Outbound::Event { event, data } => (event, data),
        _ => panic!("expected event"),
    };

    lock_tx.send(true).unwrap();
    assert_eq!(next().await, ("LockState".into(), json!({"locked": true})));
    r.wait_hints(1).await; // SetLockedHint(true) is now stuck in logind
    lock_tx.send(false).unwrap();
    assert_eq!(next().await, ("LockState".into(), json!({"locked": false})));
    assert_eq!(
        r.fake.lock().unwrap().hint_calls,
        1,
        "still stuck on the first"
    );
    relay.abort();
    hints.abort();
}

#[tokio::test]
async fn lock_signal_emits_lock_requested() {
    let mut r = rig().await;
    r.fake_conn
        .emit_signal(None::<()>, SESSION_PATH, SESSION, "Lock", &())
        .await
        .unwrap();
    assert_eq!(r.next().await, ("LockRequested".into(), json!({})));
}

#[tokio::test]
async fn run_uses_display_session_without_pid() {
    let (tx, rx) = mpsc::channel(16);
    let fake = Fake {
        by_pid_fails: true,
        display: Some(("2".into(), DISPLAY_PATH.into())),
        ..Fake::default()
    };
    let mut r = rig_with(Logind::new(tx), rx, fake).await;
    // The bridge is subscribed by the time it takes its inhibitor (rig_with waits for it).
    r.fake_conn
        .emit_signal(None::<()>, DISPLAY_PATH, SESSION, "Lock", &())
        .await
        .unwrap();
    assert_eq!(r.next().await, ("LockRequested".into(), json!({})));
    r.logind.set_locked_hint(true).await;
    assert_eq!(r.fake.lock().unwrap().hints, vec![true]);
    assert!(!r.logind.warned.load(Ordering::Relaxed), "session found");
}

#[tokio::test]
async fn locked_hint_forwarded() {
    let r = rig().await;
    r.logind.set_locked_hint(true).await;
    r.logind.set_locked_hint(false).await;
    assert_eq!(r.fake.lock().unwrap().hints, vec![true, false]);
}

#[tokio::test]
async fn session_by_pid_comes_first() {
    let bus = private_bus();
    let fake = Fake {
        display: Some(("2".into(), DISPLAY_PATH.into())),
        ..Fake::default()
    };
    let (_fake_conn, shared) = serve_fake(&bus, fake).await;
    let c = connect(&bus).await;
    assert_eq!(
        find_session(&c, Some(1000), Some("7".into()))
            .await
            .unwrap()
            .as_str(),
        SESSION_PATH
    );
    let f = shared.lock().unwrap();
    assert!(f.get_user_uids.is_empty());
    assert!(f.get_session_ids.is_empty());
}

#[tokio::test]
async fn session_falls_back_to_user_display() {
    let bus = private_bus();
    let fake = Fake {
        by_pid_fails: true,
        display: Some(("2".into(), DISPLAY_PATH.into())),
        ..Fake::default()
    };
    let (_fake_conn, shared) = serve_fake(&bus, fake).await;
    let c = connect(&bus).await;
    assert_eq!(
        find_session(&c, Some(1000), Some("7".into()))
            .await
            .unwrap()
            .as_str(),
        DISPLAY_PATH,
        "the display session wins over a (maybe stale) XDG_SESSION_ID"
    );
    let f = shared.lock().unwrap();
    assert_eq!(f.get_user_uids, vec![1000]);
    assert!(f.get_session_ids.is_empty());
}

#[tokio::test]
async fn session_falls_back_to_xdg_session_id() {
    let bus = private_bus();
    // No display session ("/"), then no user at all: both fall through to XDG_SESSION_ID.
    for no_user in [false, true] {
        let fake = Fake {
            by_pid_fails: true,
            no_user,
            ..Fake::default()
        };
        let (fake_conn, shared) = serve_fake(&bus, fake).await;
        let c = connect(&bus).await;
        assert_eq!(
            find_session(&c, Some(1000), Some("7".into()))
                .await
                .unwrap()
                .as_str(),
            SESSION_PATH
        );
        assert_eq!(shared.lock().unwrap().get_user_uids, vec![1000]);
        assert_eq!(shared.lock().unwrap().get_session_ids, vec!["7"]);
        assert_eq!(find_session(&c, Some(1000), None).await, None);
        // Without a uid the user step is skipped.
        assert_eq!(
            find_session(&c, None, Some("8".into()))
                .await
                .unwrap()
                .as_str(),
            SESSION_PATH
        );
        assert_eq!(shared.lock().unwrap().get_user_uids, vec![1000, 1000]);
        drop(c);
        fake_conn.release_name(DEST).await.unwrap();
    }
}

#[tokio::test]
async fn no_logind_degrades() {
    // A bus with no logind on it.
    let bus = private_bus();
    let (tx, _rx) = mpsc::channel(16);
    let logind = Logind::new(tx);
    let stop = CancellationToken::new();
    let task = tokio::spawn(logind.clone().run(connect(&bus).await, stop.clone()));
    tokio::time::sleep(Duration::from_millis(200)).await;
    logind.sleep_lock_ready();
    tokio::time::timeout(Duration::from_secs(2), logind.set_locked_hint(true))
        .await
        .expect("set_locked_hint answers");
    assert!(logind.warned.load(Ordering::Relaxed));
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("watch stops on cancel")
        .unwrap();

    // No system bus at all: the watch gives up, and the calls still answer.
    let (tx, _rx) = mpsc::channel(16);
    let logind = Logind::new(tx);
    tokio::time::timeout(
        Duration::from_secs(2),
        logind
            .clone()
            .connect_and_run(Some("unix:path=/tmp/uq-no-such-bus".into()), stop.clone()),
    )
    .await
    .expect("connect fails fast");
    assert!(logind.warned.load(Ordering::Relaxed));
    logind.sleep_lock_ready();
    logind.set_locked_hint(false).await;
}
