//! page.toml.

use super::*;

#[test]
fn page_phase3_manifest_still_parses() {
    let p = parse_page(&format!(
        "{PAGE}icon = \"network-vpn\"\ncategory = \"apps\"\ndbus_name = \"org.example.Good\"\n\
         keywords = [\"vpn\"]\n[[setting]]\nid = \"exit-node\"\nlabel = \"Exit node\"\n"
    ))
    .unwrap();
    assert_eq!(p.uq_api, 1, "a page without uq_api is read as 1");
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
fn page_api3_takes_ndjson_and_a_preview() {
    let p = parse_page(&format!("uq_api = 3\n{PAGE}{API3_COMMANDS}")).unwrap();
    assert_eq!(p.commands[0].output, Output::Ndjson);
    assert_eq!(p.commands[0].preview.as_deref(), Some("plan"));
}

#[test]
fn page_below_api3_is_refused_ndjson_and_preview() {
    for api in ["uq_api = 1\n", "uq_api = 2\n", ""] {
        let e = parse_page(&format!("{api}{PAGE}{API3_COMMANDS}")).unwrap_err();
        assert!(e.to_string().contains("needs uq_api = 3"), "{e}");
    }
}

#[test]
fn a_preview_must_be_a_matching_read() {
    let bad = |find: &str, with: &str| {
        let text = format!("uq_api = 3\n{PAGE}{}", API3_COMMANDS.replace(find, with));
        parse_page(&text).unwrap_err().to_string()
    };
    assert!(bad("preview = \"plan\"", "preview = \"nope\"").contains("not a declared command"));
    assert!(bad("preview = \"plan\"", "preview = \"run\"").contains("itself"));
    assert!(bad("confirm = \"Run %s?\"\n", "").contains("needs a confirm"));
    let diff = format!(
        "uq_api = 3\n{PAGE}{}",
        API3_COMMANDS.replacen(
            "output = \"json\"\nargs = [{ name = \"app\", pattern = \"^[a-z]+$\" }]",
            "output = \"json\"\nargs = [{ name = \"app\", pattern = \"^[a-z0-9]+$\" }]",
            1
        )
    );
    assert!(
        parse_page(&diff)
            .unwrap_err()
            .to_string()
            .contains("same args")
    );
    let text = API3_COMMANDS.replace("output = \"json\"", "output = \"text\"");
    assert!(
        parse_page(&format!("uq_api = 3\n{PAGE}{text}"))
            .unwrap_err()
            .to_string()
            .contains("output = \"json\"")
    );
}
