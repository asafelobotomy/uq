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

impl Served {
    async fn set_look(&self, json: &str, origin: &str) -> String {
        self.set_section("look", json, origin).await
    }

    /// Writes a whole section and returns the id of the entry it journalled.
    async fn set_section(&self, domain: &str, json: &str, origin: &str) -> String {
        let () = self
            .proxy
            .call("SetSection", &(domain, json, origin))
            .await
            .unwrap();
        self.history().await[0]["id"].as_str().unwrap().to_string()
    }

    async fn look(&self) -> serde_json::Value {
        let s: String = self.proxy.call("GetSection", &("look",)).await.unwrap();
        serde_json::from_str(&s).unwrap()
    }
}

#[tokio::test]
async fn undo_of_a_section_entry_keeps_later_changes_to_other_keys() {
    let s = served().await;
    s.set_look(r#"{"gaps_in":5,"rounding":3}"#, "settings")
        .await;
    let a = s
        .set_look(r#"{"gaps_in":8,"rounding":3}"#, "settings")
        .await;
    s.set_look(r#"{"gaps_in":8,"rounding":6}"#, "settings")
        .await;

    s.undo(&a, true).await.unwrap();
    let look = s.look().await;
    assert_eq!(look["gaps_in"], 5, "{look}");
    assert_eq!(look["rounding"], 6, "{look}");
    let entry = s.history().await[0].clone();
    assert_eq!(entry["summary"], "Gaps between windows: 8 → 5");
}

#[tokio::test]
async fn undone_change_does_not_come_back_when_a_later_one_is_undone() {
    let s = served().await;
    s.set_look(r#"{"gaps_in":5,"rounding":3}"#, "settings")
        .await;
    let gaps = s
        .set_look(r#"{"gaps_in":8,"rounding":3}"#, "settings")
        .await;
    let rounding = s
        .set_look(r#"{"gaps_in":8,"rounding":6}"#, "settings")
        .await;

    // Undo the gaps change from the session sheet: rounding changed since, so it asks first.
    let err = s.undo(&gaps, false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    s.undo(&gaps, true).await.unwrap();
    // Then Ctrl+Z, which forces through the app's own later entries, undoes rounding.
    s.undo(&rounding, true).await.unwrap();

    let look = s.look().await;
    assert_eq!(
        look["gaps_in"], 5,
        "the undone gaps change came back: {look}"
    );
    assert_eq!(look["rounding"], 3, "{look}");
}

#[tokio::test]
async fn undo_of_a_wallpaper_entry_keeps_other_screens() {
    let s = served().await;
    let all = s
        .set_section(
            "wallpaper",
            r#"{"outputs":{"*":{"image":"/a.png"}}}"#,
            "settings",
        )
        .await;
    s.set_section(
        "wallpaper",
        r#"{"outputs":{"*":{"image":"/a.png"},"DP-1":{"image":"/b.png"}}}"#,
        "settings",
    )
    .await;
    s.undo(&all, true).await.unwrap();
    let got: String = s.proxy.call("GetSection", &("wallpaper",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["outputs"]["DP-1"]["image"], "/b.png", "{got}");
    assert!(got["outputs"].get("*").is_none(), "{got}");
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
    assert!(
        error_message(&err).contains("isn't installed any more"),
        "{err:?}"
    );
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

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";

fn report(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

#[tokio::test]
async fn profile_crud_over_dbus() {
    let s = served().await;
    let id: String = s.proxy.call("SetProfile", &(GAMING, false)).await.unwrap();
    assert_eq!(id, "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    let list = report(&list);
    assert_eq!(list[0]["id"], "gaming");
    assert_eq!(list[0]["keys"], 2);
    assert_eq!(list[0]["active"], false);

    let text: String = s.proxy.call("GetProfile", &("gaming",)).await.unwrap();
    let again = uq_core::profile::Profile::parse(&text).unwrap();
    assert_eq!(again, uq_core::profile::Profile::parse(GAMING).unwrap());

    let err = s
        .proxy
        .call::<_, _, String>("SetProfile", &(GAMING, false))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    let renamed = GAMING.replace("Gaming", "Games");
    let id: String = s
        .proxy
        .call("SetProfile", &(renamed.as_str(), true))
        .await
        .unwrap();
    assert_eq!(id, "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list)[0]["name"], "Games");

    let err = s
        .proxy
        .call::<_, _, String>("GetProfile", &("nope",))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    let err = s
        .proxy
        .call::<_, _, String>("SetProfile", &("format = 1\nid = \"../x\"\n", false))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidSetting");

    let () = s.proxy.call("DeleteProfile", &("gaming",)).await.unwrap();
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list), serde_json::json!([]));
}

#[tokio::test]
async fn apply_and_off_over_dbus_emit_profile_changed() {
    let s = served().await;
    let _: String = s.proxy.call("SetProfile", &(GAMING, false)).await.unwrap();
    let mut changed = s.proxy.receive_signal("ProfileChanged").await.unwrap();
    let next_id = |msg: zbus::Message| msg.body().deserialize::<String>().unwrap();
    let wait = std::time::Duration::from_secs(5);

    let json: String = s.proxy.call("ApplyProfile", &("gaming",)).await.unwrap();
    assert_eq!(report(&json)["active"], "gaming");
    assert_eq!(s.gaps().await, 0);
    let msg = tokio::time::timeout(wait, changed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_id(msg), "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list)[0]["active"], true);

    let json: String = s.proxy.call("ProfileOff", &()).await.unwrap();
    assert_eq!(report(&json)["active"], "");
    let msg = tokio::time::timeout(wait, changed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_id(msg), "");
    assert_ne!(s.gaps().await, 0);
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
    assert_eq!(proxy.list_windows().await.unwrap(), "[]");
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

#[tokio::test]
async fn input_section_round_trip_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let generated = paths.generated_lua.clone();
    let comp = Arc::new(FakeCompositor::running());
    *comp.devices_json.lock().unwrap() =
        r#"{"mice":[{"name":"mx","defaultSpeed":0.25}],"keyboards":[{"name":"kb"}],"touch":[]}"#
            .into();
    let desktop = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
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
            &(
                "input",
                r#"{"keyboard":{"layouts":[{"layout":"us"},{"layout":"de"}]}}"#,
                "settings",
            ),
        )
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("input",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["keyboard"]["layouts"][1]["layout"], "de");
    assert!(
        std::fs::read_to_string(&generated)
            .unwrap()
            .contains(r#"kb_layout = "us,de""#)
    );
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let (domain, _): (String, String) = msg.body().deserialize().unwrap();
    assert_eq!(domain, "input");

    let devices: String = proxy.call("ListInputDevices", &()).await.unwrap();
    let devices: serde_json::Value = serde_json::from_str(&devices).unwrap();
    assert_eq!(devices["pointers"][0]["name"], "mx");
    assert_eq!(devices["keyboards"][0]["name"], "kb");

    let err = proxy
        .call::<_, _, ()>("SetSection", &("monitors", r#"{"outputs":{}}"#, "settings"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
}

#[tokio::test]
async fn rules_section_and_list_windows_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let generated = paths.generated_lua.clone();
    let comp = Arc::new(FakeCompositor::running());
    *comp.clients_json.lock().unwrap() = r#"[{"address":"0x1","pid":7,"mapped":true,
        "class":"firefox","title":"Docs","initialClass":"firefox","initialTitle":"Docs",
        "workspace":{"id":2,"name":"2"},"floating":false,"xwayland":false,"pinned":false,
        "fullscreen":0,"tags":[]}]"#
        .into();
    let desktop = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
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

    let rules = r#"{"window":[{"name":"Floaty","enabled":true,"match":{"class":"^pavucontrol$"},
        "effects":{"float":true}}],"layer":[],"workspace":[]}"#;
    let () = proxy
        .call("SetSection", &("rules", rules, "settings"))
        .await
        .unwrap();
    let got: String = proxy.call("GetSection", &("rules",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    let id = got["window"][0]["id"].as_str().unwrap().to_string();
    assert!(
        std::fs::read_to_string(&generated)
            .unwrap()
            .contains(&format!("uq:{id}"))
    );
    let explained: String = proxy.call("Explain", &("rules",)).await.unwrap();
    let explained: serde_json::Value = serde_json::from_str(&explained).unwrap();
    assert_eq!(explained[format!("window.{id}")]["source"], "uq");

    let windows: String = proxy.call("ListWindows", &()).await.unwrap();
    let windows: serde_json::Value = serde_json::from_str(&windows).unwrap();
    assert_eq!(windows[0]["class"], "firefox");
    assert_eq!(windows[0]["workspace_id"], 2);
    assert!(windows[0].get("address").is_none() && windows[0].get("pid").is_none());
}

async fn pick_server() -> (
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
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
    )
    .await
    .unwrap();
    std::mem::forget(server);
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    (bus, client, comp, dir)
}

#[tokio::test]
async fn pick_over_dbus() {
    let (bus, client, _comp, _dir) = pick_server().await;
    let proxy = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    let mut requested = proxy.receive_pick_requested().await.unwrap();
    let shell = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
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

#[tokio::test]
async fn startup_sections_over_dbus() {
    let s = served().await;
    let writes = [
        ("env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#),
        ("compat", r#"{"qt_wayland":true}"#),
        (
            "autostart",
            r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
        ),
    ];
    for (domain, json) in writes {
        let () = s
            .proxy
            .call("SetSection", &(domain, json, "settings"))
            .await
            .unwrap();
        let got: String = s.proxy.call("GetSection", &(domain,)).await.unwrap();
        let got: serde_json::Value = serde_json::from_str(&got).unwrap();
        let sent: serde_json::Value = serde_json::from_str(json).unwrap();
        for (k, v) in sent.as_object().unwrap() {
            assert_eq!(&got[k], v, "{domain}.{k}");
        }
    }
    let g = std::fs::read_to_string(&s.paths.generated_lua).unwrap();
    assert!(g.contains(r#"hl.env("FOO", "bar", true)"#), "{g}");
    assert!(
        g.contains(r#"hl.env("QT_QPA_PLATFORM", "wayland;xcb", true)"#),
        "{g}"
    );
    let env_d = s.paths.startup.config_home.join("environment.d/60-uq.conf");
    let env_d = std::fs::read_to_string(env_d).unwrap();
    assert!(env_d.contains("FOO='bar'"), "{env_d}");

    let explain = |domain: &'static str| {
        let proxy = s.proxy.clone();
        async move {
            let e: String = proxy.call("Explain", &(domain,)).await.unwrap();
            serde_json::from_str::<serde_json::Value>(&e).unwrap()
        }
    };
    assert_eq!(explain("env").await["vars.FOO"]["source"], "uq");
    let compat = explain("compat").await;
    assert_eq!(compat["qt_wayland"]["source"], "uq");
    assert_eq!(compat["sharp_x11"]["source"], "default");
    assert_eq!(
        explain("autostart").await["nm-applet.desktop"]["source"],
        "uq"
    );

    let err = s
        .proxy
        .call::<_, _, ()>(
            "SetSection",
            &(
                "compat",
                r#"{"apps":[{"id":"code.desktop","platform":"x11","elsewhere":true}]}"#,
                "settings",
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidSetting");
    assert_eq!(error_message(&err), "uQ can't find this app's file.");
}

/// uqd served with a fake autostart spawner, with `files` in the user's autostart folder.
struct AutostartServed {
    _dir: tempfile::TempDir,
    _bus: uq_core::test_bus::Bus,
    _server: (zbus::Connection, uqd::dbus::SharedDesktop),
    fake: Arc<uqd::autostart::spawn::FakeSpawner>,
    proxy: uq_core::client::DesktopProxy<'static>,
}

async fn autostart_served(
    files: &[(&str, &str)],
    fake: uqd::autostart::spawn::FakeSpawner,
) -> AutostartServed {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let folder = paths.startup.user_autostart();
    std::fs::create_dir_all(&folder).unwrap();
    for (id, text) in files {
        std::fs::write(folder.join(id), text).unwrap();
    }
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let fake = Arc::new(fake);
    let cfg = uqd::autostart::AutostartConfig::new(
        fake.clone(),
        dir.path().join("run/uq/autostart-run.json"),
        Some("test_session".into()),
        false,
    );
    let server = uqd::dbus::serve_with(
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
        cfg,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = uq_core::client::DesktopProxy::new(&client).await.unwrap();
    AutostartServed {
        _dir: dir,
        _bus: bus,
        _server: server,
        fake,
        proxy,
    }
}

const NM_APPLET: &str = "[Desktop Entry]\nType=Application\nName=Network\nExec=nm-applet\n";

async fn autostart_rows(p: &uq_core::client::DesktopProxy<'_>) -> Vec<serde_json::Value> {
    serde_json::from_str(&p.list_autostart().await.unwrap()).unwrap()
}

#[tokio::test]
async fn list_autostart_over_dbus() {
    let s = autostart_served(&[("nm-applet.desktop", NM_APPLET)], Default::default()).await;
    let () = s
        .proxy
        .set_section(
            "autostart",
            r#"{"added":[{"name":"Backup","argv":["backup","--quiet"],"enabled":false}]}"#,
            "settings",
        )
        .await
        .unwrap();
    let rows = autostart_rows(&s.proxy).await;
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0]["id"], "nm-applet.desktop");
    assert_eq!(rows[0]["name"], "Network");
    assert_eq!(rows[0]["source"], "yours");
    assert_eq!(rows[0]["can_toggle"], true);
    assert_eq!(rows[0]["status"], "not_started");
    assert!(rows[0].get("argv").is_none());
    assert_eq!(rows[1]["source"], "added");
    assert_eq!(rows[1]["status"], "off");
    assert_eq!(rows[1]["argv"], serde_json::json!(["backup", "--quiet"]));

    s.proxy.shell_ready().await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while autostart_rows(&s.proxy).await[0]["status"] != "running" {
        assert!(std::time::Instant::now() < deadline, "never running");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let calls = s.fake.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].argv, ["nm-applet"]);
}

#[tokio::test]
async fn autostart_changed_signal() {
    let s = autostart_served(&[("nm-applet.desktop", NM_APPLET)], Default::default()).await;
    let mut changed = s.proxy.receive_autostart_changed().await.unwrap();
    s.proxy.shell_ready().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .expect("no AutostartChanged")
        .unwrap();
}

#[tokio::test]
async fn list_autostart_does_not_hold_the_desktop_lock() {
    let fake = uqd::autostart::spawn::FakeSpawner::default();
    fake.exit(
        "uq-autostart-nm-applet",
        uqd::autostart::spawn::ExitInfo::Code(3),
    );
    *fake.journal_delay.lock().unwrap() = Some(std::time::Duration::from_secs(2));
    *fake.journal.lock().unwrap() = vec!["boom".into()];
    let s = autostart_served(&[("nm-applet.desktop", NM_APPLET)], fake).await;
    let mut changed = s.proxy.receive_autostart_changed().await.unwrap();
    s.proxy.shell_ready().await.unwrap();
    // Waiting, running, failed.
    for _ in 0..3 {
        tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
            .await
            .expect("no AutostartChanged")
            .unwrap();
    }
    let lister = s.proxy.clone();
    let listing = tokio::spawn(async move { autostart_rows(&lister).await });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        s.fake.tails.lock().unwrap().len(),
        1,
        "the journal read began"
    );
    let look = s.proxy.get_section("look").await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        s.proxy.set_section("look", &look, "test"),
    )
    .await
    .expect("SetSection blocked while ListAutostart read the journal")
    .unwrap();
    let rows = listing.await.unwrap();
    assert_eq!(rows[0]["status"], "failed");
    assert_eq!(rows[0]["exit"], 3);
    assert_eq!(rows[0]["log"], serde_json::json!(["boom"]));
}

#[tokio::test]
async fn welcome_status_done_and_reset_over_dbus() {
    let s = served().await;
    let status = |json: String| serde_json::from_str::<serde_json::Value>(&json).unwrap();
    let current = uq_core::welcome::CURRENT_WELCOME_VERSION;

    let st = status(s.proxy.call("GetWelcome", &()).await.unwrap());
    assert_eq!(
        st,
        serde_json::json!({"done": false, "version": 0, "current": current, "pending": true})
    );

    // Out-of-range versions are refused and change nothing.
    for bad in [0u32, current + 1] {
        let err = s
            .proxy
            .call::<_, _, ()>("SetWelcomeDone", &(bad,))
            .await
            .unwrap_err();
        assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    }
    assert_eq!(
        status(s.proxy.call("GetWelcome", &()).await.unwrap())["pending"],
        true
    );

    let () = s.proxy.call("SetWelcomeDone", &(current,)).await.unwrap();
    let st = status(s.proxy.call("GetWelcome", &()).await.unwrap());
    assert_eq!(
        (st["done"].clone(), st["pending"].clone()),
        (true.into(), false.into())
    );
    let saved = uq_core::state::State::load(&s.paths.state_file);
    assert!(saved.welcome.done && saved.welcome.version == current);
    assert!(
        s.history().await.is_empty(),
        "finishing Welcome is not a settings change"
    );

    // A restore point taken before Welcome finished doesn't bring Welcome back.
    let () = s.proxy.call("ResetWelcome", &()).await.unwrap();
    let id: String = s
        .proxy
        .call("SaveRestorePoint", &("before",))
        .await
        .unwrap();
    let () = s.proxy.call("SetWelcomeDone", &(current,)).await.unwrap();
    s.set_gaps(9, "settings").await;
    s.restore(&id).await.unwrap();
    assert_eq!(
        status(s.proxy.call("GetWelcome", &()).await.unwrap())["pending"],
        false
    );

    let () = s.proxy.call("ResetWelcome", &()).await.unwrap();
    let st = status(s.proxy.call("GetWelcome", &()).await.unwrap());
    assert_eq!(st["pending"], true);
    assert!(
        !uq_core::state::State::load(&s.paths.state_file)
            .welcome
            .done
    );

    // The client proxy names the same methods.
    let proxy = uq_core::client::DesktopProxy::new(s.proxy.connection())
        .await
        .unwrap();
    proxy.set_welcome_done(current).await.unwrap();
    assert_eq!(status(proxy.get_welcome().await.unwrap())["pending"], false);
    proxy.reset_welcome().await.unwrap();
    assert_eq!(status(proxy.get_welcome().await.unwrap())["done"], false);
}
