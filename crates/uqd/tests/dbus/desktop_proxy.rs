//! The typed client proxy matches the interface.

use super::*;

/// The typed client every caller uses matches the server's signatures, method by method.
#[tokio::test]
async fn desktop_proxy_matches_the_interface() {
    let s = served().await;
    let proxy = uq_core::client::DesktopProxy::new(s.proxy.connection())
        .await
        .unwrap();
    let mut history_changed = proxy.receive_history_changed().await.unwrap();
    proxy.set_theme("dracula", "light", "uqctl").await.unwrap();
    proxy
        .set_schedule(r#"{"mode":"off"}"#, "uqctl")
        .await
        .unwrap();
    proxy
        .set_section("look", r#"{"gaps_in":7}"#, "uqctl")
        .await
        .unwrap();
    proxy.set_setting("idle.dim", "200", "uqctl").await.unwrap();
    proxy.set_layout("2", "scrolling", "uqctl").await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), history_changed.next())
        .await
        .unwrap()
        .unwrap();

    let history: Vec<serde_json::Value> =
        serde_json::from_str(&proxy.list_history(500).await.unwrap()).unwrap();
    assert_eq!(history.len(), 4, "{history:?}");
    assert!(
        history.iter().all(|e| e["origin"] == "uqctl"),
        "{history:?}"
    );
    let id = history[0]["id"].as_str().unwrap();
    assert!(!proxy.undo(id, false).await.unwrap().is_empty());

    let point = proxy.save_restore_point("p").await.unwrap();
    let points: Vec<serde_json::Value> =
        serde_json::from_str(&proxy.list_restore_points().await.unwrap()).unwrap();
    assert!(points.iter().any(|p| p["id"] == point.as_str()));
    proxy.restore(&point).await.unwrap();
    proxy.delete_restore_point(&point).await.unwrap();

    let explained: serde_json::Value =
        serde_json::from_str(&proxy.explain("look").await.unwrap()).unwrap();
    assert_eq!(explained["gaps_in"]["stored"], 7, "{explained}");
    proxy.reapply("look").await.unwrap();
    assert!(proxy.explain("bogus").await.is_err());
    assert_eq!(proxy.list_windows().await.unwrap(), "[]");
    let binds: serde_json::Value =
        serde_json::from_str(&proxy.list_binds().await.unwrap()).unwrap();
    assert_eq!(binds["binds"].as_array().unwrap().len(), 68);
    assert!(
        serde_json::from_str::<serde_json::Value>(&proxy.explain("keybinds").await.unwrap())
            .unwrap()
            .is_object()
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&proxy.explain("rules").await.unwrap())
            .unwrap()
            .is_object()
    );

    let mut changed = proxy.receive_profile_changed().await.unwrap();
    assert_eq!(proxy.set_profile(GAMING, false).await.unwrap(), "gaming");
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&proxy.list_profiles().await.unwrap()).unwrap();
    assert_eq!(list[0]["id"], "gaming");
    assert!(
        proxy
            .get_profile("gaming")
            .await
            .unwrap()
            .contains("[look]")
    );
    let report: serde_json::Value =
        serde_json::from_str(&proxy.apply_profile("gaming").await.unwrap()).unwrap();
    assert_eq!(report["active"], "gaming");
    let report: serde_json::Value =
        serde_json::from_str(&proxy.profile_off().await.unwrap()).unwrap();
    assert_eq!(report["active"], "");
    proxy.delete_profile("gaming").await.unwrap();
    let signal = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal.args().unwrap().id, "");

    // `serve()`'s runner never starts anything, but counts `ShellReady` (F1, F39).
    let _autostart_changed = proxy.receive_autostart_changed().await.unwrap();
    proxy.shell_ready().await.unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&proxy.list_autostart().await.unwrap()).unwrap();
    assert!(rows.is_empty(), "{rows:?}");
    let iface = s
        ._server
        .0
        .object_server()
        .interface::<_, uqd::dbus::DesktopIface>(PATH)
        .await
        .unwrap();
    let runner = iface.get().await.runner().clone();
    let r = runner.lock().unwrap();
    assert!(!r.enabled());
    assert_eq!(r.shell_ready_calls(), 1);
}
