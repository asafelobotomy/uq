use super::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

fn fast() -> Timing {
    Timing {
        debounce: Duration::from_millis(150),
        backoff_start: Duration::from_millis(20),
        backoff_max: Duration::from_millis(80),
    }
}

fn socket_in(dir: &std::path::Path) -> PathBuf {
    let d = dir.join("hypr/SIG");
    std::fs::create_dir_all(&d).unwrap();
    d.join(".socket2.sock")
}

fn start(path: PathBuf, timing: Timing) -> mpsc::Receiver<Burst> {
    let (tx, rx) = mpsc::channel(8);
    tokio::spawn(async move {
        listen_with(path, tx, timing, &AtomicUsize::new(0)).await;
    });
    rx
}

#[test]
fn parses_only_monitor_and_reload_events() {
    assert_eq!(parse_event("monitoraddedv2>>1,DP-1,Dell"), None);
    assert_eq!(
        parse_event("configreloaded>>"),
        Some(OutputEvent::ConfigReloaded)
    );
    assert_eq!(
        parse_event("configreloaded>>\n"),
        Some(OutputEvent::ConfigReloaded)
    );
    assert_eq!(parse_event("configreloaded>>x"), None);
    assert_eq!(parse_event("workspace>>2"), None);
    assert_eq!(parse_event(r#"monitoradded>>DP-1"x"#), None);
    assert_eq!(parse_event("monitoradded>>"), None);
    assert_eq!(
        parse_event("monitoradded>>DP-1"),
        Some(OutputEvent::Added("DP-1".into()))
    );
    assert_eq!(
        parse_event("monitorremoved>>HDMI-A-1\n"),
        Some(OutputEvent::Removed("HDMI-A-1".into()))
    );
    let long = format!("monitoradded>>{}", "a".repeat(5000));
    assert_eq!(parse_event(&long), None);
}

#[tokio::test]
async fn burst_is_debounced() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let listener = UnixListener::bind(&path).unwrap();
    let mut rx = start(path, fast());
    let (mut s, _) = listener.accept().await.unwrap();
    let started = std::time::Instant::now();
    for _ in 0..5 {
        s.write_all(b"monitoradded>>DP-1\nmonitoraddedv2>>1,DP-1,x\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let sent_all = started.elapsed();
    let burst = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        burst,
        Burst {
            outputs: true,
            added: true,
            config: false
        }
    );
    // It fired a quiet debounce after the last event, not after the first.
    assert!(started.elapsed() >= sent_all + Duration::from_millis(50));
    assert!(
        tokio::time::timeout(Duration::from_millis(600), rx.recv())
            .await
            .is_err(),
        "five events must make one burst"
    );
}

#[tokio::test]
async fn a_removal_alone_is_not_an_add() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let listener = UnixListener::bind(&path).unwrap();
    let mut rx = start(path, fast());
    let (mut s, _) = listener.accept().await.unwrap();
    s.write_all(b"monitorremoved>>DP-2\n").await.unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        Burst {
            outputs: true,
            added: false,
            config: false
        }
    );
}

#[tokio::test]
async fn oversized_and_split_lines_are_handled() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let listener = UnixListener::bind(&path).unwrap();
    let mut rx = start(path, fast());
    let (mut s, _) = listener.accept().await.unwrap();
    // A huge line with no event, then an event split across two writes.
    s.write_all(&vec![b'x'; 20_000]).await.unwrap();
    s.write_all(b"\nmonitorrem").await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    s.write_all(b"oved>>DP-3\n").await.unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        Burst {
            outputs: true,
            added: false,
            config: false
        }
    );
}

#[tokio::test]
async fn reconnects_after_hyprland_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let listener = UnixListener::bind(&path).unwrap();
    let mut rx = start(path.clone(), fast());
    let (mut s, _) = listener.accept().await.unwrap();
    s.write_all(b"monitoradded>>DP-1\n").await.unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(b.added);
    // Hyprland goes away.
    drop(s);
    drop(listener);
    std::fs::remove_file(&path).unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    // And comes back.
    let listener = UnixListener::bind(&path).unwrap();
    let (mut s, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    s.write_all(b"monitorremoved>>DP-1\n").await.unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        Burst {
            outputs: true,
            added: false,
            config: false
        }
    );
}

#[tokio::test(start_paused = true)]
async fn missing_socket_is_quiet() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let attempts = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = mpsc::channel(1);
    let counter = attempts.clone();
    let task = tokio::spawn(async move {
        listen_with(path, tx, Timing::default(), &counter).await;
    });
    tokio::time::sleep(Duration::from_secs(10)).await;
    // Attempts at 0 s, 1 s, 3 s and 7 s: one per backoff step, doubling.
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
    assert!(!task.is_finished());
    assert!(rx.try_recv().is_err());
    // The delay stops growing at the cap.
    tokio::time::sleep(Duration::from_secs(300)).await;
    assert!(attempts.load(Ordering::SeqCst) <= 4 + 12, "no busy loop");
    task.abort();
}

#[tokio::test]
async fn a_config_reload_is_its_own_kind_of_burst() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket_in(dir.path());
    let listener = UnixListener::bind(&path).unwrap();
    let mut rx = start(path, fast());
    let (mut s, _) = listener.accept().await.unwrap();
    s.write_all(b"configreloaded>>\n").await.unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        Burst {
            outputs: false,
            added: false,
            config: true
        }
    );
    // A reload with a screen change in the same burst carries both.
    s.write_all(b"monitoradded>>DP-1\nconfigreloaded>>\n")
        .await
        .unwrap();
    let b = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        b,
        Burst {
            outputs: true,
            added: true,
            config: true
        }
    );
}
