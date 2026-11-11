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

#[test]
fn widget_spec_example_parses() {
    let w = parse_widget(WIDGET).unwrap();
    assert_eq!(w.id, "hauler");
    assert_eq!(w.slot, Slot::Bar);
    assert_eq!(w.order, 50);
    assert_eq!(w.uq_api, 1);
    assert_eq!(w.requires.bins, ["hauler"]);
    let d = w.dbus.unwrap();
    assert_eq!(d.bus, Bus::Session);
    assert_eq!(d.methods, ["Pause", "Resume"]);
    assert_eq!(d.signals, ["Finished"]);
    assert_eq!(w.commands.len(), 2);
    assert_eq!(w.commands[0].output, Output::Json);
    assert_eq!(w.commands[0].timeout_ms, 3000);
    assert_eq!(w.commands[1].timeout_ms, DEFAULT_TIMEOUT_MS);
    assert_eq!(w.commands[1].args[0].name, "app");
    assert_eq!(
        w.commands[1].confirm_text(&["Maps".into()]).as_deref(),
        Some("Remove Maps and its leftovers?")
    );
}

#[test]
fn widget_minimal_and_defaults() {
    let w =
        parse_widget("uq_api = 1\nid = \"a-1\"\nname = \"A\"\nslot = \"quick\"\nqml = \"A.qml\"\n")
            .unwrap();
    assert_eq!(w.order, 50);
    assert_eq!(w.slot, Slot::Quick);
    assert!(w.dbus.is_none() && w.commands.is_empty());
    assert_eq!(w.requires, Requires::default());
}

#[test]
fn widget_rejects_every_bad_key() {
    let cases: &[(&str, &str, &str)] = &[
        ("missing uq_api", "uq_api = 1\n", ""),
        ("uq_api 0", "uq_api = 1", "uq_api = 0"),
        ("uq_api string", "uq_api = 1", "uq_api = \"1\""),
        ("bad id", "id = \"hauler\"", "id = \"Hauler\""),
        (
            "id too long",
            "id = \"hauler\"",
            &format!("id = \"{}\"", "a".repeat(41)),
        ),
        ("id underscore", "id = \"hauler\"", "id = \"a_b\""),
        ("empty name", "name = \"Downloads\"", "name = \"\""),
        (
            "control name",
            "name = \"Downloads\"",
            "name = \"a\\u0007\"",
        ),
        ("slot", "slot = \"bar\"", "slot = \"dock\""),
        (
            "qml dotdot",
            "qml = \"Widget.qml\"",
            "qml = \"../Widget.qml\"",
        ),
        (
            "qml dir",
            "qml = \"Widget.qml\"",
            "qml = \"sub/Widget.qml\"",
        ),
        (
            "qml absolute",
            "qml = \"Widget.qml\"",
            "qml = \"/tmp/W.qml\"",
        ),
        ("qml ext", "qml = \"Widget.qml\"", "qml = \"Widget.js\""),
        ("qml hidden", "qml = \"Widget.qml\"", "qml = \".qml\""),
        ("qml url", "qml = \"Widget.qml\"", "qml = \"W.qml?x=1\""),
        ("order", "order = 50", "order = 101"),
        ("order negative", "order = 50", "order = -1"),
        ("unknown top key", "order = 50", "order = 50\ncolour = 1"),
        (
            "requires unknown",
            "bins = [\"hauler\"]",
            "bins = [\"hauler\"]\nfiles = []",
        ),
        (
            "requires bus",
            "bus_name = \"org.hauler.Hauler1\"\nbins",
            "bus_name = \":1.2\"\nbins",
        ),
        (
            "requires bin path",
            "bins = [\"hauler\"]",
            "bins = [\"/usr/bin/hauler\"]",
        ),
        (
            "requires bin dotdot",
            "bins = [\"hauler\"]",
            "bins = [\"..\"]",
        ),
        (
            "dbus name",
            "name = \"org.hauler.Hauler1\"\npath",
            "name = \"nodots\"\npath",
        ),
        (
            "dbus path",
            "path = \"/org/hauler/Hauler1\"",
            "path = \"org/hauler\"",
        ),
        (
            "dbus interface",
            "interface = \"org.hauler.Hauler1\"",
            "interface = \"x\"",
        ),
        (
            "dbus member",
            "methods = [\"Pause\", \"Resume\"]",
            "methods = [\"Pa.use\"]",
        ),
        (
            "dbus dup member",
            "methods = [\"Pause\", \"Resume\"]",
            "methods = [\"Pause\", \"Pause\"]",
        ),
        (
            "dbus unknown",
            "signals = [\"Finished\"]",
            "signals = [\"Finished\"]\nobjects = []",
        ),
        (
            "dbus bus",
            "signals = [\"Finished\"]",
            "signals = [\"Finished\"]\nbus = \"user\"",
        ),
        (
            "command unknown",
            "timeout_ms = 3000",
            "timeout_ms = 3000\nshell = true",
        ),
        ("command id", "id = \"status\"", "id = \"Status!\""),
        ("command dup id", "id = \"remove\"", "id = \"status\""),
        ("argv relative", "\"/usr/bin/tailscale\"", "\"tailscale\""),
        (
            "argv outside",
            "\"/usr/bin/tailscale\"",
            "\"/opt/tailscale\"",
        ),
        (
            "argv local",
            "\"/usr/bin/tailscale\"",
            "\"/usr/local/bin/tailscale\"",
        ),
        (
            "argv dotdot",
            "\"/usr/bin/tailscale\"",
            "\"/usr/bin/../../bin/sh\"",
        ),
        ("argv prefix only", "\"/usr/bin/tailscale\"", "\"/usr/bin\""),
        (
            "argv similar prefix",
            "\"/usr/bin/tailscale\"",
            "\"/usr/bin2/x\"",
        ),
        (
            "argv empty",
            "argv = [\"/usr/bin/tailscale\", \"status\", \"--json\"]",
            "argv = []",
        ),
        ("output", "output = \"json\"", "output = \"yaml\""),
        ("output missing", "output = \"json\"\n", ""),
        ("timeout 0", "timeout_ms = 3000", "timeout_ms = 0"),
        ("timeout max", "timeout_ms = 3000", "timeout_ms = 600001"),
        (
            "pattern unanchored",
            "\"^[A-Za-z0-9._:@+-]{1,128}$\"",
            "\"[a-z]+\"",
        ),
        (
            "pattern start only",
            "\"^[A-Za-z0-9._:@+-]{1,128}$\"",
            "\"^[a-z]+\"",
        ),
        (
            "pattern bad",
            "\"^[A-Za-z0-9._:@+-]{1,128}$\"",
            "\"^[a-z$\"",
        ),
        ("arg name", "name = \"app\"", "name = \"App Name\""),
        (
            "arg unknown key",
            "pattern = \"^",
            "default = \"x\", pattern = \"^",
        ),
        (
            "confirm two %s",
            "\"Remove %s and its leftovers?\"",
            "\"Remove %s and %s?\"",
        ),
        (
            "confirm other %",
            "\"Remove %s and its leftovers?\"",
            "\"Remove %d?\"",
        ),
        (
            "confirm control",
            "\"Remove %s and its leftovers?\"",
            "\"Remove\\n%s?\"",
        ),
        ("confirm empty", "\"Remove %s and its leftovers?\"", "\"\""),
    ];
    for (why, old, new) in cases {
        let r = widget_with(old, new);
        assert!(matches!(r, Err(ManifestError::Invalid(_))), "{why}: {r:?}");
    }
    // A confirm with %s on a command without args.
    let r = widget_with(
        "timeout_ms = 3000",
        "timeout_ms = 3000\nconfirm = \"Really %s?\"",
    );
    assert!(r.is_err(), "%s without args: {r:?}");
    // Two args with the same name.
    let r = widget_with(
        "args = [{ name = \"app\", pattern = \"^[A-Za-z0-9._:@+-]{1,128}$\" }]",
        "args = [{ name = \"a\", pattern = \"^x$\" }, { name = \"a\", pattern = \"^y$\" }]",
    );
    assert!(r.is_err(), "duplicate arg: {r:?}");
}

#[test]
fn newer_uq_api_is_refused_before_strict_checks() {
    let text = "uq_api = 2\nid = \"x\"\nbrand_new_key = true\n";
    assert_eq!(parse_widget(text), Err(ManifestError::NeedsNewerUq(2)));
    assert!(
        parse_widget(text)
            .unwrap_err()
            .to_string()
            .contains("needs a newer uQ")
    );
    assert_eq!(parse_page(text), Err(ManifestError::NeedsNewerUq(2)));
}

const PAGE: &str = "id = \"tailscale\"\nname = \"Tailscale\"\nqml = \"Page.qml\"\n";

#[test]
fn page_phase3_manifest_still_parses() {
    let p = parse_page(&format!(
        "{PAGE}icon = \"network-vpn\"\ncategory = \"apps\"\ndbus_name = \"org.example.Good\"\n\
         keywords = [\"vpn\"]\n[[setting]]\nid = \"exit-node\"\nlabel = \"Exit node\"\n"
    ))
    .unwrap();
    assert_eq!(p.uq_api, UQ_API);
    assert_eq!(p.dbus_name.as_deref(), Some("org.example.Good"));
    assert!(p.dbus.is_none());
    assert_eq!(p.settings[0].label, "Exit node");
}

#[test]
fn page_takes_commands_requires_and_a_dbus_table() {
    let p = parse_page(&format!(
        "uq_api = 1\n{PAGE}[requires]\nbins = [\"tailscale\"]\n\
         [dbus]\nname = \"io.github.XZram1\"\npath = \"/io/github/XZram1\"\n\
         interface = \"io.github.XZram.Manager\"\nmethods = [\"SetSize\"]\nbus = \"system\"\n\
         [[command]]\nid = \"status\"\nargv = [\"/usr/bin/tailscale\", \"status\", \"--json\"]\n\
         output = \"json\"\n"
    ))
    .unwrap();
    assert_eq!(
        p.dbus_name.as_deref(),
        Some("io.github.XZram1"),
        "from [dbus]"
    );
    assert_eq!(p.dbus.as_ref().unwrap().bus, Bus::System);
    assert_eq!(p.commands[0].id, "status");
    assert_eq!(p.requires.bins, ["tailscale"]);

    let differ = parse_page(&format!(
        "{PAGE}dbus_name = \"org.a.B\"\n[dbus]\nname = \"org.c.D\"\npath = \"/\"\ninterface = \"org.c.D\"\n"
    ));
    assert!(differ.is_err(), "{differ:?}");
    let system_requires = parse_page(&format!(
        "{PAGE}[requires]\nbus_name = \"org.x.Other\"\n\
         [dbus]\nname = \"org.c.D\"\npath = \"/\"\ninterface = \"org.c.D\"\nbus = \"system\"\n"
    ));
    assert!(system_requires.is_err(), "{system_requires:?}");
}

#[test]
fn page_dbus_timeouts_are_for_listed_methods_only() {
    let table = |extra: &str| {
        parse_page(&format!(
            "{PAGE}[dbus]\nname = \"org.x.Y\"\npath = \"/o\"\ninterface = \"org.x.I\"\n\
             methods = [\"Pause\", \"State\"]\nbus = \"system\"\n{extra}"
        ))
    };
    let p = table("timeout_ms = { Pause = 120000 }\n").unwrap();
    let d = p.dbus.unwrap();
    assert_eq!(d.deadline("Pause"), std::time::Duration::from_secs(120));
    assert_eq!(
        d.deadline("State"),
        crate::integration::dbus_json::CALL_DEADLINE,
        "an unlisted method keeps 2 s"
    );
    for (why, extra) in [
        ("not a method", "timeout_ms = { Resume = 5000 }\n"),
        ("too long", "timeout_ms = { Pause = 120001 }\n"),
        ("zero", "timeout_ms = { Pause = 0 }\n"),
        ("not a number", "timeout_ms = { Pause = \"long\" }\n"),
    ] {
        let r = table(extra);
        assert!(r.is_err(), "{why}: {r:?}");
    }
    let w = widget_with(
        "signals = [\"Finished\"]\n",
        "signals = [\"Finished\"]\ntimeout_ms = { Pause = 5000 }\n",
    );
    assert!(w.unwrap_err().to_string().contains("pages only"));
}

#[test]
fn page_rejects_bad_keys() {
    for (why, extra) in [
        ("unknown", "surprise = 1\n"),
        ("category", "category = \"system\"\n"),
        ("icon path", "icon = \"/x.png\"\n"),
        ("dbus_name", "dbus_name = \"not a name\"\n"),
        ("keywords", "keywords = [\"\"]\n"),
        (
            "setting key",
            "[[setting]]\nid = \"a\"\nlabel = \"A\"\nx = 1\n",
        ),
        ("setting id", "[[setting]]\nid = \"a b\"\nlabel = \"A\"\n"),
        ("uq_api 0", "uq_api = 0\n"),
        ("requires key", "[requires]\nfiles = [\"x\"]\n"),
        (
            "command key",
            "[[command]]\nid = \"a\"\nargv = [\"/usr/bin/a\"]\noutput = \"none\"\nenv = {}\n",
        ),
    ] {
        let r = parse_page(&format!("{PAGE}{extra}"));
        assert!(r.is_err(), "{why}: {r:?}");
    }
    let unknown = parse_page(&format!("{PAGE}surprise = 1\n")).unwrap_err();
    assert!(unknown.to_string().contains("unknown field"), "{unknown}");
}

#[test]
fn path_under_is_lexical_and_by_component() {
    let pre = command_prefixes();
    for good in ["/usr/bin/x", "/usr/lib/x/y", "/usr/lib/a b"] {
        assert!(path_under(good, &pre), "{good}");
    }
    for bad in [
        "/usr/bin",
        "/usr/bin/",
        "/usr/bin/./x",
        "/usr/bin/../sbin/x",
        "/usr//bin/x",
        "usr/bin/x",
        "/usr/libexec/x",
        "/usr/bin/x\0",
        "",
    ] {
        assert!(!path_under(bad, &pre), "{bad:?}");
    }
}

fn folder(root: &Path, dir: &str, manifest: &str, file: &str, text: &str) {
    let d = root.join(dir);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join(manifest), text).unwrap();
    fs::write(d.join(file), "import QtQuick\nItem {}\n").unwrap();
}

#[test]
fn folders_need_matching_ids_and_contained_files() {
    let tmp = tempfile::tempdir().unwrap();
    let r = fs::canonicalize(tmp.path()).unwrap();
    let page = |id: &str| format!("id = \"{id}\"\nname = \"N\"\nqml = \"Page.qml\"\n");
    folder(&r, "good", "page.toml", "Page.qml", &page("good"));
    folder(&r, "other", "page.toml", "Page.qml", &page("good"));
    folder(&r, "noqml", "page.toml", "Other.qml", &page("noqml"));
    let outside = tempfile::tempdir().unwrap();
    folder(
        outside.path(),
        "evil",
        "page.toml",
        "Page.qml",
        &page("evil"),
    );
    symlink(outside.path().join("evil"), r.join("evil")).unwrap();
    folder(&r, "qmllink", "page.toml", "Real.qml", &page("qmllink"));
    symlink(
        outside.path().join("evil/Page.qml"),
        r.join("qmllink/Page.qml"),
    )
    .unwrap();
    folder(&r, "inner", "page.toml", "Real.qml", &page("inner"));
    symlink("Real.qml", r.join("inner/Page.qml")).unwrap();
    fs::create_dir_all(r.join("big")).unwrap();
    fs::write(r.join("big/page.toml"), "#".repeat(70 * 1024)).unwrap();

    let ok = load_page(&r.join("good"), &r).unwrap();
    assert_eq!(ok.qml_path, r.join("good/Page.qml"));
    assert_eq!(ok.dir, r.join("good"));
    let inner = load_page(&r.join("inner"), &r).unwrap();
    assert_eq!(inner.qml_path, r.join("inner/Real.qml"));
    for (dir, needle) in [
        ("other", "folder name"),
        ("noqml", "qml"),
        ("evil", "outside"),
        ("qmllink", "outside"),
        ("big", "64 KiB"),
        ("missing", "No such file"),
    ] {
        let e = load_page(&r.join(dir), &r).unwrap_err().to_string();
        assert!(e.contains(needle), "{dir}: {e}");
    }

    folder(
        &r,
        "w",
        "widget.toml",
        "Widget.qml",
        "uq_api = 1\nid = \"w\"\nname = \"W\"\nslot = \"bar\"\nqml = \"Widget.qml\"\n",
    );
    assert_eq!(load_widget(&r.join("w"), &r).unwrap().manifest.id, "w");
    assert!(load_widget(&r.join("good"), &r).is_err(), "no widget.toml");
}

#[test]
fn requires_reports_missing_bins() {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("here"), "").unwrap();
    let r = Requires {
        bus_name: None,
        bins: vec!["here".into(), "gone".into()],
    };
    assert_eq!(r.missing_bins(d.path()), ["gone"]);
}

#[test]
fn small_validators() {
    assert!(valid_id("a") && valid_id("scale-tail") && valid_id(&"a".repeat(40)));
    assert!(!valid_id("") && !valid_id("A") && !valid_id("a_b") && !valid_id(&"a".repeat(41)));
    assert!(valid_bus_name("org.a.B") && !valid_bus_name(":1.5") && !valid_bus_name("org"));
    assert!(valid_bin_name("tailscale") && valid_bin_name("g++"));
    assert!(!valid_bin_name("a/b") && !valid_bin_name(".hidden") && !valid_bin_name(""));
}
