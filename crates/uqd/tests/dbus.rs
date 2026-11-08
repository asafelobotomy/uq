//! D-Bus round trip against a private dbus-daemon (never the user's session bus).

use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use uqd::compositor::fake::FakeCompositor;
use uqd::dbus::{IFACE, NAME, PATH, serve};
use uqd::service::Desktop;
use uqd::system::FakeSystem;

struct Bus {
    child: Child,
    address: String,
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn private_bus(dir: &std::path::Path) -> Bus {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address"])
        .arg(format!("--address=unix:path={}", dir.join("bus").display()))
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    Bus {
        child,
        address: line.trim().to_string(),
    }
}

#[tokio::test]
async fn dbus_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus(dir.path());

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
