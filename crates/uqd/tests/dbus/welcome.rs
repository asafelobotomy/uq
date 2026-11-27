//! Welcome status and integration rules.

use super::*;

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

/// Rules from apps over the bus: listed off, turned on with `SetSection`, a missing one refused.
#[tokio::test]
async fn integration_rules_over_dbus() {
    let dir = tempfile::tempdir().unwrap();
    let bus = private_bus();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(&dir.path().join("root"));
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let rules = paths.integrations_dir.join("romforge");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::write(
        rules.join("rules.toml"),
        "[[window_rule]]\nid = \"romforge.emulators\"\nclass = [\"^retroarch$\"]\n\
         workspace = \"name:game\"\nfullscreen = true\n",
    )
    .unwrap();
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    let _server = serve(
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
    let list = |text: String| serde_json::from_str::<serde_json::Value>(&text).unwrap();
    let v = list(proxy.call("ListIntegrationRules", &()).await.unwrap());
    assert_eq!(v["rules"][0]["id"], "romforge.emulators");
    assert_eq!(v["rules"][0]["enabled"], false);
    let () = proxy
        .call(
            "SetSection",
            &(
                "integration_rules",
                r#"{"enabled":["romforge.emulators"]}"#,
                "settings",
            ),
        )
        .await
        .unwrap();
    let v = list(proxy.call("ListIntegrationRules", &()).await.unwrap());
    assert_eq!(v["rules"][0]["enabled"], true);
    let err = proxy
        .call::<_, _, ()>(
            "SetSection",
            &(
                "integration_rules",
                r#"{"enabled":["romforge.emulators","hauler.x"]}"#,
                "settings",
            ),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, zbus::Error::MethodError(ref n, Some(ref m), _)
            if n.as_str() == "org.uq.Desktop1.Error.InvalidSetting" && m.contains("isn't installed")),
        "{err:?}"
    );
}
