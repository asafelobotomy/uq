//! Workspace rules and rules from apps.

use super::*;

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

#[test]
fn rules_from_apps_go_between_workspace_and_window_rules() {
    use crate::integration::rules::parse_rules;
    let text = r#"
[[window_rule]]
id = "romforge.emulators"
class = ["^(retroarch|x\"y)$", "^b\\\\$"]
title = "]] hl.exec_cmd(\"x\") --"
workspace = "name:game"
fullscreen = true
"#;
    let app = parse_rules(text, "romforge").unwrap().remove(0);
    let app_rules = app.window_rules();
    let rules = Rules {
        window: vec![pavucontrol()],
        workspace: vec![WorkspaceRule {
            workspace: "4".into(),
            monitor: None,
            default: None,
            persistent: Some(true),
            gaps_in: None,
            gaps_out: None,
            border_size: None,
        }],
        ..Default::default()
    };
    let out = crate::lua::emit_generated_with_apps(
        &theme(),
        &Default::default(),
        &rules,
        &app_rules,
        &crate::keybinds::Keybinds::default(),
        &Settings::default(),
        &StartupRender::default(),
    );
    let ws = out.find("-- Workspace rules").unwrap();
    let apps = out.find("\n-- Rules from apps\n").expect("app block");
    let own = out.find("-- Window rules").unwrap();
    assert!(ws < apps && apps < own, "{out}");
    let block: Vec<&str> = out[apps..own]
        .lines()
        .filter(|l| l.starts_with("hl."))
        .collect();
    assert_eq!(block.len(), 2, "one call per class pattern: {out}");
    assert_eq!(
        block[0],
        r#"hl.window_rule({ name = "uq:app:romforge.emulators.1", enabled = true, match = { class = "^(retroarch|x\"y)$", title = "]] hl.exec_cmd(\"x\") --" }, fullscreen = true, workspace = "name:game" })"#
    );
    assert!(block[1].contains(r#"class = "^b\\\\$""#), "{}", block[1]);
    // With no app rules, the output is exactly emit_generated's.
    let plain = emit_generated(
        &theme(),
        &Default::default(),
        &rules,
        &crate::keybinds::Keybinds::default(),
        &Settings::default(),
        &StartupRender::default(),
    );
    assert!(!plain.contains("Rules from apps"));
    assert_eq!(
        plain,
        crate::lua::emit_generated_with_apps(
            &theme(),
            &Default::default(),
            &rules,
            &[],
            &crate::keybinds::Keybinds::default(),
            &Settings::default(),
            &StartupRender::default(),
        )
    );
}

#[test]
fn planned_rules_merge() {
    use crate::lua::merge_planned;
    use crate::settings::plan::Planned;
    let rule = |ws: &str| WorkspaceRule {
        workspace: ws.into(),
        monitor: None,
        default: None,
        persistent: None,
        gaps_in: None,
        gaps_out: None,
        border_size: None,
    };
    let user = [
        WorkspaceRule {
            monitor: Some("HDMI-A-1".into()),
            ..rule("3")
        },
        WorkspaceRule {
            gaps_in: Some(7),
            ..rule("4")
        },
    ];
    let plan = |ws: u8, out: &str, default: bool| Planned {
        workspace: ws,
        output: out.into(),
        default,
    };
    let planned = [
        plan(1, "DP-1", true),
        plan(3, "DP-1", false),
        plan(4, "DP-2", false),
        plan(6, "DP-2", true),
    ];
    let merged = merge_planned(&user, &planned);
    type Row<'a> = (&'a str, Option<&'a str>, Option<bool>, Option<u32>);
    let got: Vec<Row> = merged
        .iter()
        .map(|r| {
            (
                r.workspace.as_str(),
                r.monitor.as_deref(),
                r.default,
                r.gaps_in,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("3", Some("HDMI-A-1"), None, None),
            ("4", Some("DP-2"), None, Some(7)),
            ("1", Some("DP-1"), Some(true), None),
            ("6", Some("DP-2"), Some(true), None),
        ]
    );
    // One call per selector, in that order.
    let out = emit_workspace_rules(&BTreeMap::new(), &merged);
    assert_eq!(out.matches("hl.workspace_rule").count(), 4, "{out}");
    // No plan, no change.
    assert_eq!(merge_planned(&user, &[]), user);
}
