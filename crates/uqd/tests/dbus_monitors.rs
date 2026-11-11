//! Displays over D-Bus: uqd's own timer reverts an unconfirmed change.

use futures_util::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uq_core::test_bus::private_bus;
use uqd::compositor::fake::FakeCompositor;
use uqd::dbus::{IFACE, NAME, PATH, serve};
use uqd::service::Desktop;
use uqd::system::FakeSystem;

const FIXTURE: &str = include_str!("../../uq-core/tests/fixtures/monitors-all.json");
const SCALE: &str = r#"{"outputs":{"DP-1":{"scale":1.5}}}"#;

struct Served {
    _dir: tempfile::TempDir,
    _bus: uq_core::test_bus::Bus,
    _server: (zbus::Connection, uqd::dbus::SharedDesktop),
    comp: Arc<FakeCompositor>,
    proxy: zbus::Proxy<'static>,
}

async fn served(window: Duration) -> Served {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    *comp.monitors_json.lock().unwrap() = FIXTURE.into();
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_revert_after(window);
    let server = serve(
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(&client, NAME, PATH, IFACE).await.unwrap();
    Served {
        _dir: dir,
        _bus: bus,
        _server: server,
        comp,
        proxy,
    }
}

impl Served {
    async fn apply(&self, json: &str) -> String {
        self.proxy.call("ApplyMonitors", &(json,)).await.unwrap()
    }

    async fn outputs(&self) -> serde_json::Value {
        let s: String = self.proxy.call("ListOutputs", &()).await.unwrap();
        serde_json::from_str(&s).unwrap()
    }

    /// The calls after the last `eval`.
    fn after_eval(&self) -> Vec<String> {
        let calls = self.comp.calls();
        let i = calls.iter().rposition(|c| c.starts_with("eval ")).unwrap();
        calls[i + 1..].to_vec()
    }
}

fn is_invalid_setting(err: &zbus::Error) -> bool {
    matches!(err, zbus::Error::MethodError(n, _, _) if n.as_str().ends_with("InvalidSetting"))
}

#[tokio::test]
async fn unconfirmed_apply_reverts_on_its_own() {
    let s = served(Duration::from_millis(300)).await;
    let mut changed = s.proxy.receive_signal("MonitorsChanged").await.unwrap();
    let token = s.apply(SCALE).await;
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(3), changed.next())
            .await
            .expect("MonitorsChanged")
            .unwrap();
    }
    assert_eq!(s.after_eval(), ["reload"]);
    let err = s
        .proxy
        .call::<_, _, ()>("ConfirmMonitors", &(token.as_str(), "settings"))
        .await
        .unwrap_err();
    assert!(is_invalid_setting(&err), "{err}");
    assert!(s.outputs().await["pending"].is_null());
}

#[tokio::test]
async fn confirmed_apply_survives_the_window() {
    let s = served(Duration::from_millis(300)).await;
    let token = s.apply(SCALE).await;
    let () = s
        .proxy
        .call("ConfirmMonitors", &(token.as_str(), "settings"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!s.after_eval().contains(&"reload".to_string()));
    let got: String = s.proxy.call("GetSection", &("monitors",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["outputs"]["DP-1"]["scale"], 1.5);
    let history: String = s.proxy.call("ListHistory", &(10u32,)).await.unwrap();
    let history: serde_json::Value = serde_json::from_str(&history).unwrap();
    assert_eq!(history[0]["domain"], "monitors");
    assert_eq!(history[0]["origin"], "settings");
}

#[tokio::test]
async fn confirm_origin_is_recorded() {
    let s = served(Duration::from_secs(15)).await;
    for (origin, want) in [("uqctl", "uqctl"), ("", "unknown")] {
        let scale = if origin.is_empty() { 2.0 } else { 1.25 };
        let token = s
            .apply(&format!(r#"{{"outputs":{{"DP-1":{{"scale":{scale}}}}}}}"#))
            .await;
        let () = s
            .proxy
            .call("ConfirmMonitors", &(token.as_str(), origin))
            .await
            .unwrap();
        let history: String = s.proxy.call("ListHistory", &(1u32,)).await.unwrap();
        let history: serde_json::Value = serde_json::from_str(&history).unwrap();
        assert_eq!(history[0]["origin"], want);
    }
}

#[tokio::test]
async fn revert_now_over_dbus() {
    let s = served(Duration::from_secs(15)).await;
    let token = s.apply(SCALE).await;
    let () = s
        .proxy
        .call("RevertMonitors", &(token.as_str(),))
        .await
        .unwrap();
    assert_eq!(s.after_eval(), ["reload"]);
    assert!(s.outputs().await["pending"].is_null());
    let err = s
        .proxy
        .call::<_, _, ()>("RevertMonitors", &(token.as_str(),))
        .await
        .unwrap_err();
    assert!(is_invalid_setting(&err), "{err}");
}

#[tokio::test]
async fn list_outputs_reports_pending() {
    let s = served(Duration::from_millis(300)).await;
    let v = s.outputs().await;
    assert!(v["pending"].is_null());
    assert_eq!(v["live"].as_array().unwrap().len(), 2);
    let token = s.apply(SCALE).await;
    let v = s.outputs().await;
    assert_eq!(v["pending"]["token"], token.as_str());
    assert!(v["pending"]["seconds_left"].as_u64().unwrap() <= 1);
    let () = s
        .proxy
        .call("ConfirmMonitors", &(token.as_str(), "settings"))
        .await
        .unwrap();
    assert!(s.outputs().await["pending"].is_null());
}

#[tokio::test]
async fn apply_refusals_name_the_reason() {
    let s = served(Duration::from_secs(15)).await;
    let err = s
        .proxy
        .call::<_, _, String>(
            "ApplyMonitors",
            &(r#"{"outputs":{"eDP-1":{"enabled":false},"DP-1":{"enabled":false}}}"#,),
        )
        .await
        .unwrap_err();
    assert!(is_invalid_setting(&err), "{err}");
    assert!(
        err.to_string()
            .contains("at least one display must stay on")
    );
    let err = s
        .proxy
        .call::<_, _, ()>(
            "SetSection",
            &(
                "monitors",
                r#"{"outputs":{"DP-1":{"scale":1.5}}}"#,
                "settings",
            ),
        )
        .await
        .unwrap_err();
    assert!(is_invalid_setting(&err), "{err}");
}

#[tokio::test]
async fn remember_over_dbus() {
    let s = served(Duration::from_secs(15)).await;
    let id: String = s
        .proxy
        .call("RememberMonitors", &("Desk", "settings"))
        .await
        .unwrap();
    assert!(id.starts_with("m-"), "{id}");
    let v = s.outputs().await;
    assert_eq!(v["arrangement"]["name"], "Desk");
    assert_eq!(v["roles"]["main"], "eDP-1");
    // Refused while a display change waits.
    let _token = s.apply(SCALE).await;
    let err = s
        .proxy
        .call::<_, _, String>("RememberMonitors", &("Other", "settings"))
        .await
        .unwrap_err();
    assert!(is_invalid_setting(&err), "{err}");
}
