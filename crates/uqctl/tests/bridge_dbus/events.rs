//! Events and uqd going away and coming back.

use super::*;

#[tokio::test]
async fn section_changed_is_forwarded() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    // The typed client against the real DesktopIface, on the same private bus.
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = uq_core::client::DesktopProxy::new(&conn).await.unwrap();
    let mut section =
        serde_json::from_str::<Value>(&proxy.get_section("appearance").await.unwrap()).unwrap();
    section["font_size"] = json!(14);
    proxy
        .set_section("appearance", &section.to_string(), "settings")
        .await
        .unwrap();
    let ev = b.event("SectionChanged").await;
    assert_eq!(ev["domain"], "appearance", "{ev}");
    assert_eq!(ev["value"]["font_size"], 14, "{ev}");
    let got: Value = serde_json::from_str(&proxy.get_section("appearance").await.unwrap()).unwrap();
    assert_eq!(got["font_size"], 14);
    assert!(!proxy.get_theme().await.unwrap().1.is_empty());
}

#[tokio::test]
async fn monitors_changed_is_forwarded() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    // The shell asks for the roles through the bridge; the reply is parsed JSON.
    let outputs = b.call(1, "ListOutputs", json!([])).await;
    assert!(outputs["ok"]["live"].is_array(), "{outputs}");
    assert!(outputs["ok"]["roles"].is_object(), "{outputs}");
    assert!(b.call(2, "ListOutputs", json!([1])).await["err"].is_string());

    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .build()
        .await
        .unwrap();
    let proxy = uq_core::client::DesktopProxy::new(&conn).await.unwrap();
    proxy
        .set_section("monitors", "{}", "settings")
        .await
        .unwrap();
    // No arguments travel with the signal; the shell asks `ListOutputs` again.
    assert_eq!(b.event("MonitorsChanged").await, json!({}));
}

#[tokio::test]
async fn uqd_down_then_up_events() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    assert_eq!(b.next().await["data"]["uqd_up"], true);

    // Dropping the connection is not enough (the server's own tasks keep it alive), and
    // graceful_shutdown hangs, so give up the well-known name: uqd is off the bus as far as
    // callers can tell.
    uqd.release_name(NAME).await.unwrap();
    b.event("UqdDown").await;
    let down = b.call(1, "GetTheme", json!([])).await;
    assert_eq!(down, json!({"v":1,"id":1,"err":"uqd is not running"}));

    uqd.request_name(NAME).await.unwrap();
    b.event("UqdUp").await;
    assert!(b.call(2, "GetTheme", json!([])).await["ok"].is_array());
    // Signals keep flowing after the restart.
    b.send(3, "SetTheme", json!(["dracula", "dark"])).await;
    assert_eq!(b.event("ThemeChanged").await["name"], "dracula");
}

#[tokio::test]
async fn bridge_starts_with_uqd_down_and_exits_on_eof() {
    let bus = private_bus();
    let mut b = Bridge::start(&bus);
    assert_eq!(b.next().await["data"]["uqd_up"], false);
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    b.event("UqdUp").await;

    drop(b.stdin.take());
    let status = tokio::time::timeout(Duration::from_secs(2), b.child.wait())
        .await
        .expect("bridge exits within 2 s of stdin EOF")
        .unwrap();
    assert!(status.success());
}

/// A real restart: the first connection gives up the name and a second `Desktop` is served on a
/// NEW connection (new unique name). Signals must still reach the bridge.
#[tokio::test]
async fn signals_survive_uqd_restart_on_new_connection() {
    let bus = private_bus();
    let dir1 = tempfile::tempdir().unwrap();
    let first = serve_uqd(&bus, dir1.path()).await;
    let mut b = Bridge::start(&bus);
    assert_eq!(b.next().await["data"]["uqd_up"], true);

    first.release_name(NAME).await.unwrap();
    b.event("UqdDown").await;
    let dir2 = tempfile::tempdir().unwrap();
    let _second = serve_uqd(&bus, dir2.path()).await;
    b.event("UqdUp").await;

    assert_eq!(
        b.call(1, "SetTheme", json!(["dracula", "light"])).await["ok"],
        Value::Null
    );
    assert_eq!(b.event("ThemeChanged").await["name"], "dracula");
    let ws = b.call(2, "GetLayouts", json!([])).await["ok"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "1".into());
    let reply = b.call(3, "SetLayout", json!([ws, "scrolling"])).await;
    assert_eq!(reply["ok"], Value::Null, "{reply}");
    let ev = b.event("LayoutChanged").await;
    assert_eq!(ev["layout"], "scrolling", "{ev}");
}

/// uqd is up but doesn't answer before `Hello`: `Hello` still comes within READY_WAIT (2 s)
/// with Welcome unknown, so the shell asks again later instead of waiting 20 s.
#[tokio::test]
async fn bridge_hello_is_not_held_up_by_a_stuck_uqd() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _stuck = zbus::connection::Builder::address(bus.address.as_str())
        .name(NAME)
        .serve_at("/org/uq/Desktop1", StuckUqd)
        .build()
        .await
        .unwrap();
    let run = dir.path().join("run");
    let started = std::time::Instant::now();
    let mut b = Bridge::start_with_env(&bus, &[("XDG_RUNTIME_DIR", run.as_os_str())]);
    let hello = b.next().await;
    assert_eq!(hello["event"], "Hello", "{hello}");
    assert_eq!(hello["data"]["uqd_up"], true, "{hello}");
    assert_eq!(hello["data"]["welcome_pending"], Value::Null, "{hello}");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "Hello took {:?}",
        started.elapsed()
    );
}
