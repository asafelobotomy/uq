//! Discovery, gating and providers started on demand.

use super::*;

#[tokio::test]
async fn discovery_gating_and_live_changes() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    widget(r, "fake", FAKE);
    widget(r, "plain", "order = 90\n");
    widget(r, "nobins", "[requires]\nbins = [\"uq-no-such-program\"]\n");
    put(
        &r.join("shell/widgets/broken/widget.toml"),
        "id = \"broken\"\n",
    );
    let mut b = Bridge::start_with_root(&bus, &[], r);
    b.event("Hello").await;

    let start = first(&mut b, "Widgets").await;
    let ids: Vec<&str> = start
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["fake", "nobins", "plain"],
        "by order then id, broken skipped"
    );
    assert_eq!(visible(&start, "fake"), Some(false), "no owner yet");
    assert_eq!(visible(&start, "nobins"), Some(false));
    assert_eq!(visible(&start, "plain"), Some(true));
    let plain = find(&start, "plain").unwrap();
    assert_eq!(plain["slot"], "bar");
    assert_eq!(plain["order"], 90);
    assert_eq!(plain["name"], "Widget plain");
    let url = plain["qml_url"].as_str().unwrap();
    assert!(
        url.starts_with("file:///") && url.ends_with("/shell/widgets/plain/Widget.qml"),
        "{url}"
    );
    assert_eq!(first(&mut b, "AppActions").await, json!([]));

    // The service appears: the widget shows; it goes: hidden again.
    let svc = serve_fake(&bus).await;
    wait_for(&mut b, "Widgets", Duration::from_secs(5), |l| {
        visible(l, "fake") == Some(true)
    })
    .await;
    svc.release_name(NAME).await.unwrap();
    wait_for(&mut b, "Widgets", Duration::from_secs(5), |l| {
        visible(l, "fake") == Some(false)
    })
    .await;

    // A package installs a widget, then removes it: no restart needed.
    widget(r, "later", "slot = \"quick\"\n");
    let l = wait_for(&mut b, "Widgets", Duration::from_secs(6), |l| {
        find(l, "later").is_some()
    })
    .await;
    assert_eq!(find(&l, "later").unwrap()["slot"], "quick");
    std::fs::remove_dir_all(r.join("shell/widgets/later")).unwrap();
    wait_for(&mut b, "Widgets", Duration::from_secs(6), |l| {
        find(l, "later").is_none()
    })
    .await;
}

#[tokio::test]
async fn launcher_open_starts_cold_activatable_providers_once() {
    let bus = private_bus();
    counting_service(&bus, "org.test.Cold", "cold");
    counting_service(&bus, "org.test.Warm", "warm");
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    for (file, name, act) in [
        ("cold.provider", "org.test.Cold", "true"),
        ("warm.provider", "org.test.Warm", "true"),
        ("plain.provider", "org.test.Plain", "false"),
        ("nofile.provider", "org.test.NoFile", "true"),
    ] {
        put(
            &r.join("launcher").join(file),
            &format!("[Launcher Provider]\nBusName={name}\nName={name}\nActivatable={act}\n"),
        );
    }
    // Already running: never started again.
    let _warm = zbus::connection::Builder::address(bus.address.as_str())
        .name("org.test.Warm")
        .build()
        .await
        .unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut b = Bridge::start_with_root(&bus, &[("XDG_DATA_HOME", data.path().as_os_str())], r);
    b.event("Hello").await;

    // Typing never starts anything.
    b.call(1, "Search", json!(["hello", 1])).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(starts(&bus, "cold"), 0);

    // Opening the launcher starts the cold one, without waiting for it (the bus would wait
    // 25 s for a name the service never takes).
    let t = Instant::now();
    assert_eq!(
        b.call(2, "LauncherOpened", json!([])).await["ok"],
        Value::Null
    );
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    let end = Instant::now() + Duration::from_secs(15);
    while starts(&bus, "cold") == 0 && Instant::now() < end {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(starts(&bus, "cold"), 1);

    // Reopened within 30 s: not started again.
    assert_eq!(
        b.call(3, "LauncherOpened", json!([])).await["ok"],
        Value::Null
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(starts(&bus, "cold"), 1);
    assert_eq!(starts(&bus, "warm"), 0);
    let bad = b.call(4, "LauncherOpened", json!([1])).await;
    assert!(bad["err"].is_string(), "{bad}");
}
