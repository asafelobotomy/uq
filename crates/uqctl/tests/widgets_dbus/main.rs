//! End-to-end: widgets, app actions and activatable providers in the real `uqctl bridge`,
//! against a private bus, a fake widget service and a fake Hyprland socket (Phase 4 spec §7).
//! Never touches the user's session.

#[path = "../common/mod.rs"]
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
        .name(NAME)
        .serve_at(PATH, Fake { count: 0 })
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

mod actions;
mod calls;
mod discovery;
