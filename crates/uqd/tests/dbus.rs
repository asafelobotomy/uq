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

    let () = proxy.call("SetTheme", &("dracula", "light")).await.unwrap();
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
        .call("SetSetting", &("notif.backend", "\"mako\""))
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
        .call::<_, _, ()>("SetSetting", &("nope", "1"))
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

    let () = proxy.call("SetLayout", &("2", "scrolling")).await.unwrap();
    let layouts: HashMap<String, String> = proxy.call("GetLayouts", &()).await.unwrap();
    assert_eq!(layouts.get("2").map(String::as_str), Some("scrolling"));

    let err = proxy
        .call::<_, _, ()>("SetLayout", &("1", "monocle"))
        .await
        .unwrap_err();
    match err {
        zbus::Error::MethodError(name, _, _) => {
            assert_eq!(name.as_str(), "org.uq.Desktop1.Error.InvalidLayout")
        }
        other => panic!("unexpected error {other:?}"),
    }
    let err = proxy
        .call::<_, _, ()>("SetTheme", &("dracula", "dim"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, _, _) if n.as_str() == "org.uq.Desktop1.Error.InvalidArgument"),
        "{err:?}"
    );

    let () = proxy
        .call("SetSchedule", &(r#"{"mode":"off"}"#,))
        .await
        .unwrap();
    let err = proxy
        .call::<_, _, ()>("SetSchedule", &("not json",))
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
            &(r#"{"mode":"sun","lat":51.5,"lon":-0.13}"#,),
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
        .call("SetSection", &("wallpaper", json))
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
        .call::<_, _, ()>("SetSection", &("appearance", r#"{"font_size":2}"#))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
    let err = proxy
        .call::<_, _, String>("GetSection", &("bogus",))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("InvalidSetting"), "{err}");
}
