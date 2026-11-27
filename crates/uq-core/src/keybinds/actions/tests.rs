use super::*;
use crate::keybinds::hypr::DISPATCHERS;

/// Every shell action says what it does in its own words (none falls through to another's).
#[test]
fn every_shell_action_has_its_own_text() {
    let texts: Vec<String> = SHELL_NAMES
        .iter()
        .map(|n| Action::Shell { name: (*n).into() }.text())
        .collect();
    for (i, t) in texts.iter().enumerate() {
        assert!(!texts[..i].contains(t), "{} repeats {t:?}", SHELL_NAMES[i]);
    }
}

fn all() -> Vec<Action> {
    let s = |n: &str| Action::Shell { name: n.into() };
    let mut v: Vec<Action> = SHELL_NAMES.iter().map(|n| s(n)).collect();
    for n in ["close", "float", "pin"] {
        v.push(Action::Window {
            name: n.into(),
            direction: None,
        });
    }
    for d in DIRECTIONS {
        v.push(Action::Window {
            name: "focus".into(),
            direction: Some(d.into()),
        });
    }
    for n in ["focus", "move"] {
        v.push(Action::Workspace {
            name: n.into(),
            workspace: Some(10),
        });
    }
    for n in ["toggle_special", "move_special", "next", "prev"] {
        v.push(Action::Workspace {
            name: n.into(),
            workspace: None,
        });
    }
    for n in ["terminal", "exit", "profile-next", "settings"] {
        v.push(Action::Uq {
            name: n.into(),
            layout: None,
            media: None,
        });
    }
    for l in LAYOUTS {
        v.push(Action::Uq {
            name: "layout".into(),
            layout: Some(l.into()),
            media: None,
        });
    }
    for m in MEDIA {
        v.push(Action::Uq {
            name: "media".into(),
            layout: None,
            media: Some(m.into()),
        });
    }
    v.push(Action::Launch {
        app: "org.mozilla.firefox".into(),
    });
    v.push(Action::Command {
        command: "echo hi".into(),
    });
    v
}

#[test]
fn every_action_renders_a_known_dispatcher() {
    for a in all() {
        let a = a.check().unwrap_or_else(|e| panic!("{a:?}: {e}"));
        let lua = a.lua();
        let rest = lua
            .strip_prefix("hl.dsp.")
            .unwrap_or_else(|| panic!("{lua}"));
        let name = rest.split('(').next().unwrap();
        assert!(DISPATCHERS.contains(&name), "{lua}");
        assert!(!lua.contains('\n'));
        assert!(!a.text().is_empty());
    }
}

#[test]
fn shell_quote_round_trips() {
    if std::process::Command::new("sh")
        .arg("-c")
        .arg("true")
        .status()
        .is_err()
    {
        eprintln!("skipped: no sh");
        return;
    }
    let long = "x'y\"z\\$ ".repeat(128);
    for s in ["a'b", "''", "$(x)", "\"", "\\", long.as_str(), ""] {
        // The shell parses `printf %s <quoted>`; the one word it gets must be `s` itself.
        let via = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf %s {}", shell_quote(s)))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&via.stdout), s);
    }
}

#[test]
fn launch_renders_uqctl_launch() {
    let a = Action::Launch {
        app: "org.mozilla.firefox".into(),
    }
    .check()
    .unwrap();
    assert_eq!(
        a.lua(),
        "hl.dsp.exec_cmd(\"uqctl launch 'org.mozilla.firefox.desktop'\")"
    );
    assert_eq!(a.text(), "Starts Firefox");
}

#[test]
fn command_renders_a_scope() {
    let a = Action::Command {
        command: "grim -g \"$(slurp)\"".into(),
    };
    assert_eq!(
        a.lua(),
        "hl.dsp.exec_cmd(\"systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c 'grim -g \\\"$(slurp)\\\"'\")"
    );
    assert_eq!(a.text(), "Runs a command");
}

#[test]
fn closed_values() {
    let json = |s: &str| serde_json::from_str::<Action>(s);
    for bad in [
        r#"{"kind":"shell","name":"nope"}"#,
        r#"{"kind":"workspace","name":"focus","workspace":11}"#,
        r#"{"kind":"workspace","name":"focus"}"#,
        r#"{"kind":"window","name":"focus","direction":"diag"}"#,
        r#"{"kind":"uq","name":"layout","layout":"monocle"}"#,
        r#"{"kind":"launch","app":"x.desktop; rm -rf ~"}"#,
        r#"{"kind":"command","command":""}"#,
        r#"{"kind":"command","command":"a\nb"}"#,
    ] {
        assert!(json(bad).unwrap().check().is_err(), "{bad}");
    }
    assert!(
        json(r#"{"kind":"command","command":"x"}"#)
            .unwrap()
            .check()
            .is_ok()
    );
    assert!(json(r#"{"kind":"fly","name":"x"}"#).is_err());
    assert!(json(r#"{"kind":"shell","name":"lock","extra":1}"#).is_err());
    let long = Action::Command {
        command: "a".repeat(1025),
    };
    assert!(long.check().is_err());
    assert!(
        Action::Command {
            command: "a".repeat(1024)
        }
        .check()
        .is_ok()
    );
}

#[test]
fn lock_and_repeat_limits() {
    for a in all() {
        let osd = matches!(&a, Action::Shell { name } if OSD_NAMES.contains(&name.as_str()));
        let media = matches!(&a, Action::Uq { name, .. } if name == "media");
        assert_eq!(a.may_lock(), osd || media, "{a:?}");
        // Only things that start something are refused; the uQ exec actions start one.
        let starts = matches!(a, Action::Launch { .. } | Action::Command { .. })
            || matches!(&a, Action::Uq { name, .. } if name != "media");
        assert_eq!(a.may_repeat(), !starts, "{a:?}");
    }
}
