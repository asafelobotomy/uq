//! Window, layer and app rules.

use super::*;

/// Window rules from apps (Phase 4 §3.3): romforge's packaged rule, plus one with several
/// patterns and a hostile title, as uqd renders them.
pub(crate) fn app_rules() -> Vec<WindowRule> {
    let text = r#"
[[window_rule]]
id = "romforge.emulators"
class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
workspace = "name:game"
fullscreen = true

[[window_rule]]
id = "romforge.tools"
class = ["^romforge$", "^romforge-cli$"]
title = "\"]] hl.exec_cmd(\"x\") --"
workspace = 7
float = true
"#;
    uq_core::integration::rules::parse_rules(text, "romforge")
        .unwrap()
        .iter()
        .flat_map(|r| r.window_rules())
        .collect()
}

/// F19: each hostile rule is exactly one call, and the payload never runs as code.
#[test]
pub(crate) fn hostile_rules_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let hostile = hostile_rules();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &hostile,
        &keybinds(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("hostile"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let count = |name: &str| text.lines().filter(|l| *l == name).count();
    assert_eq!(count("hl.window_rule"), hostile.window.len(), "{text}");
    // base.lua has layer rules of its own (the shell's popups animate themselves).
    let base_layer_rules = std::fs::read_to_string(repo().join("config/hypr/base.lua"))
        .unwrap()
        .matches("hl.layer_rule(")
        .count();
    assert_eq!(
        count("hl.layer_rule"),
        hostile.layer.len() + base_layer_rules,
        "{text}"
    );
    // base.lua's own hl.exec_cmd calls are expected; the payload is not.
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
    assert!(!text.contains("hl.exec_cmd:\"x"), "{text}");
}

/// The app rules are one `hl.window_rule` call each, and the title never runs as code.
#[test]
pub(crate) fn app_rules_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let apps = app_rules();
    assert_eq!(apps.len(), 3);
    let gen_ = emit_generated_with_apps(
        t,
        &layouts(),
        &rules(),
        &apps,
        &Keybinds::default(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("apps"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let count = |name: &str| text.lines().filter(|l| *l == name).count();
    assert_eq!(count("hl.window_rule"), rules().window.len() + 3, "{text}");
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
pub(crate) fn app_rules_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated_with_apps(
        t,
        &layouts(),
        &rules(),
        &app_rules(),
        &Keybinds::default(),
        &settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("apps"), &gen_);
    let out = Command::new("Hyprland")
        .arg("--verify-config")
        .arg("-c")
        .arg(&main)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
}
