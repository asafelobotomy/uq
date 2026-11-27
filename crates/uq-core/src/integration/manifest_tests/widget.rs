//! widget.toml.

use super::*;

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
fn widgets_may_not_use_ndjson_or_preview() {
    let text = WIDGET.replace("uq_api = 1", "uq_api = 3") + "\n" + API3_COMMANDS;
    let e = parse_widget(&text).unwrap_err();
    assert!(e.to_string().contains("settings pages only"), "{e}");
}
