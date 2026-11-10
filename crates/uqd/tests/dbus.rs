//! D-Bus round trip against a private dbus-daemon (never the user's session bus).

use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uq_core::test_bus::private_bus;
use uqd::compositor::fake::FakeCompositor;
use uqd::dbus::{IFACE, NAME, PATH, serve};
use uqd::service::Desktop;
use uqd::system::FakeSystem;

#[tokio::test]
async fn dbus_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = serve(
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
    let mut changed = proxy.receive_signal("ThemeChanged").await.unwrap();

    let ids: Vec<String> = proxy.call("ListThemes", &()).await.unwrap();
    assert_eq!(ids.len(), 7);

    let () = proxy
        .call("SetTheme", &("dracula", "light", "settings"))
        .await
        .unwrap();
    let theme: (String, String) = proxy.call("GetTheme", &()).await.unwrap();
    assert_eq!(theme, ("dracula".to_string(), "light".to_string()));

    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let (name, variant, tokens): (String, String, String) = msg.body().deserialize().unwrap();
    assert_eq!((name.as_str(), variant.as_str()), ("dracula", "light"));
    assert!(tokens.ends_with("tokens.json"), "{tokens}");

    let mut settings_changed = proxy.receive_signal("SettingsChanged").await.unwrap();
    let () = proxy
        .call("SetSetting", &("notif.backend", "\"mako\"", "settings"))
        .await
        .unwrap();
    let json: String = proxy.call("GetSettings", &()).await.unwrap();
    assert!(json.contains("\"mako\""), "{json}");
    assert!(!json.contains("launch_counts"), "{json}");
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), settings_changed.next())
        .await
        .unwrap()
        .unwrap();
    let body: String = msg.body().deserialize().unwrap();
    assert!(body.contains("\"mako\""), "{body}");
    let err = proxy
        .call::<_, _, ()>("SetSetting", &("nope", "1", ""))
        .await
        .unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.InvalidSetting"),
        "{err:?}"
    );

    let mut launches = proxy.receive_signal("LaunchCountsChanged").await.unwrap();
    let () = proxy.call("RecordLaunch", &("a.desktop",)).await.unwrap();
    let counts: HashMap<String, u32> = proxy.call("GetLaunchCounts", &()).await.unwrap();
    assert_eq!(counts.get("a.desktop"), Some(&1));
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), launches.next())
        .await
        .unwrap()
        .unwrap();
    let body: HashMap<String, u32> = msg.body().deserialize().unwrap();
    assert_eq!(body.get("a.desktop"), Some(&1));
    let () = proxy.call("ClearLaunchCounts", &()).await.unwrap();
    let counts: HashMap<String, u32> = proxy.call("GetLaunchCounts", &()).await.unwrap();
    assert!(counts.is_empty());

    let () = proxy
        .call("SetLayout", &("2", "scrolling", "settings"))
        .await
        .unwrap();
    let layouts: HashMap<String, String> = proxy.call("GetLayouts", &()).await.unwrap();
    assert_eq!(layouts.get("2").map(String::as_str), Some("scrolling"));

    let err = proxy
        .call::<_, _, ()>("SetLayout", &("1", "monocle", ""))
        .await
        .unwrap_err();
    match err {
        zbus::Error::MethodError(name, _, _) => {
            assert_eq!(name.as_str(), "org.uq.Desktop1.Error.InvalidLayout")
        }
        other => panic!("unexpected error {other:?}"),
    }
    let err = proxy
        .call::<_, _, ()>("SetTheme", &("dracula", "dim", ""))
        .await
        .unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.InvalidArgument"),
        "{err:?}"
    );

    let () = proxy
        .call("SetSchedule", &(r#"{"mode":"off"}"#, "settings"))
        .await
        .unwrap();
    let err = proxy
        .call::<_, _, ()>("SetSchedule", &("not json", ""))
        .await
        .unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.InvalidSchedule"),
        "{err:?}"
    );
}

#[tokio::test]
async fn get_schedule_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = serve(
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

    let fresh: String = proxy.call("GetSchedule", &()).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fresh).unwrap(),
        serde_json::json!({"mode":"off"})
    );
    let () = proxy
        .call(
            "SetSchedule",
            &(r#"{"mode":"sun","lat":51.5,"lon":-0.13}"#, "settings"),
        )
        .await
        .unwrap();
    let got: String = proxy.call("GetSchedule", &()).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["mode"], "sun");
    assert_eq!(got["lat"], 51.5);
}

#[tokio::test]
async fn section_round_trip_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = serve(
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
    let mut changed = proxy.receive_signal("SectionChanged").await.unwrap();

    let json = r#"{"outputs":{"*":{"image":"/a.png","fit":"tile"}}}"#;
    let () = proxy
        .call("SetSection", &("wallpaper", json, "settings"))
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("wallpaper",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["outputs"]["*"]["image"], "/a.png");
    assert_eq!(got["outputs"]["*"]["fit"], "tile");

    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let (domain, body): (String, String) = msg.body().deserialize().unwrap();
    assert_eq!(domain, "wallpaper");
    assert!(body.contains("/a.png"), "{body}");

    let err = proxy
        .call::<_, _, ()>("SetSection", &("appearance", r#"{"font_size":2}"#, ""))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
    let err = proxy
        .call::<_, _, String>("GetSection", &("bogus",))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
}

#[tokio::test]
async fn look_section_round_trip_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let generated = paths.generated_lua.clone();
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = serve(
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
    let mut changed = proxy.receive_signal("SectionChanged").await.unwrap();

    let () = proxy
        .call(
            "SetSection",
            &("look", r#"{"gaps_in":9,"blur_size":5}"#, "settings"),
        )
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("look",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["gaps_in"], 9);
    assert!(
        std::fs::read_to_string(&generated)
            .unwrap()
            .contains("gaps_in = 9")
    );

    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let (domain, body): (String, String) = msg.body().deserialize().unwrap();
    assert_eq!(domain, "look");
    assert!(body.contains("\"gaps_in\":9"), "{body}");

    let err = proxy
        .call::<_, _, ()>("SetSection", &("look", r#"{"gaps_in":999}"#, ""))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
}

/// uqd on a private bus, with a client proxy for `org.uq.Desktop1`.
struct Served {
    _dir: tempfile::TempDir,
    _bus: uq_core::test_bus::Bus,
    _server: (zbus::Connection, uqd::dbus::SharedDesktop),
    paths: uq_core::paths::Paths,
    proxy: zbus::Proxy<'static>,
}

async fn served() -> Served {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths.clone(),
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
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
        paths,
        proxy,
    }
}

impl Served {
    async fn set_gaps(&self, gaps: u32, origin: &str) {
        let json = format!(r#"{{"gaps_in":{gaps}}}"#);
        let () = self
            .proxy
            .call("SetSection", &("look", json.as_str(), origin))
            .await
            .unwrap();
    }

    async fn gaps(&self) -> serde_json::Value {
        let s: String = self.proxy.call("GetSection", &("look",)).await.unwrap();
        serde_json::from_str::<serde_json::Value>(&s).unwrap()["gaps_in"].clone()
    }

    async fn history(&self) -> Vec<serde_json::Value> {
        let s: String = self.proxy.call("ListHistory", &(100u32,)).await.unwrap();
        serde_json::from_str(&s).unwrap()
    }

    async fn points(&self) -> Vec<serde_json::Value> {
        let s: String = self.proxy.call("ListRestorePoints", &()).await.unwrap();
        serde_json::from_str(&s).unwrap()
    }

    async fn undo(&self, id: &str, force: bool) -> zbus::Result<String> {
        self.proxy.call("Undo", &(id, force)).await
    }

    async fn restore(&self, id: &str) -> zbus::Result<()> {
        self.proxy.call("Restore", &(id,)).await
    }

    fn point_file(&self, id: &str) -> PathBuf {
        self.paths.restore_dir.join(format!("{id}.toml"))
    }
}

fn error_name(err: &zbus::Error) -> &str {
    match err {
        zbus::Error::MethodError(n, _, _) => n.as_str(),
        other => panic!("unexpected error {other:?}"),
    }
}

fn error_message(err: &zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(_, Some(m), _) => m.clone(),
        other => panic!("unexpected error {other:?}"),
    }
}

#[tokio::test]
async fn set_section_with_origin_records_it() {
    let s = served().await;
    let mut changed = s.proxy.receive_signal("HistoryChanged").await.unwrap();
    s.set_gaps(9, "uqctl").await;
    let history = s.history().await;
    assert_eq!(history.len(), 1, "{history:?}");
    assert_eq!(history[0]["origin"], "uqctl");
    assert_eq!(history[0]["domain"], "look");
    assert_eq!(history[0]["after"]["gaps_in"], 9);
    tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();

    // An empty origin is recorded as unknown; the limit caps the list.
    s.set_gaps(10, "").await;
    let history = s.history().await;
    assert_eq!(history[0]["origin"], "unknown");
    let one: String = s.proxy.call("ListHistory", &(1u32,)).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&one)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn undo_round_trip_over_dbus() {
    let s = served().await;
    let before = s.gaps().await;
    s.set_gaps(9, "settings").await;
    let id = s.history().await[0]["id"].as_str().unwrap().to_string();
    let new_id = s.undo(&id, false).await.unwrap();
    assert_eq!(s.gaps().await, before);
    let history = s.history().await;
    assert_eq!(history[0]["id"], new_id.as_str());
    assert_eq!(history[0]["origin"], "restore");
    assert_eq!(history[0]["domain"], "look");
}

#[tokio::test]
async fn undo_conflict_then_force() {
    let s = served().await;
    let before = s.gaps().await;
    s.set_gaps(9, "settings").await;
    let first = s.history().await[0]["id"].as_str().unwrap().to_string();
    s.set_gaps(12, "uqctl").await;

    let err = s.undo(&first, false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    assert!(
        error_message(&err).ends_with("was changed again after this. Undo anyway?"),
        "{err:?}"
    );
    assert_eq!(s.gaps().await, 12);

    s.undo(&first, true).await.unwrap();
    assert_eq!(s.gaps().await, before);
}

#[tokio::test]
async fn undo_unknown_id_is_invalid_argument() {
    let s = served().await;
    let err = s.undo("1700000000-0000", false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
}

#[tokio::test]
async fn undo_theme_and_schedule_pseudo_domains() {
    let s = served().await;
    let (name, variant): (String, String) = s.proxy.call("GetTheme", &()).await.unwrap();
    let () = s
        .proxy
        .call("SetTheme", &("dracula", "light", "settings"))
        .await
        .unwrap();
    let id = s.history().await[0]["id"].as_str().unwrap().to_string();
    s.undo(&id, false).await.unwrap();
    let now: (String, String) = s.proxy.call("GetTheme", &()).await.unwrap();
    assert_eq!(now, (name, variant));

    let () = s
        .proxy
        .call(
            "SetSchedule",
            &(r#"{"mode":"sun","lat":51.5,"lon":-0.13}"#, "settings"),
        )
        .await
        .unwrap();
    let entry = s.history().await[0].clone();
    assert_eq!(entry["domain"], "schedule");
    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    let got: String = s.proxy.call("GetSchedule", &()).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&got).unwrap()["mode"],
        "off"
    );

    let settings = |json: String| serde_json::from_str::<serde_json::Value>(&json).unwrap();
    let dim_before =
        settings(s.proxy.call("GetSettings", &()).await.unwrap())["idle"]["dim"].clone();
    let () = s
        .proxy
        .call("SetSetting", &("idle.dim", "200", "settings"))
        .await
        .unwrap();
    let entry = s.history().await[0].clone();
    assert_eq!(entry["domain"], "settings");
    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    let got = settings(s.proxy.call("GetSettings", &()).await.unwrap());
    assert_ne!(dim_before, 200);
    assert_eq!(got["idle"]["dim"], dim_before, "{got}");
}

#[tokio::test]
async fn restore_brings_back_old_state_and_is_undoable() {
    let s = served().await;
    s.set_gaps(9, "settings").await;
    let id: String = s.proxy.call("SaveRestorePoint", &("mine",)).await.unwrap();
    s.set_gaps(12, "settings").await;

    s.restore(&id).await.unwrap();
    assert_eq!(s.gaps().await, 9);
    let points = s.points().await;
    assert!(
        points
            .iter()
            .any(|p| p["kind"] == serde_json::json!({"auto": "before_restore"})),
        "{points:?}"
    );
    let entry = s.history().await[0].clone();
    assert_eq!(entry["origin"], "restore");
    assert_eq!(entry["domain"], "look");

    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    assert_eq!(s.gaps().await, 12);
}

#[tokio::test]
async fn restore_with_missing_theme_changes_nothing() {
    let s = served().await;
    let id: String = s.proxy.call("SaveRestorePoint", &("old",)).await.unwrap();
    let path = s.point_file(&id);
    let mut point: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    point.insert("theme".into(), toml::Value::String("uninstalled".into()));
    std::fs::write(&path, toml::to_string(&point).unwrap()).unwrap();
    s.set_gaps(12, "settings").await;
    let points = s.points().await.len();
    let history = s.history().await.len();

    let err = s.restore(&id).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.UnknownTheme");
    assert!(error_message(&err).contains("uninstalled"), "{err:?}");
    assert_eq!(s.gaps().await, 12);
    assert_eq!(s.points().await.len(), points);
    assert_eq!(s.history().await.len(), history);
}

#[tokio::test]
async fn restore_damaged_point_changes_nothing() {
    let s = served().await;
    let id: String = s.proxy.call("SaveRestorePoint", &("old",)).await.unwrap();
    s.set_gaps(12, "settings").await;
    let points = s.points().await.len();
    for damage in [
        "",
        "theme = \"nord\"\n[look\ngaps_in = ",
        "variant = \"purple\"\n",
        "theme = 5\n",
    ] {
        std::fs::write(s.point_file(&id), damage).unwrap();
        let err = s.restore(&id).await.unwrap_err();
        assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
        assert_eq!(error_message(&err), "restore point is damaged");
        assert_eq!(s.gaps().await, 12);
        assert_eq!(s.points().await.len(), points);
        // The damaged file is left where it is, untouched, and still listed.
        assert_eq!(std::fs::read_to_string(s.point_file(&id)).unwrap(), damage);
        assert!(s.points().await.iter().any(|p| p["id"] == id.as_str()));
    }
}

#[tokio::test]
async fn restore_point_crud() {
    let s = served().await;
    assert!(s.points().await.is_empty());
    let id: String = s.proxy.call("SaveRestorePoint", &("a",)).await.unwrap();
    let points = s.points().await;
    assert_eq!(points.len(), 1, "{points:?}");
    assert_eq!(points[0]["id"], id.as_str());
    assert_eq!(points[0]["name"], "a");
    assert_eq!(points[0]["kind"], "manual");

    let () = s.proxy.call("DeleteRestorePoint", &(&id,)).await.unwrap();
    assert!(s.points().await.is_empty());
    assert!(!s.point_file(&id).exists());

    let err = s
        .proxy
        .call::<_, _, ()>("DeleteRestorePoint", &("../x",))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    let err = s.restore(&id).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
}

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
}
