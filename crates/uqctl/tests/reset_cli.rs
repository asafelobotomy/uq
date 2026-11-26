//! `uqctl reset` and `uqctl backup` end to end: the real binary against a real uqd on a private
//! bus, with every home under a temporary directory. Never touches the user's session.

mod common;

use common::{connect, private_bus, serve_uqd};
use uq_core::client::DesktopProxy;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_and_backup_each_save_a_restore_point() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, root.path()).await;
    let uqctl = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_uqctl"))
            .args(args)
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap()
    };
    let conn = connect(&bus).await;
    let proxy = DesktopProxy::new(&conn).await.unwrap();
    let clock = || async {
        let v: serde_json::Value =
            serde_json::from_str(&proxy.get_section("shell").await.unwrap()).unwrap();
        v["clock_24h"].as_bool().unwrap()
    };
    proxy
        .set_section("shell", r#"{"clock_24h":false}"#, "test")
        .await
        .unwrap();

    let file = root.path().join("backup.toml");
    let o = uqctl(&["backup", "save", file.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .starts_with("# uQ settings backup")
    );

    // Without a terminal, a reset needs --yes.
    let o = uqctl(&["reset"]);
    assert!(!o.status.success());
    assert!(!clock().await);
    let o = uqctl(&["reset", "--yes"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(clock().await);

    let o = uqctl(&["backup", "restore", file.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!clock().await);
    let o = uqctl(&["restore", "list"]);
    let list = String::from_utf8_lossy(&o.stdout);
    assert!(list.contains("auto:before_reset"), "{list}");
    assert!(list.contains("auto:before_backup"), "{list}");

    std::fs::write(&file, "theme = \"x\"\n").unwrap();
    let o = uqctl(&["backup", "restore", file.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("isn't a uQ settings backup"));
}
