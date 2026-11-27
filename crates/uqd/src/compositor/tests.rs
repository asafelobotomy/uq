use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

/// `verify`, retrying while exec fails with ETXTBSY (see `test_util`).
async fn verify_retrying_busy(h: &Hyprland, cfg: &str) -> Result<(), CompositorError> {
    for _ in 0..250 {
        match h.verify(std::path::Path::new(cfg)).await {
            Err(CompositorError::Io(e)) if crate::test_util::is_busy(&e) => {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            other => return other,
        }
    }
    h.verify(std::path::Path::new(cfg)).await
}

#[tokio::test]
async fn hyprland_without_signature_is_not_running() {
    let h = Hyprland::new(Some("/run/user/1000".into()), None);
    assert!(matches!(
        h.eval("return 1").await,
        Err(CompositorError::NotRunning)
    ));
    assert!(matches!(h.reload().await, Err(CompositorError::NotRunning)));
}

#[tokio::test]
async fn hyprland_missing_socket_is_not_running() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hyprland::new(Some(dir.path().into()), Some("gone".into()));
    assert!(matches!(
        h.eval("return 1").await,
        Err(CompositorError::NotRunning)
    ));
}

#[tokio::test]
async fn hyprland_socket_reply_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    let server = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in ["ok", "bad thing"] {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 65536];
            let n = s.read(&mut buf).await.unwrap();
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
            s.write_all(reply.as_bytes()).await.unwrap();
        }
        seen
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    let rule = r#"hl.workspace_rule({ workspace = "2", layout = "scrolling" })"#;
    assert!(h.eval(rule).await.is_ok());
    match h.reload().await {
        Err(CompositorError::Rejected(msg)) => assert_eq!(msg, "bad thing"),
        other => panic!("expected Rejected, got {other:?}"),
    }
    assert_eq!(
        server.await.unwrap(),
        vec![format!("eval {rule}"), "reload".to_string()]
    );
}

#[tokio::test]
async fn oversized_reply_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    tokio::spawn(async move {
        for len in [
            crate::hypr::MAX_REPLY_BYTES,
            crate::hypr::MAX_REPLY_BYTES + 1,
        ] {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 64];
            let _ = s.read(&mut buf).await.unwrap();
            let _ = s.write_all(&vec![b'x'; len]).await;
        }
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    assert_eq!(
        h.devices().await.unwrap().len(),
        crate::hypr::MAX_REPLY_BYTES
    );
    assert!(matches!(
        h.devices().await,
        Err(CompositorError::Rejected(_))
    ));
}

#[tokio::test]
async fn request_times_out_when_hyprland_hangs() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    tokio::spawn(async move {
        let (_held, _) = listener.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into())).with_timeouts(
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(1),
    );
    let started = std::time::Instant::now();
    assert!(matches!(
        h.eval("return 1").await,
        Err(CompositorError::Timeout(_))
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[tokio::test]
async fn verify_times_out_and_kills_hung_binary() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("Hyprland");
    let marker = dir.path().join("finished");
    crate::test_util::write_script(
        &fake,
        &format!("#!/bin/sh\nsleep 3\ntouch {}\n", marker.display()),
    );
    let h = Hyprland::new(None, None).with_binary(&fake).with_timeouts(
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(200),
    );
    // Retry ETXTBSY (script just written, other threads forking) before timing the real run.
    let mut tries = 0;
    let started = loop {
        let started = std::time::Instant::now();
        match h.verify(std::path::Path::new("/x.lua")).await {
            Err(CompositorError::Timeout(_)) => break started,
            Err(CompositorError::Io(e)) if crate::test_util::is_busy(&e) && tries < 250 => {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    };
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    assert!(!marker.exists(), "hung verify process was not killed");
}

#[tokio::test]
async fn config_errors_parses_json_lines() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        let _ = s.read(&mut buf).await.unwrap();
        s.write_all(b"[\n\t\"user.lua:3: oops\",\n\t\"\"\n]\n")
            .await
            .unwrap();
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    assert_eq!(
        h.config_errors().await.unwrap(),
        vec!["user.lua:3: oops".to_string()]
    );
}

#[tokio::test]
async fn get_option_returns_the_value_field() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    let server = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in [
            r#"{"option": "general:gaps_in", "css": "5 5 5 5", "set": true }"#,
            r#"{"option": "master:mfact", "float": 0.550000, "set": false }"#,
            r#"{"option": "decoration:blur:enabled", "bool": false, "set": true }"#,
            "no such option",
        ] {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = s.read(&mut buf).await.unwrap();
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
            s.write_all(reply.as_bytes()).await.unwrap();
        }
        seen
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    assert_eq!(
        h.get_option("general:gaps_in").await.unwrap(),
        serde_json::json!("5 5 5 5")
    );
    assert_eq!(
        h.get_option("master:mfact").await.unwrap(),
        serde_json::json!(0.55)
    );
    assert_eq!(
        h.get_option("decoration:blur:enabled").await.unwrap(),
        serde_json::json!(false)
    );
    assert!(matches!(
        h.get_option("general:nope").await,
        Err(CompositorError::Rejected(m)) if m == "no such option"
    ));
    assert_eq!(server.await.unwrap()[0], "j/getoption general:gaps_in");
}

#[tokio::test]
async fn clients_and_cursorpos_requests() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    let server = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in ["[]", r#"{"x":5,"y":6}"#, "[]"] {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let n = s.read(&mut buf).await.unwrap();
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
            s.write_all(reply.as_bytes()).await.unwrap();
        }
        seen
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    assert_eq!(h.clients().await.unwrap(), "[]");
    assert_eq!(h.cursor_pos().await.unwrap(), r#"{"x":5,"y":6}"#);
    assert_eq!(h.binds().await.unwrap(), "[]");
    assert_eq!(
        server.await.unwrap(),
        vec!["j/clients", "j/cursorpos", "j/binds"]
    );
}

#[tokio::test]
async fn workspaces_request() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let listener = UnixListener::bind(sock_dir.join(".socket.sock")).unwrap();
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        let n = s.read(&mut buf).await.unwrap();
        s.write_all(br#"[{"id":3,"monitor":"eDP-1"}]"#)
            .await
            .unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    });
    let h = Hyprland::new(Some(dir.path().into()), Some("SIG".into()));
    assert_eq!(
        h.workspaces().await.unwrap(),
        r#"[{"id":3,"monitor":"eDP-1"}]"#
    );
    assert_eq!(server.await.unwrap(), "j/workspaces");
    assert_eq!(
        HyprSocket::new(Some(dir.path().into()), Some("SIG".into())).event_path(),
        Some(sock_dir.join(".socket2.sock"))
    );
    assert_eq!(HyprSocket::new(None, None).event_path(), None);
}

#[tokio::test]
async fn verify_reports_missing_binary_as_not_running() {
    let h = Hyprland::new(None, None).with_binary("/nonexistent/Hyprland");
    assert!(matches!(
        h.verify(std::path::Path::new("/tmp/x.lua")).await,
        Err(CompositorError::NotRunning)
    ));
}

#[tokio::test]
async fn verify_uses_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("Hyprland");
    crate::test_util::write_script(
        &fake,
        "#!/bin/sh\necho \"Your config has errors: $3\"\nexit 1\n",
    );
    let h = Hyprland::new(None, None).with_binary(&fake);
    match verify_retrying_busy(&h, "/cfg/hyprland.lua").await {
        Err(CompositorError::Rejected(msg)) => {
            assert!(msg.contains("/cfg/hyprland.lua"), "{msg}")
        }
        other => panic!("expected Rejected, got {other:?}"),
    }
}
