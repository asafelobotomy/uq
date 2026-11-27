//! Window and layer rules.

use super::*;

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
        &crate::keybinds::Keybinds::default(),
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
        &crate::keybinds::Keybinds::default(),
        &Settings::default(),
        &StartupRender::default(),
    );
    // The shortcuts block comes after the rules blocks (R1).
    let binds = out.find("-- Keyboard shortcuts").unwrap();
    assert!(out[..binds].ends_with("blur = true })\n\n"), "{out}");
    assert!(out.find("-- Window rules") < out.find("-- Panel rules"));
    assert!(out.find("-- Panel rules") < Some(binds));
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
        &crate::keybinds::Keybinds::default(),
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
