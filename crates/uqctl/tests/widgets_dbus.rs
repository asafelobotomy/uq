//! End-to-end: widgets, app actions and activatable providers in the real `uqctl bridge`,
//! against a private bus, a fake widget service and a fake Hyprland socket (Phase 4 spec §7).
//! Never touches the user's session.

mod common;

use common::{Bridge, Bus, FakeHypr, private_bus};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;
use tokio::time::Instant;
use zbus::object_server::SignalEmitter;

const NAME: &str = "org.test.Widget1";
const PATH: &str = "/org/test/Widget1";

struct Fake {
    count: u32,
}

#[zbus::interface(name = "org.test.Widget1")]
impl Fake {
    async fn pause(&self) {}

    async fn add(&self, url: String) -> bool {
        url.starts_with("http")
    }

    async fn secret(&self) -> String {
        "nope".into()
    }

    /// Far longer than the bridge's 2 s deadline, so a loaded machine can't blur the two.
    async fn slow(&self) {
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    async fn big(&self) -> String {
        "x".repeat(300 * 1024)
    }

    #[zbus(property)]
    async fn count(&self) -> u32 {
        self.count
    }

    #[zbus(property)]
    async fn name(&self) -> String {
        "fake".into()
    }

    #[zbus(property)]
    async fn hidden(&self) -> String {
        "secret".into()
    }

    #[zbus(signal)]
    async fn finished(emitter: &SignalEmitter<'_>, id: &str, ok: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn other(emitter: &SignalEmitter<'_>, x: &str) -> zbus::Result<()>;
}

async fn serve_fake(bus: &Bus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(NAME)
        .unwrap()
        .serve_at(PATH, Fake { count: 0 })
        .unwrap()
        .build()
        .await
        .unwrap()
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn widget(root: &Path, id: &str, body: &str) {
    let dir = root.join("shell/widgets").join(id);
    put(
        &dir.join("widget.toml"),
        &format!(
            "uq_api = 1\nid = \"{id}\"\nname = \"Widget {id}\"\nqml = \"Widget.qml\"\n{}{body}",
            if body.starts_with("slot") {
                ""
            } else {
                "slot = \"bar\"\n"
            }
        ),
    );
    put(&dir.join("Widget.qml"), "import QtQuick\nItem {}\n");
}

const FAKE: &str = "order = 10\n\
[requires]\nbus_name = \"org.test.Widget1\"\n\
[dbus]\nname = \"org.test.Widget1\"\npath = \"/org/test/Widget1\"\ninterface = \"org.test.Widget1\"\n\
methods = [\"Pause\", \"Add\", \"Slow\", \"Big\"]\nproperties = [\"Count\", \"Name\"]\nsignals = [\"Finished\"]\n\
[[command]]\nid = \"status\"\nargv = [\"/usr/bin/echo\", \"{\\\"ok\\\": true}\"]\noutput = \"json\"\n\
[[command]]\nid = \"nap\"\nargv = [\"/usr/bin/sleep\", \"1\"]\noutput = \"none\"\n\
[[command]]\nid = \"say\"\nargv = [\"/usr/bin/echo\"]\noutput = \"text\"\n\
args = [{ name = \"word\", pattern = \"^[a-z]{1,10}$\" }]\n\
[[command]]\nid = \"ask\"\nargv = [\"/usr/bin/true\"]\noutput = \"none\"\nconfirm = \"Really?\"\n";

/// The next event called `name` whose data satisfies `ok`, within `within`. Earlier events of
/// other names stay queued; earlier ones of this name that don't match are dropped.
async fn wait_for(
    b: &mut Bridge,
    name: &str,
    within: Duration,
    ok: impl Fn(&Value) -> bool,
) -> Value {
    let end = Instant::now() + within;
    let mut kept = std::collections::VecDeque::new();
    let found = loop {
        let v = match b.events.pop_front() {
            Some(v) => v,
            None => tokio::time::timeout_at(end, b.next())
                .await
                .unwrap_or_else(|_| panic!("no matching {name} within {within:?}")),
        };
        if v["event"] == name {
            if ok(&v["data"]) {
                break v["data"].clone();
            }
        } else {
            kept.push_back(v);
        }
    };
    kept.append(&mut b.events);
    b.events = kept;
    found
}

/// The next event called `name`, keeping others queued.
async fn first(b: &mut Bridge, name: &str) -> Value {
    wait_for(b, name, Duration::from_secs(10), |_| true).await
}

fn find<'a>(list: &'a Value, id: &str) -> Option<&'a Value> {
    list.as_array()?.iter().find(|w| w["id"] == id)
}

fn visible(list: &Value, id: &str) -> Option<bool> {
    find(list, id).and_then(|w| w["visible"].as_bool())
}

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

fn data_dirs() -> (tempfile::TempDir, Vec<(&'static str, OsString)>) {
    let t = tempfile::tempdir().unwrap();
    let apps = t.path().join("sys/applications");
    put(
        &apps.join("org.test.App.desktop"),
        "[Desktop Entry]\nName=App\nExec=app\n",
    );
    put(
        &apps.join("term.desktop"),
        "[Desktop Entry]\nName=Term\nExec=/usr/bin/term-x\nX-TerminalArgExec=--run\n",
    );
    std::fs::create_dir_all(t.path().join("home/applications")).unwrap();
    let env = vec![
        ("XDG_DATA_HOME", t.path().join("home").into()),
        ("XDG_DATA_DIRS", t.path().join("sys").into()),
    ];
    (t, env)
}

#[tokio::test]
async fn app_actions_are_listed_and_launched_by_the_bridge() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    let actions = r.join("launcher/actions");
    put(
        &actions.join("check.appaction"),
        "[App Action]\nName=Check for update\nIcon=view-refresh\nMatch=org.test.*\n\
         Exec=/usr/bin/true check --id %d\n",
    );
    put(
        &actions.join("gone.appaction"),
        "[App Action]\nName=Gone\nExec=/usr/bin/uq-no-such-program %d\n",
    );
    let mut hypr = FakeHypr::start();
    let (_data, data) = data_dirs();
    let mut env = hypr.env();
    env.extend(data);
    let env_refs: Vec<(&str, &std::ffi::OsStr)> =
        env.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
    let mut b = Bridge::start_with_root(&bus, &env_refs, r);
    b.event("Hello").await;
    assert_eq!(
        first(&mut b, "AppActions").await,
        json!([{"id": "check", "name": "Check for update", "icon": "view-refresh",
                "match": "org.test.*", "terminal": false}])
    );

    let ok = b
        .call(
            1,
            "Launch",
            json!([{"app_action": "check", "desktop_id": "org.test.App"}]),
        )
        .await;
    assert_eq!(ok["ok"], Value::Null, "{ok}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(sent.contains("systemd-run --user --scope"), "{sent}");
    assert!(
        sent.contains("-- '/usr/bin/true' 'check' '--id' 'org.test.App.desktop'"),
        "{sent}"
    );

    for (bad, why) in [
        (
            json!({"app_action": "check", "desktop_id": "-rf"}),
            "invalid desktop id",
        ),
        (
            json!({"app_action": "check", "desktop_id": "a;b"}),
            "invalid desktop id",
        ),
        (
            json!({"app_action": "check", "desktop_id": "term"}),
            "doesn't apply",
        ),
        (
            json!({"app_action": "check", "desktop_id": "org.test.Missing"}),
            "no app called",
        ),
        (
            json!({"app_action": "gone", "desktop_id": "org.test.App"}),
            "no app action",
        ),
        (
            json!({"app_action": "check", "desktop_id": "org.test.App", "argv": ["sh"]}),
            "only",
        ),
    ] {
        let r = b.call(2, "Launch", json!([bad])).await;
        assert!(r["err"].as_str().unwrap().contains(why), "{bad}: {r}");
    }
    assert!(hypr.next(Duration::from_millis(300)).await.is_none());

    // A terminal action, installed while running, uses the chosen terminal (none set: kitty).
    put(
        &actions.join("rm.appaction"),
        "[App Action]\nName=Uninstall…\nExec=/usr/bin/true rm %d\nTerminal=true\n",
    );
    wait_for(&mut b, "AppActions", Duration::from_secs(6), |l| {
        l.as_array().is_some_and(|a| a.len() == 2)
    })
    .await;
    let ok = b
        .call(
            3,
            "Launch",
            json!([{"app_action": "rm", "desktop_id": "org.test.App.desktop"}]),
        )
        .await;
    assert_eq!(ok["ok"], Value::Null, "{ok}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(
        sent.contains("-- 'kitty' '-e' '/usr/bin/true' 'rm' 'org.test.App.desktop'"),
        "{sent}"
    );
}

/// A session service file whose Exec appends a line to `<dir>/<tag>.count` and exits without
/// taking the name (so the start fails, and is logged). The script is run by `/bin/sh`, never
/// exec'd itself: a file just written can't be exec'd while another test thread's fork still
/// holds it open for writing (ETXTBSY).
fn counting_service(bus: &Bus, name: &str, tag: &str) {
    let script = bus.dir().join(format!("{tag}.sh"));
    std::fs::write(
        &script,
        format!(
            "echo x >> {}\n",
            bus.dir().join(format!("{tag}.count")).display()
        ),
    )
    .unwrap();
    std::fs::write(
        bus.dir().join("s").join(format!("{tag}.service")),
        format!(
            "[D-BUS Service]\nName={name}\nExec=/bin/sh {}\n",
            script.display()
        ),
    )
    .unwrap();
}

fn starts(bus: &Bus, tag: &str) -> usize {
    std::fs::read_to_string(bus.dir().join(format!("{tag}.count")))
        .map(|t| t.lines().count())
        .unwrap_or(0)
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
        .unwrap()
        .name("org.test.Warm")
        .unwrap()
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
