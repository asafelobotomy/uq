//! The theme calls over a private bus (never the user's session bus).

use futures_util::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uq_core::test_bus::private_bus;
use uqd::compositor::fake::FakeCompositor;
use uqd::dbus::{IFACE, NAME, PATH, serve};
use uqd::service::Desktop;
use uqd::system::FakeSystem;

struct Served {
    _dir: tempfile::TempDir,
    _bus: uq_core::test_bus::Bus,
    _server: (zbus::Connection, uqd::dbus::SharedDesktop),
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
        paths,
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
        proxy,
    }
}

fn error_name(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(n, _, _) => n.to_string(),
        other => format!("{other:?}"),
    }
}

fn error_text(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(t), _) => t.clone(),
        _ => String::new(),
    }
}

async fn ocean(s: &Served) -> String {
    let text: String = s
        .proxy
        .call("ExportTheme", &("gruvbox", false))
        .await
        .unwrap();
    text.replacen("id = \"gruvbox\"", "id = \"ocean\"", 1)
        .replacen("name = \"Gruvbox\"", "name = \"Ocean\"", 1)
}

async fn info(s: &Served) -> serde_json::Value {
    let json: String = s.proxy.call("ListThemeInfo", &()).await.unwrap();
    serde_json::from_str(&json).unwrap()
}

#[tokio::test]
async fn import_list_set_delete_over_dbus() {
    let s = served().await;
    let text = ocean(&s).await;
    let mut changed = s.proxy.receive_signal("ThemesChanged").await.unwrap();
    let id: String = s
        .proxy
        .call("ImportTheme", &(text.as_str(), false, false, "settings"))
        .await
        .unwrap();
    assert_eq!(id, "user-ocean");
    tokio::time::timeout(Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();
    let v = info(&s).await;
    let t = v["themes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "user-ocean")
        .unwrap();
    assert_eq!((&t["user"], &t["name"]), (&true.into(), &"Ocean".into()));

    let () = s
        .proxy
        .call("SetTheme", &("user-ocean", "light", "settings"))
        .await
        .unwrap();
    let out: String = s
        .proxy
        .call("DeleteTheme", &("user-ocean", "settings"))
        .await
        .unwrap();
    let out: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(out["switched"], true);
    let ids: Vec<String> = s.proxy.call("ListThemes", &()).await.unwrap();
    assert!(!ids.contains(&"user-ocean".to_string()));
}

#[tokio::test]
async fn import_errors_have_plain_names() {
    let s = served().await;
    let text = ocean(&s).await;
    let () = s
        .proxy
        .call("ImportTheme", &(text.as_str(), false, false, "settings"))
        .await
        .map(|_: String| ())
        .unwrap();
    let err = s
        .proxy
        .call::<_, _, String>("ImportTheme", &(text.as_str(), false, false, "settings"))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    let err = s
        .proxy
        .call::<_, _, String>("ImportTheme", &("not a theme", false, false, "settings"))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    assert_eq!(error_text(&err), "This file isn't a uQ theme.");
    let err = s
        .proxy
        .call::<_, _, String>("DeleteTheme", &("user-nope", "settings"))
        .await
        .unwrap_err();
    let text = error_text(&err);
    assert!(
        text.starts_with(char::is_uppercase) && text.ends_with('.'),
        "{text}"
    );
}

#[tokio::test]
async fn oversized_text_refused_before_lock() {
    let s = served().await;
    let big = " ".repeat(70_000);
    let err = s
        .proxy
        .call::<_, _, String>("ImportTheme", &(big.as_str(), false, false, "settings"))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    assert_eq!(error_text(&err), "The file is too big to be a theme.");
    assert!(
        info(&s).await["themes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["user"] == false)
    );
}

#[tokio::test]
async fn export_round_trips_over_dbus() {
    let s = served().await;
    let text = ocean(&s).await;
    let _: String = s
        .proxy
        .call("ImportTheme", &(text.as_str(), false, false, "settings"))
        .await
        .unwrap();
    let out: String = s
        .proxy
        .call("ExportTheme", &("user-ocean", false))
        .await
        .unwrap();
    assert_eq!(out, text);
    let err = s
        .proxy
        .call::<_, _, String>("ExportTheme", &("nope", false))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.UnknownTheme");
}
