use super::*;
use uq_core::integration::manifest::parse_page;

fn page(extra: &str) -> Option<PageManifest> {
    Some(
        parse_page(&format!(
            "id = \"p\"\nname = \"P\"\nqml = \"Page.qml\"\n{extra}"
        ))
        .unwrap(),
    )
}

const CMD: &str = "[[command]]\nid = \"remove\"\nargv = [\"/usr/bin/crumbs\", \"rm\"]\n\
                   output = \"json\"\nargs = [{ name = \"app\", pattern = \"^[a-z.]+$\" }]\n\
                   confirm = \"Remove %s?\"\n";

#[test]
fn prepare_checks_the_declared_command_and_args() {
    let (spec, args) = prepare(page(CMD), "remove", "[\"org.maps\"]").unwrap();
    assert_eq!(
        spec.confirm_text(&args).as_deref(),
        Some("Remove org.maps?")
    );
    for (cmd, args) in [
        ("nope", "[\"a\"]"),
        ("remove", "[]"),
        ("remove", "[\"-rf\"]"),
        ("remove", "[\"A B\"]"),
        ("remove", "[\"a\", \"b\"]"),
        ("remove", "[1]"),
        ("remove", "not json"),
    ] {
        assert!(prepare(page(CMD), cmd, args).is_err(), "{cmd} {args}");
    }
    assert!(prepare(None, "remove", "[\"a\"]").is_err(), "page gone");
}

#[test]
fn dbus_targets_follow_the_manifest() {
    // Phase 3 shape: dbus_name only, the page names path and interface.
    let p3 = || page("dbus_name = \"org.x.Y\"\n");
    let r = dbus_target(
        p3(),
        "Get",
        false,
        r#"{"path": "/org/x/Y", "interface": "org.x.Y"}"#,
    )
    .unwrap();
    let t = &r.target;
    assert_eq!((t.name.as_str(), t.path.as_str()), ("org.x.Y", "/org/x/Y"));
    assert_eq!(r.bus, Bus::Session);
    assert!(r.auto_start);
    assert_eq!(r.deadline, dbus_json::CALL_DEADLINE);
    assert!(
        dbus_target(p3(), "Get", false, "").is_err(),
        "needs a target"
    );
    assert!(
        dbus_target(page(""), "Get", false, "{}").is_err(),
        "no dbus_name"
    );

    // With a [dbus] table: only its members; a different target is refused.
    let table = || {
        page(
            "[requires]\nbus_name = \"org.x.Y\"\n[dbus]\nname = \"org.x.Y\"\npath = \"/o\"\n\
             interface = \"org.x.I\"\nmethods = [\"Pause\", \"State\"]\n\
             properties = [\"Count\"]\nbus = \"system\"\ntimeout_ms = { Pause = 120000 }\n",
        )
    };
    let r = dbus_target(table(), "Pause", false, "").unwrap();
    let t = &r.target;
    assert_eq!((t.path.as_str(), t.interface.as_str()), ("/o", "org.x.I"));
    assert_eq!(r.bus, Bus::System);
    assert!(!r.auto_start, "a page waiting for its name never starts it");
    assert_eq!(r.deadline, Duration::from_secs(120), "its timeout_ms");
    assert_eq!(
        dbus_target(table(), "State", false, "").unwrap().deadline,
        dbus_json::CALL_DEADLINE
    );
    assert!(dbus_target(table(), "Count", true, "{}").is_ok());
    assert!(
        dbus_target(table(), "Count", false, "").is_err(),
        "not a method"
    );
    assert!(
        dbus_target(table(), "Pause", true, "").is_err(),
        "not a property"
    );
    assert!(dbus_target(table(), "Resume", false, "").is_err());
    assert!(
        dbus_target(table(), "Pause", false, r#"{"path": "/other"}"#).is_err(),
        "other path"
    );
    assert!(dbus_target(table(), "Pause", false, r#"{"path": "/o"}"#).is_ok());
}

#[test]
fn only_commands_with_a_watch_are_followed() {
    let p = || {
        page(
            "[[command]]\nid = \"status\"\nargv = [\"/usr/bin/x\"]\noutput = \"json\"\n\
              watch = { files = [\"/etc/x.conf\"] }\n\
              [[command]]\nid = \"plain\"\nargv = [\"/usr/bin/x\"]\noutput = \"json\"\n",
        )
    };
    assert_eq!(watch_spec(p(), "status").unwrap().files, ["/etc/x.conf"]);
    assert!(watch_spec(p(), "plain").is_err());
    assert!(watch_spec(p(), "nope").is_err());
    assert!(watch_spec(None, "status").is_err());
}

#[test]
fn open_app_takes_plain_desktop_ids_only() {
    assert_eq!(
        open_app_argv("org.noti_fi.App.desktop").unwrap(),
        ["uqctl", "launch", "org.noti_fi.App.desktop"]
    );
    for bad in [
        "",
        "firefox",
        "../x.desktop",
        "/usr/share/applications/x.desktop",
        "a/b.desktop",
        "-x.desktop",
        ".x.desktop",
        "x y.desktop",
        "x.desktop\n",
    ] {
        assert!(open_app_argv(bad).is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn launch_reports_the_exit() {
    let wait = Duration::from_secs(5);
    assert_eq!(launch(&["true".into()], wait).await, json!({"ok": true}));
    assert!(launch(&["false".into()], wait).await["error"].is_string());
    assert!(launch(&["/nonexistent/uq-launch".into()], wait).await["error"].is_string());
    // Slower than the wait: left running, reported as opened.
    let slow = ["sleep".to_string(), "2".to_string()];
    assert_eq!(
        launch(&slow, Duration::from_millis(100)).await,
        json!({"ok": true})
    );
}

#[test]
fn in_flight_budget_per_page() {
    let slots: Vec<Slot> = (0..MAX_IN_FLIGHT)
        .map(|_| Slot::take("budget").unwrap())
        .collect();
    assert!(Slot::take("budget").is_none());
    assert!(Slot::take("other-page").is_some());
    drop(slots);
    assert!(Slot::take("budget").is_some());
}
