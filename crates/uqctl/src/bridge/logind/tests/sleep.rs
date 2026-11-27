//! Sleep: inhibitors, delays and fallbacks.

use super::*;

#[tokio::test]
async fn inhibit_taken_at_start() {
    let r = rig().await;
    assert_eq!(
        r.fake.lock().unwrap().inhibit_args,
        vec![(
            "sleep".to_string(),
            "uQ".to_string(),
            "Lock the screen before sleep".to_string(),
            "delay".to_string()
        )]
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!r.closed(0), "the inhibitor is held");
}

#[tokio::test]
async fn sleep_emits_event_and_ready_releases() {
    let mut r = rig().await;
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    assert!(!r.closed(0), "held until ready");
    r.logind.sleep_lock_ready();
    // Returns as soon as it closes; the long limit only matters on a loaded CI runner,
    // where 500 ms was sometimes too short.
    assert!(r.wait_closed(0, Duration::from_secs(5)).await);
}

#[tokio::test]
async fn sleep_fallback_releases() {
    // logind waits 4 s: the inhibitor is held for 3.5 s, well past the old 2 s.
    let (tx, rx) = mpsc::channel(16);
    let fake = Fake {
        delay_usec: Some(4_000_000),
        ..Fake::default()
    };
    let mut r = rig_with(Logind::new(tx), rx, fake).await;
    let t = Instant::now();
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    tokio::time::sleep(Duration::from_millis(3200)).await;
    assert!(!r.closed(0), "held until logind's limit less the margin");
    assert!(r.wait_closed(0, Duration::from_millis(1500)).await);
    assert!(
        t.elapsed() < Duration::from_millis(4000),
        "released before logind's own limit"
    );
}

#[tokio::test]
async fn sleep_fallback_default_without_delay() {
    let (tx, rx) = mpsc::channel(16);
    let fake = Fake {
        delay_usec: None,
        ..Fake::default()
    };
    let mut r = rig_with(Logind::new(tx), rx, fake).await;
    assert_eq!(
        r.logind.fallback.load(Ordering::Relaxed),
        DEFAULT_FALLBACK_MS
    );
    let t = Instant::now();
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    tokio::time::sleep(Duration::from_millis(4200)).await;
    assert!(!r.closed(0), "held for the 4.5 s default");
    assert!(r.wait_closed(0, Duration::from_millis(1500)).await);
    assert!(t.elapsed() < Duration::from_millis(5000));
}

#[tokio::test]
async fn resume_retakes() {
    let mut r = rig().await;
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    r.logind.sleep_lock_ready();
    // Returns as soon as it closes; the long limit only matters on a loaded CI runner,
    // where 500 ms was sometimes too short.
    assert!(r.wait_closed(0, Duration::from_secs(5)).await);
    r.prepare(false).await;
    assert_eq!(r.next().await, sleeping(false));
    assert_eq!(r.inhibits(), 2, "a fresh inhibitor before the event");
    assert!(!r.closed(1));
    // The old fallback timer does not drop the fresh inhibitor.
    tokio::time::sleep(Duration::from_millis(TEST_FALLBACK_MS + 300)).await;
    assert!(!r.closed(1));
}

#[tokio::test]
async fn stale_fallback_spares_next_sleeps_inhibitor() {
    // An 8 s fallback, so the windows below have seconds of slack on a loaded machine.
    let (tx, rx) = mpsc::channel(16);
    let fake = Fake {
        delay_usec: Some(8_500_000),
        ..Fake::default()
    };
    let mut r = rig_with(Logind::new(tx), rx, fake).await;
    let fallback = Duration::from_millis(fallback_ms(Some(8_500_000)));
    let t1 = Instant::now();
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    r.logind.sleep_lock_ready();
    // Returns as soon as it closes; the long limit only matters on a loaded CI runner,
    // where 500 ms was sometimes too short.
    assert!(r.wait_closed(0, Duration::from_secs(5)).await);
    r.prepare(false).await;
    assert_eq!(r.next().await, sleeping(false));
    assert_eq!(r.inhibits(), 2);
    // Sleep again within sleep 1's fallback window.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let t2 = Instant::now();
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    assert!(t2 - t1 < fallback, "sleep 2 starts inside sleep 1's window");

    // Past sleep 1's deadline (and before sleep 2's), the fresh inhibitor is still held.
    tokio::time::sleep_until((t1 + fallback + Duration::from_millis(200)).into()).await;
    assert!(t2.elapsed() < fallback);
    assert!(
        !r.closed(1),
        "sleep 1's timer must not release sleep 2's inhibitor"
    );
    // Sleep 2's own fallback releases it.
    assert!(r.wait_closed(1, fallback).await);
    assert!(t2.elapsed() >= fallback - Duration::from_millis(50));
}

#[test]
fn streak_logs_once_until_ok() {
    let s = Streak::default();
    assert!(s.fail("one"));
    assert!(!s.fail("two"));
    assert!(s.ok(), "ok ends a streak");
    assert!(!s.ok());
    assert!(s.fail("three"));
}

#[tokio::test]
async fn failed_inhibit_is_retried_while_awake() {
    let (tx, rx) = mpsc::channel(16);
    let logind = Logind::with_retry(tx, Duration::from_millis(200));
    let fake = Fake {
        inhibit_fails: 2,
        ..Fake::default()
    };
    let mut r = rig_with(logind, rx, fake).await;
    assert_eq!(r.attempts(), 3, "two failures, then held");
    // The fake records the inhibitor before the bridge reads the reply: give it a moment.
    let t = Instant::now();
    while r.logind.inhibit_fail.0.load(Ordering::Relaxed) {
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "success ended the streak"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(r.attempts(), 3, "no retry while held");

    // No retry while a sleep is pending, even though no inhibitor is held.
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    r.logind.sleep_lock_ready();
    // Returns as soon as it closes; the long limit only matters on a loaded CI runner,
    // where 500 ms was sometimes too short.
    assert!(r.wait_closed(0, Duration::from_secs(5)).await);
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(r.attempts(), 3, "no retry while sleeping");

    // A failed re-take on resume is retried too.
    r.fake.lock().unwrap().inhibit_fails = 1;
    r.prepare(false).await;
    assert_eq!(r.next().await, sleeping(false));
    r.wait_inhibits(2).await;
    assert_eq!(r.attempts(), 5);
    assert!(!r.closed(1));
}

#[tokio::test]
async fn hint_failures_streak() {
    let r = rig().await;
    r.fake.lock().unwrap().hint_fails = true;
    r.logind.set_locked_hint(false).await;
    r.logind.set_locked_hint(true).await;
    assert!(
        r.logind.hint_fail.0.load(Ordering::Relaxed),
        "in a failure streak"
    );
    r.fake.lock().unwrap().hint_fails = false;
    r.logind.set_locked_hint(false).await;
    assert!(
        !r.logind.hint_fail.0.load(Ordering::Relaxed),
        "success ends it"
    );
    assert_eq!(r.fake.lock().unwrap().hints, vec![false]);
}

#[tokio::test]
async fn ready_without_sleep_is_noop() {
    let r = rig().await;
    r.logind.sleep_lock_ready();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!r.closed(0));
}

#[tokio::test]
async fn double_prepare_does_not_stack() {
    let mut r = rig().await;
    // Resume while the inhibitor is still held: no second inhibitor.
    r.prepare(false).await;
    assert_eq!(r.next().await, sleeping(false));
    assert_eq!(r.inhibits(), 1);

    r.prepare(true).await;
    r.prepare(true).await;
    assert_eq!(r.next().await, sleeping(true));
    assert_eq!(r.next().await, sleeping(true));
    r.logind.sleep_lock_ready();
    // Returns as soon as it closes; the long limit only matters on a loaded CI runner,
    // where 500 ms was sometimes too short.
    assert!(r.wait_closed(0, Duration::from_secs(5)).await);

    r.prepare(false).await;
    r.prepare(false).await;
    assert_eq!(r.next().await, sleeping(false));
    assert_eq!(r.next().await, sleeping(false));
    assert_eq!(r.inhibits(), 2, "one fresh inhibitor, not two");
    assert!(!r.closed(1));
}
