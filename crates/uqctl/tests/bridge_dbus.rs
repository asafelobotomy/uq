//! End-to-end: the real `uqctl bridge` binary against a private dbus-daemon serving a real
//! `Desktop` with fakes. Never touches the user's session bus.

mod common;

use common::{Bridge, Bus, private_bus};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uqd::compositor::fake::FakeCompositor;
use uqd::dbus::{NAME, serve};
use uqd::service::Desktop;
use uqd::system::FakeSystem;

async fn serve_uqd(bus: &Bus, root: &std::path::Path) -> zbus::Connection {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(root);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    serve(
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
    )
    .await
    .unwrap()
    .0
}

#[tokio::test]
async fn bridge_forwards_set_theme() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    let hello = b.next().await;
    assert_eq!(
        hello,
        json!({"v":1,"event":"Hello","data":{"v":1,"uqd_up":true,"locked":false}})
    );

    // Signal and reply race (uqd may emit inside the method), so do not assume an order.
    assert_eq!(
        b.call(1, "SetTheme", json!(["dracula", "light"])).await,
        json!({"v":1,"id":1,"ok":null})
    );
    let ev = b.event("ThemeChanged").await;
    assert_eq!(ev["name"], "dracula");
    assert_eq!(ev["variant"], "light");

    assert_eq!(
        b.call(2, "GetTheme", json!([])).await["ok"],
        json!(["dracula", "light"])
    );
    // The bridge's writes are recorded as the shell's.
    let history = uq_core::paths::Paths::under(dir.path()).history_dir;
    let entries = uq_core::history::Journal::open(history).list(10);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].origin, "shell");
    let themes = b.call(3, "ListThemes", json!([])).await;
    assert_eq!(themes["ok"].as_array().unwrap().len(), 7);
    assert!(b.call(4, "GetLayouts", json!([])).await["ok"].is_object());
    let bad = b.call(5, "SetTheme", json!(["nope", "light"])).await;
    assert!(bad["err"].as_str().unwrap().contains("nope"), "{bad}");
    let bad = b.call(6, "SetTheme", json!(["dracula"])).await;
    assert!(bad["err"].is_string(), "{bad}");
}

#[tokio::test]
async fn bridge_get_schedule() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    let got = b.call(1, "GetSchedule", json!([])).await;
    assert_eq!(got["ok"]["mode"], "off", "{got}");
    let bad = b.call(2, "GetSchedule", json!(["x"])).await;
    assert!(bad["err"].is_string(), "{bad}");
}

#[tokio::test]
async fn bridge_settings_round_trip() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    // 350 s: the stages must stay increasing (default screen_off is 360 s), so not 400.
    assert_eq!(
        b.call(1, "SetSetting", json!(["idle.lock", 350])).await,
        json!({"v":1,"id":1,"ok":null})
    );
    let ev = b.event("SettingsChanged").await;
    assert_eq!(ev["idle"]["lock"], 350, "{ev}");
    let got = b.call(2, "GetSettings", json!([])).await;
    assert_eq!(got["ok"]["idle"]["lock"], 350, "{got}");
    assert!(got["ok"].get("launch_counts").is_none());

    let bad = b.call(3, "SetSetting", json!(["nope", 1])).await;
    assert!(bad["err"].is_string(), "{bad}");

    assert_eq!(
        b.call(4, "RecordLaunch", json!(["a.desktop"])).await,
        json!({"v":1,"id":4,"ok":null})
    );
    assert_eq!(
        b.event("LaunchCountsChanged").await,
        json!({"a.desktop": 1})
    );
    assert_eq!(
        b.call(5, "GetLaunchCounts", json!([])).await["ok"],
        json!({"a.desktop": 1})
    );
}

#[tokio::test]
async fn section_changed_is_forwarded() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    // The typed client against the real DesktopIface, on the same private bus.
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
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

#[tokio::test]
async fn bad_id_request_gets_error_reply() {
    let bus = private_bus();
    let mut b = Bridge::start(&bus);
    b.event("Hello").await;
    let stdin = b.stdin.as_mut().unwrap();
    stdin
        .write_all(b"{\"v\":1,\"id\":7,\"call\":5}\n{\"v\":2,\"id\":8,\"call\":\"Ping\"}\n")
        .await
        .unwrap();
    let r = b.reply(7).await;
    assert_eq!(r["id"], 7);
    assert!(r["err"].is_string());
    let r = b.reply(8).await;
    assert_eq!(r["id"], 8);
    assert!(r["err"].is_string());
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

#[tokio::test]
async fn bridge_lists_applies_and_turns_off_profiles() {
    let bus = private_bus();
    let dir = tempfile::tempdir().unwrap();
    let _uqd = serve_uqd(&bus, dir.path()).await;
    let mut b = Bridge::start(&bus);
    b.next().await;

    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = uq_core::client::DesktopProxy::new(&conn).await.unwrap();
    let gaming = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\n";
    assert_eq!(proxy.set_profile(gaming, false).await.unwrap(), "gaming");

    let list = b.call(1, "ListProfiles", json!([])).await;
    let rows = list["ok"].as_array().expect("an array");
    assert!(rows.iter().any(|p| p["id"] == "gaming"), "{list}");

    let applied = b.call(2, "ApplyProfile", json!(["gaming"])).await;
    assert_eq!(applied["ok"]["active"], "gaming", "{applied}");
    // Saving the profile sends ProfileChanged("") first; wait for the one the apply sent.
    loop {
        let ev = b.event("ProfileChanged").await;
        if ev == json!({"id": "gaming"}) {
            break;
        }
        assert_eq!(ev, json!({"id": ""}));
    }

    let off = b.call(3, "ProfileOff", json!([])).await;
    assert_eq!(off["ok"]["active"], "", "{off}");
    assert!(off["ok"]["kept"].is_array(), "{off}");

    let bad = b.call(4, "ApplyProfile", json!(["nope"])).await;
    assert!(bad["err"].is_string(), "{bad}");
}
