//! Pause/resume and the window picker.

use super::*;

pub(crate) async fn pick_server() -> (
    uq_core::test_bus::Bus,
    zbus::Connection,
    Arc<FakeCompositor>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    *comp.clients_json.lock().unwrap() = r#"[{"address":"0x1","pid":7,"mapped":true,
        "hidden":false,"at":[0,0],"size":[100,100],"class":"firefox","title":"Docs",
        "initialClass":"firefox","initialTitle":"Docs","workspace":{"id":1,"name":"1"},
        "floating":false,"xwayland":false,"pinned":false,"fullscreen":0,"tags":[],
        "focusHistoryID":0}]"#
        .into();
    *comp.monitors_json.lock().unwrap() = r#"[{"id":0,"disabled":false,
        "activeWorkspace":{"id":1,"name":"1"},"specialWorkspace":{"id":0,"name":""}}]"#
        .into();
    *comp.cursor_json.lock().unwrap() = r#"{"x":10,"y":10}"#.into();
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    let server = serve(
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
    )
    .await
    .unwrap();
    std::mem::forget(server);
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    (bus, client, comp, dir)
}

#[tokio::test]
async fn pause_and_resume_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    let _server = serve(
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(&client, NAME, PATH, IFACE).await.unwrap();
    let evals = || -> Vec<String> {
        comp.calls()
            .into_iter()
            .filter(|c| c.starts_with("eval"))
            .collect()
    };

    let token: String = proxy.call("PauseShortcuts", &()).await.unwrap();
    assert_eq!(token.len(), 32);
    // A forged token is refused, and the pause stays.
    let err = proxy
        .call::<_, _, ()>("ResumeShortcuts", &("nope",))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidArgument"), "{err}");
    assert_eq!(evals().len(), 1);
    // The desktop isn't locked meanwhile.
    let () = proxy
        .call("SetSection", &("look", r#"{"gaps_in":7}"#, "settings"))
        .await
        .unwrap();
    let () = proxy
        .call("ResumeShortcuts", &(token.as_str(),))
        .await
        .unwrap();
    let e = evals();
    assert!(
        e[0].contains("uq_capture") && e.last().unwrap().contains("\"reset\""),
        "{e:?}"
    );

    // The typed proxy has both calls too; Hyprland down is a plain error.
    let typed = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    let t = typed.pause_shortcuts().await.unwrap();
    typed.resume_shortcuts(&t).await.unwrap();
    *comp.running.lock().unwrap() = false;
    let err = typed.pause_shortcuts().await.unwrap_err();
    assert!(
        err.to_string().contains("Hyprland isn't answering"),
        "{err}"
    );
}

#[tokio::test]
async fn pick_over_dbus() {
    let (bus, client, _comp, _dir) = pick_server().await;
    let proxy = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    let mut requested = proxy.receive_pick_requested().await.unwrap();
    let shell = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let shell_proxy = uq_core::client::DesktopProxy::new(&shell).await.unwrap();
    let responder = tokio::spawn(async move {
        let sig = requested.next().await.unwrap();
        let token = sig.args().unwrap().token.to_string();
        assert_eq!(token.len(), 32);
        shell_proxy.pick_done(&token, "shown").await.unwrap();
        shell_proxy.pick_done(&token, "click").await.unwrap();
    });
    let reply = tokio::time::timeout(std::time::Duration::from_secs(5), proxy.pick_window())
        .await
        .unwrap()
        .unwrap();
    responder.await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["status"], "picked");
    assert_eq!(v["window"]["class"], "firefox");
    assert!(v["window"].get("address").is_none() && v["window"].get("pid").is_none());
    let err = proxy.pick_done("x", "bogus").await.unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.InvalidArgument"),
        "{err:?}"
    );
}

#[tokio::test]
async fn pick_does_not_hold_the_desktop_lock() {
    let (_bus, client, _comp, _dir) = pick_server().await;
    let proxy = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    let mut requested = proxy.receive_pick_requested().await.unwrap();
    let other = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    let pending = tokio::spawn(async move { proxy.pick_window().await });
    let sig = tokio::time::timeout(std::time::Duration::from_secs(5), requested.next())
        .await
        .unwrap()
        .unwrap();
    let token = sig.args().unwrap().token.to_string();
    // The pick is waiting now; a write and a second pick must both get an answer.
    let look = other.get_section("look").await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        other.set_section("look", &look, "test"),
    )
    .await
    .expect("SetSection blocked while a pick waited")
    .unwrap();
    let err = other.pick_window().await.unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.Conflict"),
        "{err:?}"
    );
    other.pick_done(&token, "cancel").await.unwrap();
    let reply = pending.await.unwrap().unwrap();
    assert!(reply.contains("cancelled"), "{reply}");
}
