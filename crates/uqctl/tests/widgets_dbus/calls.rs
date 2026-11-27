//! Calls and declared commands.

use super::*;

#[tokio::test]
async fn calls_reach_only_listed_members_within_limits() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    widget(r, "fake", FAKE);
    widget(r, "nobins", "[requires]\nbins = [\"uq-no-such-program\"]\n");
    // No requires.bus_name: the call may start an activatable name, else it fails at once.
    widget(
        r,
        "ghost",
        "[dbus]\nname = \"org.test.Ghost\"\npath = \"/g\"\ninterface = \"org.test.Ghost\"\nmethods = [\"Boo\"]\n",
    );
    widget(
        r,
        "nobody",
        "[dbus]\nname = \"org.test.Nobody\"\npath = \"/n\"\ninterface = \"org.test.Nobody\"\nmethods = [\"Hi\"]\n",
    );
    let svc = serve_fake(&bus).await;
    let mut b = Bridge::start_with_root(&bus, &[], r);
    b.event("Hello").await;
    wait_for(&mut b, "Widgets", Duration::from_secs(5), |l| {
        visible(l, "fake") == Some(true)
    })
    .await;

    let r1 = b.call(1, "WidgetCall", json!(["fake", "Pause", []])).await;
    assert_eq!(r1["ok"], Value::Null, "{r1}");
    let r2 = b
        .call(2, "WidgetCall", json!(["fake", "Add", ["https://x"]]))
        .await;
    assert_eq!(r2["ok"], true, "{r2}");
    let r3 = b.call(3, "WidgetCall", json!(["fake", "Secret", []])).await;
    assert!(r3["err"].as_str().unwrap().contains("not one of"), "{r3}");
    let r4 = b.call(4, "WidgetCall", json!(["fake", "Add", [5]])).await;
    assert!(r4["err"].is_string(), "types come from introspection: {r4}");
    let r5 = b.call(5, "WidgetGet", json!(["fake"])).await;
    assert_eq!(r5["ok"], json!({"Count": 0, "Name": "fake"}), "{r5}");
    let r6 = b.call(6, "WidgetCall", json!(["nobins", "X", []])).await;
    assert!(r6["err"].as_str().unwrap().contains("hidden"), "{r6}");
    let r7 = b.call(7, "WidgetCall", json!(["nope", "X", []])).await;
    assert!(r7["err"].as_str().unwrap().contains("no widget"), "{r7}");
    let big_arg = "x".repeat(17 * 1024);
    let r8 = b
        .call(8, "WidgetCall", json!(["fake", "Add", [big_arg]]))
        .await;
    assert!(r8["err"].as_str().unwrap().contains("16 KiB"), "{r8}");
    let r9 = b.call(9, "WidgetCall", json!(["fake", "Big", []])).await;
    assert!(r9["err"].as_str().unwrap().contains("256 KiB"), "{r9}");
    let r10 = b
        .call(10, "WidgetCall", json!(["fake", "Pause", "x"]))
        .await;
    assert!(r10["err"].is_string(), "{r10}");

    // Properties: listed ones only, coalesced to at most 4 a second.
    let iface = svc
        .object_server()
        .interface::<_, Fake>(PATH)
        .await
        .unwrap();
    let t0 = Instant::now();
    for i in 1..=20u32 {
        iface.get_mut().await.count = i;
        iface
            .get()
            .await
            .count_changed(iface.signal_emitter())
            .await
            .unwrap();
        iface
            .get()
            .await
            .hidden_changed(iface.signal_emitter())
            .await
            .unwrap();
    }
    let mut events = 0;
    loop {
        let p = wait_for(&mut b, "WidgetProps", Duration::from_secs(10), |_| true).await;
        events += 1;
        assert_eq!(p["widget"], "fake");
        assert!(p["props"].get("Hidden").is_none(), "{p}");
        if p["props"]["Count"] == 20 {
            break;
        }
    }
    let rate_cap = 1 + (t0.elapsed().as_millis() / 250) as usize;
    assert!(events <= rate_cap, "{events} events in {:?}", t0.elapsed());

    // Signals: listed ones only.
    Fake::other(iface.signal_emitter(), "x").await.unwrap();
    Fake::finished(iface.signal_emitter(), "job", true)
        .await
        .unwrap();
    let s = wait_for(&mut b, "WidgetSignal", Duration::from_secs(10), |_| true).await;
    assert_eq!(
        s,
        json!({"widget": "fake", "member": "Finished", "args": ["job", true]})
    );

    // Off screen: no WidgetProps (signals still come); back on screen, the current values.
    let r = b.call(20, "WidgetShown", json!(["fake", false])).await;
    assert_eq!(r["ok"], Value::Null, "{r}");
    iface.get_mut().await.count = 30;
    iface
        .get()
        .await
        .count_changed(iface.signal_emitter())
        .await
        .unwrap();
    Fake::finished(iface.signal_emitter(), "off", true)
        .await
        .unwrap();
    let s = wait_for(&mut b, "WidgetSignal", Duration::from_secs(10), |_| true).await;
    assert_eq!(s["args"], json!(["off", true]));
    let quiet = Instant::now() + Duration::from_millis(800);
    while let Ok(v) = tokio::time::timeout_at(quiet, b.next()).await {
        assert_ne!(v["event"], "WidgetProps", "props while off screen: {v}");
    }
    let r = b.call(21, "WidgetShown", json!(["fake", true])).await;
    assert_eq!(r["ok"], Value::Null, "{r}");
    let p = wait_for(&mut b, "WidgetProps", Duration::from_secs(10), |_| true).await;
    assert_eq!(p["props"], json!({"Count": 30, "Name": "fake"}), "{p}");
    let r = b.call(22, "WidgetShown", json!(["fake"])).await;
    assert!(r["err"].is_string(), "{r}");

    // Deadline: a call that takes 10 s ends with an error after 2 s.
    let t = Instant::now();
    let r11 = b.call(11, "WidgetCall", json!(["fake", "Slow", []])).await;
    assert!(r11["err"].as_str().unwrap().contains("2 s"), "{r11}");
    assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());

    // At most 8 in flight per widget: the ninth is busy at once.
    for id in 100..109 {
        b.send(id, "WidgetCall", json!(["fake", "Slow", []])).await;
    }
    let (mut busy, mut answered) = (0, 0);
    while answered < 9 {
        let v = b.next().await;
        if v["id"].as_u64().is_some_and(|id| (100..109).contains(&id)) {
            answered += 1;
            if v["err"] == "busy" {
                busy += 1;
            }
        }
    }
    assert_eq!(busy, 1);

    // NO_AUTO_START: a name without an owner that isn't activatable fails at once…
    let r12 = b.call(12, "WidgetCall", json!(["nobody", "Hi", []])).await;
    assert!(
        r12["err"].as_str().unwrap().contains("not running"),
        "{r12}"
    );
    // …while an activatable one, with no requires.bus_name, may be started.
    let _ = b.call(13, "WidgetCall", json!(["ghost", "Boo", []])).await;
    let started = bus.dir().join("started");
    let end = Instant::now() + Duration::from_secs(10);
    while !started.exists() && Instant::now() < end {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(started.exists(), "the activatable name was started");
}

#[tokio::test]
async fn declared_commands_run_with_checked_arguments() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    widget(r, "fake", FAKE);
    let _svc = serve_fake(&bus).await;
    let mut b = Bridge::start_with_root(&bus, &[], r);
    b.event("Hello").await;
    wait_for(&mut b, "Widgets", Duration::from_secs(5), |l| {
        visible(l, "fake") == Some(true)
    })
    .await;

    let ok = b.call(1, "WidgetRun", json!(["fake", "status", []])).await;
    assert_eq!(ok["ok"], json!({"code": 0, "output": {"ok": true}}), "{ok}");
    let said = b
        .call(2, "WidgetRun", json!(["fake", "say", ["hello"]]))
        .await;
    assert_eq!(said["ok"]["output"], "hello\n", "{said}");
    for bad in [
        json!(["-n"]),
        json!(["Hello"]),
        json!([]),
        json!(["a", "b"]),
    ] {
        let r = b.call(3, "WidgetRun", json!(["fake", "say", bad])).await;
        assert!(r["err"].is_string(), "{r}");
    }
    let r = b.call(4, "WidgetRun", json!(["fake", "rm", []])).await;
    assert!(r["err"].as_str().unwrap().contains("no command"), "{r}");
    let r = b.call(5, "WidgetRun", json!(["fake", "ask", []])).await;
    assert!(r["err"].as_str().unwrap().contains("confirmation"), "{r}");

    // One run per command at a time.
    b.send(6, "WidgetRun", json!(["fake", "nap", []])).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let r = b.call(7, "WidgetRun", json!(["fake", "nap", []])).await;
    assert_eq!(r["err"], "busy", "{r}");
    assert_eq!(b.reply(6).await["ok"]["code"], 0);
}
