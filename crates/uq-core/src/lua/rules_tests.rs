use super::*;
use crate::lua::{StartupRender, emit_generated, emit_workspace_rule};
use crate::rules::{LayerMatch, Rules, WindowMatch};
use crate::settings::Settings;
use crate::theme::{Theme, Variant, tests::FIXTURE};

fn theme() -> Theme {
    Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap()
}

fn window(id: &str, matches: WindowMatch, effects: WindowEffects) -> WindowRule {
    WindowRule {
        id: id.into(),
        name: "A name".into(),
        enabled: true,
        matches,
        effects,
    }
}

fn pavucontrol() -> WindowRule {
    window(
        "r-7f2c",
        WindowMatch {
            class: Some(r"^(org\.pulseaudio\.pavucontrol)$".into()),
            ..Default::default()
        },
        WindowEffects {
            float: Some(true),
            size: Some([800, 600]),
            center: Some(true),
            ..Default::default()
        },
    )
}

fn layer(id: &str, ns: &str, effects: LayerEffects) -> LayerRule {
    LayerRule {
        id: id.into(),
        name: "Blur notifications".into(),
        enabled: true,
        matches: LayerMatch {
            namespace: ns.into(),
        },
        effects,
    }
}

#[test]
fn golden_window_rule() {
    assert_eq!(
        emit_window_rule(&pavucontrol()),
        r#"hl.window_rule({ name = "uq:r-7f2c", enabled = true, match = { class = "^(org\\.pulseaudio\\.pavucontrol)$" }, float = true, size = { 800, 600 }, center = true })"#
    );
}

#[test]
fn golden_layer_rule() {
    let r = layer(
        "r-0a1b",
        "^(noti-fi)$",
        LayerEffects {
            blur: Some(true),
            ignore_alpha: Some(0.3),
            ..Default::default()
        },
    );
    assert_eq!(
        emit_layer_rule(&r),
        r#"hl.layer_rule({ name = "uq:r-0a1b", enabled = true, match = { namespace = "^(noti-fi)$" }, blur = true, ignore_alpha = 0.3 })"#
    );
}

#[test]
fn match_names_use_lua_keys() {
    let r = window(
        "r-0001",
        WindowMatch {
            floating: Some(true),
            pinned: Some(false),
            ..Default::default()
        },
        WindowEffects {
            tile: Some(true),
            ..Default::default()
        },
    );
    let out = emit_window_rule(&r);
    assert!(
        out.contains("match = { float = true, pin = false }"),
        "{out}"
    );
}

#[test]
fn silent_workspace_and_tag_and_opacity_forms() {
    let r = window(
        "r-0002",
        WindowMatch {
            class: Some("x".into()),
            ..Default::default()
        },
        WindowEffects {
            workspace: Some("3".into()),
            silent: Some(true),
            tag: Some("web".into()),
            opacity: Some(0.9),
            ..Default::default()
        },
    );
    let out = emit_window_rule(&r);
    assert!(out.contains(r#"workspace = "3 silent""#), "{out}");
    assert!(out.contains(r#"tag = "+web""#), "{out}");
    assert!(out.contains(r#"opacity = "0.90""#), "{out}");
}

#[test]
fn disabled_rule_renders_enabled_false() {
    let mut r = pavucontrol();
    r.enabled = false;
    assert!(emit_window_rule(&r).contains(r#"name = "uq:r-7f2c", enabled = false,"#));
}

#[test]
fn rules_keep_list_order() {
    let rules = Rules {
        window: vec![
            window(
                "r-0003",
                WindowMatch {
                    class: Some("a".into()),
                    ..Default::default()
                },
                WindowEffects {
                    float: Some(true),
                    ..Default::default()
                },
            ),
            pavucontrol(),
            window(
                "r-0001",
                WindowMatch {
                    class: Some("b".into()),
                    ..Default::default()
                },
                WindowEffects {
                    float: Some(false),
                    ..Default::default()
                },
            ),
        ],
        ..Default::default()
    };
    let out = emit_generated(
        &theme(),
        &Default::default(),
        &rules,
        &Settings::default(),
        &StartupRender::default(),
    );
    let pos: Vec<usize> = ["uq:r-0003", "uq:r-7f2c", "uq:r-0001"]
        .iter()
        .map(|n| out.find(n).expect(n))
        .collect();
    assert!(pos[0] < pos[1] && pos[1] < pos[2], "{out}");
    assert!(out.find("hl.env(\"HYPRCURSOR_SIZE\"").unwrap() < pos[0]);
    assert!(out.contains("\n-- Window rules\n"), "{out}");
    assert!(!out.contains("-- Panel rules"), "{out}");
}

#[test]
fn panel_block_comes_last() {
    let rules = Rules {
        window: vec![pavucontrol()],
        layer: vec![layer(
            "r-0a1b",
            "x",
            LayerEffects {
                blur: Some(true),
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    let out = emit_generated(
        &theme(),
        &Default::default(),
        &rules,
        &Settings::default(),
        &StartupRender::default(),
    );
    assert!(out.ends_with("blur = true })\n"), "{out}");
    assert!(out.find("-- Window rules") < out.find("-- Panel rules"));
}

#[test]
fn workspace_rule_merges_layout() {
    let layouts = BTreeMap::from([
        ("3".to_string(), Layout::Master),
        ("1".to_string(), Layout::Dwindle),
    ]);
    let rules = [WorkspaceRule {
        workspace: "3".into(),
        monitor: None,
        default: None,
        persistent: Some(true),
        gaps_in: None,
        gaps_out: None,
        border_size: None,
    }];
    let out = emit_workspace_rules(&layouts, &rules);
    let lines: Vec<&str> = out.lines().filter(|l| l.starts_with("hl.")).collect();
    assert_eq!(
        lines,
        [
            r#"hl.workspace_rule({ workspace = "3", layout = "master", persistent = true })"#,
            &emit_workspace_rule("1", Layout::Dwindle),
        ]
    );
    assert!(out.starts_with("\n-- Workspace rules\n"));
    assert_eq!(emit_workspace_rules(&BTreeMap::new(), &[]), "");
}

#[test]
fn workspace_rule_without_layout_and_every_field() {
    let rules = [WorkspaceRule {
        workspace: "name:web".into(),
        monitor: Some("DP-1".into()),
        default: Some(true),
        persistent: Some(false),
        gaps_in: Some(4),
        gaps_out: Some(9),
        border_size: Some(2),
    }];
    let out = emit_workspace_rules(&BTreeMap::new(), &rules);
    assert!(
        out.contains(r#"hl.workspace_rule({ workspace = "name:web", monitor = "DP-1", default = true, persistent = false, gaps_in = 4, gaps_out = 9, border_size = 2 })"#),
        "{out}"
    );
}

/// Undo `lua_string` for one string literal at the start of `s`; returns the value and the rest.
fn unescape(s: &str) -> (String, &str) {
    let b = s.as_bytes();
    assert_eq!(b[0], b'"', "{s}");
    let mut out = Vec::new();
    let mut i = 1;
    loop {
        match b[i] {
            b'"' => return (String::from_utf8(out).unwrap(), &s[i + 1..]),
            b'\\' => {
                i += 1;
                match b[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    d if d.is_ascii_digit() => {
                        out.push(s[i..i + 3].parse::<u8>().unwrap());
                        i += 2;
                    }
                    other => panic!("bad escape {}", other as char),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
}

/// Every `key = "…"` string value in a line, unescaped, in order.
fn string_values(line: &str) -> Vec<String> {
    let mut vals = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find(" = \"") {
        let (v, after) = unescape(&rest[i + 3..]);
        vals.push(v);
        rest = after;
    }
    vals
}

#[test]
fn hostile_strings_stay_inside_lua_strings() {
    let bad = "a\"b\\c]]d\ne\r\u{1}f\") hl.exec_cmd(\"x";
    let name = "EVILNAME\") hl.exec_cmd(\"x";
    let mut w = pavucontrol();
    w.name = name.into();
    w.matches.class = Some(bad.into());
    w.matches.title = Some(bad.into());
    w.effects.monitor = Some(bad.into());
    w.effects.tag = Some(bad.into());
    let mut l = layer(
        "r-0a1b",
        bad,
        LayerEffects {
            blur: Some(true),
            ..Default::default()
        },
    );
    l.name = name.into();
    let ws = WorkspaceRule {
        workspace: bad.into(),
        monitor: Some(bad.into()),
        default: None,
        persistent: None,
        gaps_in: None,
        gaps_out: None,
        border_size: None,
    };
    let rules = Rules {
        window: vec![w],
        layer: vec![l],
        workspace: vec![ws],
    };
    let out = emit_generated(
        &theme(),
        &Default::default(),
        &rules,
        &Settings::default(),
        &StartupRender::default(),
    );
    assert!(!out.contains("EVILNAME"), "{out}");
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("_rule({")).collect();
    assert_eq!(lines.len(), 3, "{out}");
    for line in lines {
        assert!(line.starts_with("hl."), "{line}");
        assert!(line.ends_with(" })"), "{line}");
        let vals = string_values(line);
        assert!(vals.iter().any(|v| v == bad), "{line}");
        for v in &vals {
            assert!(v == bad || v.starts_with("uq:") || v.starts_with('+') || !v.contains("x\")"));
        }
    }
    // Nothing outside the generated strings carries the payload.
    for line in out.lines().filter(|l| l.contains("_rule({")) {
        let mut stripped = String::new();
        let mut rest = line;
        while let Some(i) = rest.find('"') {
            stripped.push_str(&rest[..i]);
            let (_, after) = unescape(&rest[i..]);
            rest = after;
        }
        stripped.push_str(rest);
        assert!(!stripped.contains("exec_cmd"), "{stripped}");
    }
}
