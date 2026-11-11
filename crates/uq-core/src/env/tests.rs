use super::*;
use crate::state::State;
use serde_json::json;

const BLOCKED_REASON: &str = "uQ needs this one to run your session, so it can't be changed here";
const APPEARANCE_REASON: &str = "This is set by the Appearance page.";

fn refused(name: &str, value: &str) -> String {
    check_var("env.vars[0]", name, value)
        .expect_err("should be refused")
        .to_string()
}

#[test]
fn names_follow_the_rule() {
    for ok in ["A", "_X", "GDK_SCALE2", "GDK_BACKEND"] {
        assert!(check_name("k", ok).is_ok(), "{ok}");
    }
    let long = "A".repeat(129);
    for bad in ["a", "1A", "A-B", "A=B", "", "A B", "é", long.as_str()] {
        assert!(check_name("k", bad).is_err(), "{bad:?}");
    }
    assert!(check_name("k", &"A".repeat(128)).is_ok());
}

#[test]
fn blocked_names_refused() {
    for name in [
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "XDG_SESSION_ID",
        "HYPRLAND_X",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_CURRENT_DESKTOP",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "XDG_SEAT",
        "XDG_VTNR",
        "UQ_DEV_SESSION",
        "UQ_X",
        "HOME",
        "USER",
        "LOGNAME",
    ] {
        assert_eq!(blocked(name), Some(BLOCKED_REASON), "{name}");
        let msg = refused(name, "x");
        assert!(msg.contains(BLOCKED_REASON), "{name}: {msg}");
    }
    for name in [
        "XCURSOR_THEME",
        "XCURSOR_SIZE",
        "HYPRCURSOR_SIZE",
        "QT_QPA_PLATFORMTHEME",
    ] {
        assert_eq!(blocked(name), Some(APPEARANCE_REASON), "{name}");
        assert!(refused(name, "x").contains(APPEARANCE_REASON), "{name}");
    }
    for name in [
        "GDK_BACKEND",
        "XDG_SESSIONX",
        "HYPRLAND",
        "MYUQ_X",
        "XDG_DATA_DIRS",
    ] {
        assert_eq!(blocked(name), None, "{name}");
    }
}

#[test]
fn warned_names_allowed() {
    for name in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "PATH",
        "XDG_DATA_DIRS",
        "XDG_CONFIG_DIRS",
    ] {
        assert_eq!(
            check_var("k", name, "/x"),
            Ok(Some(Warn::BreaksApps)),
            "{name}"
        );
    }
    assert_eq!(check_var("k", "GDK_BACKEND", "x11"), Ok(None));
}

#[test]
fn values_checked() {
    assert!(check_var("k", "A", &"x".repeat(MAX_VALUE_BYTES)).is_ok());
    assert!(check_var("k", "A", &"x".repeat(MAX_VALUE_BYTES + 1)).is_err());
    // Bytes, not characters.
    assert!(check_var("k", "A", &"é".repeat(MAX_VALUE_BYTES / 2 + 1)).is_err());
    for bad in [
        "a\nb", "a\rb", "a\0b", "a\tb", "a\u{7f}b", "a\u{1b}b", "a$b", "$",
    ] {
        assert!(check_var("k", "A", bad).is_err(), "{bad:?}");
    }
    assert!(refused("A", "$HOME").contains("can't contain $"));
    assert_eq!(check_var("k", "A", ""), Ok(None));
}

#[test]
fn hostile_values_are_data() {
    // The value is only data; the Lua renderer keeps it inside its string.
    assert_eq!(check_var("k", "A", "\") hl.exec_cmd(\"x"), Ok(None));
    assert!(check_var("k", "A", "'; rm -rf ~ #").is_ok());
}

#[test]
fn duplicates_and_limit_refused() {
    let dup = json!({"vars": [{"name": "A", "value": "1"}, {"name": "A", "value": "2"}]});
    let e = Env::from_section(&dup).unwrap_err().to_string();
    assert!(e.contains("env.vars[1]") && e.contains("already"), "{e}");

    let many = |n: usize| {
        json!({"vars": (0..n)
            .map(|i| json!({"name": format!("V{i}"), "value": ""}))
            .collect::<Vec<_>>()})
    };
    assert_eq!(Env::from_section(&many(MAX_VARS)).unwrap().vars.len(), 100);
    let e = Env::from_section(&many(MAX_VARS + 1)).unwrap_err();
    assert!(e.to_string().contains("at most 100"), "{e}");
}

#[test]
fn section_shape_is_strict() {
    for bad in [
        json!([]),
        json!({"vars": "x"}),
        json!({"vars": [1]}),
        json!({"vars": [{"name": "A"}]}),
        json!({"vars": [{"name": "A", "value": 1}]}),
        json!({"vars": [{"name": "A", "value": "", "extra": 1}]}),
        json!({"vars": [], "other": 1}),
    ] {
        assert!(Env::from_section(&bad).is_err(), "{bad}");
    }
    assert_eq!(Env::from_section(&json!({})).unwrap(), Env::default());
}

#[test]
fn lenient_load_keeps_good_entries() {
    let src = r#"
[[env.vars]]
name = "GDK_BACKEND"
value = "x11"

[[env.vars]]
name = "bad name"
value = "x"

[[env.vars]]
name = "WAYLAND_DISPLAY"
value = "x"

[[env.vars]]
name = "EDITOR"
value = "nvim"
"#;
    let st = State::parse_strict(src).unwrap();
    let names: Vec<_> = st.env.vars.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["GDK_BACKEND", "EDITOR"]);
    // A wrong-shaped entry or list never fails the load.
    let st = State::parse_strict("[env]\nvars = 3\n").unwrap();
    assert_eq!(st.env, Env::default());
    let st =
        State::parse_strict("[[env.vars]]\nname = 1\n\n[[env.vars]]\nname = \"A\"\nvalue = \"\"\n")
            .unwrap();
    assert_eq!(st.env.vars.len(), 1);
}

#[test]
fn repaired_drops_duplicates_and_extras() {
    let var = |n: &str| Var {
        name: n.into(),
        value: "v".into(),
    };
    let mut vars = vec![var("A"), var("A"), var("a")];
    vars.extend((0..150).map(|i| var(&format!("V{i}"))));
    let env = Env { vars }.repaired();
    assert_eq!(env.vars.len(), MAX_VARS);
    assert_eq!(env.vars[0].name, "A");
    assert_eq!(env.vars[1].name, "V0");
    assert_eq!(env.clone().repaired(), env);
}

#[test]
fn section_round_trips() {
    let env = Env {
        vars: vec![
            Var {
                name: "GDK_BACKEND".into(),
                value: "x11".into(),
            },
            Var {
                name: "EMPTY".into(),
                value: String::new(),
            },
            Var {
                name: "MIXED".into(),
                value: "it's \"q\" `x` back\\slash #é".into(),
            },
        ],
    };
    assert_eq!(Env::from_section(&env.section()).unwrap(), env);
    assert_eq!(
        env.section(),
        json!({"vars": [
            {"name": "GDK_BACKEND", "value": "x11"},
            {"name": "EMPTY", "value": ""},
            {"name": "MIXED", "value": "it's \"q\" `x` back\\slash #é"},
        ]})
    );

    let mut st = State::default();
    st.set_section("env", env.section()).unwrap();
    assert_eq!(st.section("env").unwrap(), env.section());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.toml");
    st.save(&path).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("[[env.vars]]")
    );
    assert_eq!(State::load(&path), st);
    assert_eq!(State::load(&path).env, env);
}

#[test]
fn failed_set_section_leaves_state_alone() {
    let mut st = State::default();
    st.set_section("env", json!({"vars": [{"name": "A", "value": "1"}]}))
        .unwrap();
    let before = st.clone();
    assert!(
        st.set_section("env", json!({"vars": [{"name": "DISPLAY", "value": "1"}]}))
            .is_err()
    );
    assert_eq!(st, before);
}

#[test]
fn get_and_warnings() {
    let env = Env::from_section(&json!({"vars": [
        {"name": "A", "value": "1"},
        {"name": "LD_PRELOAD", "value": "/x.so"},
    ]}))
    .unwrap();
    assert_eq!(env.get("A"), Some("1"));
    assert_eq!(env.get("B"), None);
    assert_eq!(
        env.warnings(),
        [("LD_PRELOAD".to_string(), Warn::BreaksApps)]
    );
}

/// A reference reader for the two quoting forms, after systemd's `env-file.c`: single quotes
/// are literal; in double quotes a backslash before `"`, `\`, `` ` `` or `$` takes the character
/// and any other backslash stays.
fn parse_env_file(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (name, rest) = line.split_once('=').expect("NAME=value");
        let mut chars = rest.chars();
        let mut value = String::new();
        match chars.next() {
            Some('\'') => {
                let body: String = chars.collect();
                value = body.strip_suffix('\'').expect("closing quote").to_string();
                assert!(!value.contains('\''), "single quote inside single quotes");
            }
            Some('"') => {
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            assert!(chars.next().is_none(), "text after closing quote");
                        }
                        '\\' => match chars.next().expect("escape target") {
                            e @ ('"' | '\\' | '`' | '$') => value.push(e),
                            e => {
                                value.push('\\');
                                value.push(e);
                            }
                        },
                        c => value.push(c),
                    }
                }
                assert!(closed, "closing quote");
            }
            other => panic!("unquoted value starting {other:?}"),
        }
        out.push((name.to_string(), value));
    }
    out
}

#[test]
fn environment_d_round_trips() {
    let values = [
        "a b",
        "it's",
        "\"q\"",
        "`x`",
        "back\\slash",
        "#x",
        "é",
        "",
        "it's \"q\" `x` \\ \\\" end\\",
        "'",
        "\\'",
    ];
    let lines: Vec<(String, String)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("V{i}"), v.to_string()))
        .collect();
    let text = environment_d(&lines);
    assert!(text.starts_with(
        "# Written by uQ (Startup page). Changes here are replaced; use uQ Settings.\n"
    ));
    assert!(text.ends_with('\n'));
    assert_eq!(parse_env_file(&text), lines);
    assert!(text.contains("V0='a b'\n"));
    assert!(text.contains("V1=\"it's\"\n"));
    assert_eq!(
        environment_d(&[]),
        "# Written by uQ (Startup page). Changes here are replaced; use uQ Settings.\n"
    );
}

#[test]
fn refusals_are_plain() {
    let long = "A".repeat(129);
    let cases: Vec<(&str, String)> = vec![
        ("a", "x".into()),
        ("1A", "x".into()),
        ("", "x".into()),
        (long.as_str(), "x".into()),
        ("DISPLAY", "x".into()),
        ("XCURSOR_SIZE", "x".into()),
        ("A", "a\nb".into()),
        ("A", "a$b".into()),
        ("A", "x".repeat(MAX_VALUE_BYTES + 1)),
    ];
    let mut msgs: Vec<String> = cases
        .iter()
        .map(|(n, v)| check_var("env.vars[0]", n, v).unwrap_err().to_string())
        .collect();
    msgs.push(Env::from_section(&json!([])).unwrap_err().to_string());
    msgs.push(
        Env::from_section(&json!({"vars": [{"name": "A"}]}))
            .unwrap_err()
            .to_string(),
    );
    for m in msgs {
        for bad in ["regex", "[A-Z", "state.toml", "^", "$]"] {
            assert!(!m.contains(bad), "{m:?} contains {bad:?}");
        }
    }
}
