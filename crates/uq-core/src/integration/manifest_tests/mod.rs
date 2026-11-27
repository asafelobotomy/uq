use super::*;
use std::os::unix::fs::symlink;

const WIDGET: &str = r#"
uq_api = 1
id = "hauler"
name = "Downloads"
slot = "bar"
qml = "Widget.qml"
order = 50

[requires]
bus_name = "org.hauler.Hauler1"
bins = ["hauler"]

[dbus]
name = "org.hauler.Hauler1"
path = "/org/hauler/Hauler1"
interface = "org.hauler.Hauler1"
methods = ["Pause", "Resume"]
properties = ["ActiveCount", "BytesDone", "BytesTotal"]
signals = ["Finished"]

[[command]]
id = "status"
argv = ["/usr/bin/tailscale", "status", "--json"]
output = "json"
timeout_ms = 3000

[[command]]
id = "remove"
argv = ["/usr/lib/crumbs/crumbs", "remove"]
output = "none"
args = [{ name = "app", pattern = "^[A-Za-z0-9._:@+-]{1,128}$" }]
confirm = "Remove %s and its leftovers?"
"#;

fn widget_with(old: &str, new: &str) -> Result<WidgetManifest, ManifestError> {
    assert!(WIDGET.contains(old), "{old}");
    parse_widget(&WIDGET.replacen(old, new, 1))
}

const PAGE: &str = "id = \"tailscale\"\nname = \"Tailscale\"\nqml = \"Page.qml\"\n";

fn folder(root: &Path, dir: &str, manifest: &str, file: &str, text: &str) {
    let d = root.join(dir);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join(manifest), text).unwrap();
    fs::write(d.join(file), "import QtQuick\nItem {}\n").unwrap();
}

const API3_COMMANDS: &str = "[[command]]\nid = \"run\"\nargv = [\"/usr/bin/x\", \"run\"]\n\
    output = \"ndjson\"\nargs = [{ name = \"app\", pattern = \"^[a-z]+$\" }]\n\
    confirm = \"Run %s?\"\npreview = \"plan\"\n\
    [[command]]\nid = \"plan\"\nargv = [\"/usr/bin/x\", \"run\", \"--dry-run\", \"--json\"]\n\
    output = \"json\"\nargs = [{ name = \"app\", pattern = \"^[a-z]+$\" }]\n";

mod page;
mod shared;
mod widget;
