//! Autostart rows and their signal.

use super::*;

/// uqd served with a fake autostart spawner, with `files` in the user's autostart folder.
pub(crate) struct AutostartServed {
    pub(crate) _dir: tempfile::TempDir,
    pub(crate) _bus: uq_core::test_bus::Bus,
    pub(crate) _server: (zbus::Connection, uqd::dbus::SharedDesktop),
    pub(crate) fake: Arc<uqd::autostart::spawn::FakeSpawner>,
    pub(crate) proxy: uq_core::client::DesktopProxy<'static>,
}

pub(crate) async fn autostart_served(
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
        zbus::connection::Builder::address(bus.address.as_str()),
        desktop,
        cfg,
    )
    .await
    .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
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

pub(crate) async fn autostart_rows(
    p: &uq_core::client::DesktopProxy<'_>,
) -> Vec<serde_json::Value> {
    serde_json::from_str(&p.list_autostart().await.unwrap()).unwrap()
}

#[tokio::test]
pub(crate) async fn list_autostart_over_dbus() {
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
pub(crate) async fn autostart_changed_signal() {
    let s = autostart_served(&[("nm-applet.desktop", NM_APPLET)], Default::default()).await;
    let mut changed = s.proxy.receive_autostart_changed().await.unwrap();
    s.proxy.shell_ready().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .expect("no AutostartChanged")
        .unwrap();
}

#[tokio::test]
pub(crate) async fn list_autostart_does_not_hold_the_desktop_lock() {
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
