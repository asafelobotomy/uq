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

const NM_APPLET: &str = "[Desktop Entry]\nType=Application\nName=Network\nExec=nm-applet\n";

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";

mod autostart;
mod desktop_proxy;
mod domain_sections;
mod history;
mod pause_pick;
mod profiles;
mod sections;
mod welcome;
