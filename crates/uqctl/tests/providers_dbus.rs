//! End-to-end: the real `uqctl bridge` against a private bus with fake launcher providers.

mod common;

use common::{Bridge, Bus, private_bus};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Calls = Arc<Mutex<Vec<(String, String)>>>;

/// A fake provider: the title echoes the query; a query starting with `slow` sleeps `delay`.
struct Fake {
    tag: &'static str,
    delay: Duration,
    calls: Calls,
    searches: Calls,
}

#[zbus::interface(name = "org.uq.Launcher1.Provider")]
impl Fake {
    async fn search(&self, query: String) -> Vec<(String, String, String, String, String, f64)> {
        self.searches
            .lock()
            .unwrap()
            .push((query.clone(), String::new()));
        if query.starts_with("slow") {
            tokio::time::sleep(self.delay).await;
        }
        vec![(
            "id1".into(),
            format!("{}:{query}", self.tag),
            "sub".into(),
            "http://evil/x.png".into(),
            "[\"open\"]".into(),
            3.0,
        )]
    }

    async fn activate(&self, id: String, action: String) {
        self.calls.lock().unwrap().push((id, action));
    }
}

async fn serve_fake(
    bus: &Bus,
    name: &'static str,
    tag: &'static str,
    delay: Duration,
) -> (zbus::Connection, Calls) {
    let (conn, calls, _) = serve_fake_logged(bus, name, tag, delay).await;
    (conn, calls)
}

/// Like `serve_fake`, also returning every `Search` query the provider received.
async fn serve_fake_logged(
    bus: &Bus,
    name: &'static str,
    tag: &'static str,
    delay: Duration,
) -> (zbus::Connection, Calls, Calls) {
    let calls = Calls::default();
    let searches = Calls::default();
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(name)
        .unwrap()
        .serve_at(
            "/org/uq/Launcher1",
            Fake {
                tag,
                delay,
                calls: calls.clone(),
                searches: searches.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    (conn, calls, searches)
}

fn provider_file(data: &Path, file: &str, bus_name: &str, name: &str) {
    let dir = data.join("uq/launcher");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(file),
        format!("[Launcher Provider]\nBusName={bus_name}\nName={name}\n"),
    )
    .unwrap();
}

fn start(bus: &Bus, data: &Path) -> Bridge {
    Bridge::start_with_env(bus, &[("XDG_DATA_HOME", data.as_os_str())])
}

/// Collect `SearchResults` events for `dur`.
async fn collect(b: &mut Bridge, dur: Duration) -> Vec<Value> {
    let mut out = Vec::new();
    // Events queued while a reply was awaited come first.
    while let Some(v) = b.events.pop_front() {
        if v["event"] == "SearchResults" {
            out.push(v["data"].clone());
        }
    }
    let end = tokio::time::Instant::now() + dur;
    while let Ok(v) = tokio::time::timeout_at(end, b.next()).await {
        if v["event"] == "SearchResults" {
            out.push(v["data"].clone());
        }
    }
    out
}

#[tokio::test]
async fn search_fans_out_and_tags_provider() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.Fake", "Fakery");
    let (_c, _) = serve_fake(&bus, "org.test.Fake", "f", Duration::ZERO).await;
    let mut b = start(&bus, data.path());
    b.event("Hello").await;

    // Built-in and external results; the query is trimmed first so " =2+2" reaches calc.
    assert_eq!(
        b.call(1, "Search", json!([" =2+2", 1])).await["ok"],
        Value::Null
    );
    let got = collect(&mut b, Duration::from_millis(400)).await;
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["provider"], "calc");
    assert_eq!(got[0]["provider_name"], "Calculator");
    assert_eq!(got[0]["source_dir"], "builtin");
    assert_eq!(got[0]["items"][0]["title"], "4");

    b.send(2, "Search", json!(["hello", 2])).await;
    let got = collect(&mut b, Duration::from_millis(500)).await;
    assert_eq!(got.len(), 1, "{got:?}");
    let ev = &got[0];
    assert_eq!(ev["generation"], 2);
    assert_eq!(ev["provider"], "org.test.Fake");
    assert_eq!(ev["provider_name"], "Fakery");
    assert_eq!(
        ev["source_dir"],
        data.path().join("uq/launcher").to_str().unwrap()
    );
    let it = &ev["items"][0];
    assert_eq!(it["title"], "f:hello");
    assert_eq!(it["icon"], "system-search");
    assert!((it["score"].as_f64().unwrap() - 0.9).abs() < 1e-9);
    assert_eq!(it["actions"], json!(["open"]));

    // Gated out: one character, and ">" / "=" queries.
    for (i, q) in ["h", "> ls", "=1+1"].iter().enumerate() {
        b.send(10 + i as u64, "Search", json!([q, 3 + i])).await;
    }
    let got = collect(&mut b, Duration::from_millis(500)).await;
    assert!(
        got.iter().all(|e| e["provider"] != "org.test.Fake"),
        "{got:?}"
    );
    assert!(got.iter().any(|e| e["provider"] == "run"), "{got:?}");
}

#[tokio::test]
async fn slow_provider_dropped_after_300ms() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.Fake", "Fakery");
    let (_c, _) = serve_fake(&bus, "org.test.Fake", "f", Duration::from_millis(500)).await;
    let mut b = start(&bus, data.path());
    b.event("Hello").await;
    b.send(1, "Search", json!(["slow one", 1])).await;
    let got = collect(&mut b, Duration::from_secs(1)).await;
    assert!(got.is_empty(), "{got:?}");
}

#[tokio::test]
async fn stale_generation_results_dropped() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.Fake", "Fakery");
    let (_c, _) = serve_fake(&bus, "org.test.Fake", "f", Duration::from_millis(200)).await;
    let mut b = start(&bus, data.path());
    b.event("Hello").await;
    b.send(1, "Search", json!(["slow one", 1])).await;
    b.send(2, "Search", json!(["fast two", 2])).await;
    let got = collect(&mut b, Duration::from_millis(800)).await;
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["generation"], 2);
    assert_eq!(got[0]["items"][0]["title"], "f:fast two");
}

#[tokio::test]
async fn unowned_provider_not_activated() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.Ghost", "Ghost");
    let marker = bus.dir().join("started");
    let mut b = start(&bus, data.path());
    b.event("Hello").await;
    b.send(1, "Search", json!(["hello", 1])).await;
    let got = collect(&mut b, Duration::from_millis(600)).await;
    assert!(got.is_empty(), "{got:?}");
    let r = b
        .call(2, "Activate", json!(["org.test.Ghost", "x", "open"]))
        .await;
    let _ = r;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!marker.exists(), "service was started");

    // Control: the service file does start the service when asked to.
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let dbus = zbus::fdo::DBusProxy::new(&conn).await.unwrap();
    // The service never claims the name, so the bus would wait 25 s; only the start matters.
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        dbus.start_service_by_name("org.test.Ghost".try_into().unwrap(), 0),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(marker.exists(), "activation service file did not work");
}

#[tokio::test]
async fn activate_routes_to_provider() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.Fake", "Fakery");
    let (_c, calls) = serve_fake(&bus, "org.test.Fake", "f", Duration::ZERO).await;
    let mut b = start(&bus, data.path());
    b.event("Hello").await;

    let r = b
        .call(1, "Activate", json!(["org.test.Fake", "id1", "open"]))
        .await;
    assert_eq!(r["ok"], Value::Null, "{r}");
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("id1".to_string(), "open".to_string())]
    );

    // Unknown provider, oversize id: rejected, never forwarded.
    let r = b
        .call(2, "Activate", json!(["org.test.Other", "id1", "open"]))
        .await;
    assert!(r["err"].is_string());
    let r = b
        .call(
            3,
            "Activate",
            json!(["org.test.Fake", "i".repeat(513), "open"]),
        )
        .await;
    assert!(r["err"].is_string());
    assert_eq!(calls.lock().unwrap().len(), 1);

    // calc copies the result of the latest calc query; run needs a live id.
    b.send(4, "Search", json!(["=3*3", 5])).await;
    b.reply(4).await;
    assert_eq!(
        b.call(5, "Activate", json!(["calc", "calc", "copy"])).await["ok"],
        json!({"copy": "9"})
    );
    assert!(
        b.call(6, "Activate", json!(["calc", "other", "copy"]))
            .await["err"]
            .is_string()
    );
    b.send(7, "Search", json!(["plain", 6])).await;
    b.reply(7).await;
    assert!(b.call(8, "Activate", json!(["calc", "calc", "copy"])).await["err"].is_string());
    assert!(b.call(9, "Activate", json!(["run", "run-6", "run"])).await["err"].is_string());
}

/// A real uqd on the private bus, so the bridge learns the settings.
async fn serve_uqd(bus: &Bus, root: &Path) -> zbus::Connection {
    use std::path::PathBuf;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = uq_core::paths::Paths::under(root);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = uqd::service::Desktop::new(
        paths,
        Arc::new(uqd::compositor::fake::FakeCompositor::running()),
        Arc::new(uqd::system::FakeSystem::default()),
    )
    .unwrap();
    uqd::dbus::serve(
        zbus::connection::Builder::address(bus.address.as_str()).unwrap(),
        desktop,
    )
    .await
    .unwrap()
    .0
}

#[tokio::test]
async fn disabled_builtin_and_external_are_not_asked() {
    let bus = private_bus();
    let data = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    provider_file(data.path(), "a.provider", "org.test.P", "Pee");
    provider_file(data.path(), "b.provider", "org.test.Q", "Queue");
    let (_p, _, p_searches) = serve_fake_logged(&bus, "org.test.P", "p", Duration::ZERO).await;
    let (_q, _, q_searches) = serve_fake_logged(&bus, "org.test.Q", "q", Duration::ZERO).await;
    let _uqd = serve_uqd(&bus, state.path()).await;
    let mut b = start(&bus, data.path());
    b.event("Hello").await;

    // Control: with nothing switched off, all three answer.
    let mut got = Vec::new();
    for (i, q) in ["=2+2", "hello"].iter().enumerate() {
        b.send(1 + i as u64, "Search", json!([q, 1 + i])).await;
        got.extend(collect(&mut b, Duration::from_millis(500)).await);
    }
    let who: Vec<&str> = got.iter().filter_map(|e| e["provider"].as_str()).collect();
    assert!(who.contains(&"calc"), "{got:?}");
    assert!(
        who.contains(&"org.test.P") && who.contains(&"org.test.Q"),
        "{got:?}"
    );

    // Switch calc and P off through the typed client; the bridge relays SettingsChanged.
    let conn = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = uq_core::client::DesktopProxy::new(&conn).await.unwrap();
    let mut shell: Value =
        serde_json::from_str(&proxy.get_section("shell").await.unwrap()).unwrap();
    shell["providers"] = json!([
        {"id": "calc", "enabled": false},
        {"id": "org.test.P", "enabled": false}
    ]);
    proxy
        .set_section("shell", &shell.to_string())
        .await
        .unwrap();
    b.event("SettingsChanged").await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    p_searches.lock().unwrap().clear();
    q_searches.lock().unwrap().clear();
    // One at a time: a newer generation makes the older one's answers stale.
    let mut got = Vec::new();
    for (i, q) in ["=2+2", "hello", "> ls"].iter().enumerate() {
        b.send(3 + i as u64, "Search", json!([q, 3 + i])).await;
        got.extend(collect(&mut b, Duration::from_millis(500)).await);
    }
    let who: Vec<&str> = got.iter().filter_map(|e| e["provider"].as_str()).collect();
    assert!(!who.contains(&"calc"), "{got:?}");
    assert!(!who.contains(&"org.test.P"), "{got:?}");
    assert!(who.contains(&"org.test.Q"), "{got:?}");
    assert!(who.contains(&"run"), "{got:?}");
    assert!(p_searches.lock().unwrap().is_empty(), "P was asked");
    assert!(!q_searches.lock().unwrap().is_empty(), "Q was not asked");
}
