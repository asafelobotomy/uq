//! Gestures under the Lua mock and Hyprland.

use super::*;

/// One gesture per action, every direction, modifiers, a special workspace name and every swipe
/// option: all on different triggers, so none shadows another.
pub(crate) fn gesture_settings() -> Settings {
    let list: Vec<Gesture> = serde_json::from_value(serde_json::json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
        {"fingers": 3, "direction": "vertical", "action": "workspace", "modifier": "ALT",
         "enabled": true},
        {"fingers": 4, "direction": "left", "action": "move", "enabled": true},
        {"fingers": 4, "direction": "right", "action": "resize", "enabled": true},
        {"fingers": 4, "direction": "up", "action": "special", "workspace": "scratch",
         "enabled": true},
        {"fingers": 5, "direction": "down", "action": "close", "enabled": true},
        {"fingers": 5, "direction": "pinch_in", "action": "fullscreen", "enabled": true},
        {"fingers": 5, "direction": "pinch_out", "action": "float", "enabled": true},
        {"fingers": 3, "direction": "up", "action": "uq:lookseedo", "modifier": "SUPER",
         "enabled": true},
        {"fingers": 4, "direction": "down", "action": "uq:quick-settings",
         "modifier": "SUPER SHIFT", "enabled": true},
        {"fingers": 3, "direction": "down", "action": "special", "modifier": "CTRL SHIFT",
         "enabled": true},
        {"fingers": 5, "direction": "left", "action": "close", "enabled": false},
    ]))
    .unwrap();
    gestures::validate(&list).expect("the fixture gestures don't clash");
    let mut s = settings();
    s.input.gestures = Some(list);
    s.input.swipe = gestures::SwipeOptions {
        distance: Some(450),
        invert: Some(false),
        create_new: Some(false),
        cancel_ratio: Some(0.35),
    };
    s
}

/// Gestures render and run under the mock, whose `hl.gesture` checks every key, the direction
/// spelling and the action type.
#[test]
pub(crate) fn gestures_run_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &gesture_settings(),
        &StartupRender::default(),
    );
    assert!(
        gen_.contains("gestures = { workspace_swipe_distance = 450"),
        "{gen_}"
    );
    let (p, _) = tree(&dir.path().join("gestures"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && text.starts_with("ok "),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Eleven enabled gestures (the twelfth is off), and nothing from base.lua.
    assert_eq!(
        text.lines().filter(|l| *l == "hl.gesture").count(),
        11,
        "{text}"
    );
    // The shell actions run their function: a dispatch of the global shortcut.
    assert!(text.contains("hl.dsp.global:uq:lookseedo"), "{text}");
    assert!(text.contains("hl.dsp.global:uq:quick-settings"), "{text}");
}

/// The first gesture the person's defaults make also runs: uQ's own two, from a never-set
/// list, pass the same mock.
#[test]
pub(crate) fn default_gestures_run_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("default-gestures"), &gen_);
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
    assert_eq!(
        text.lines().filter(|l| *l == "hl.gesture").count(),
        2,
        "{text}"
    );
}

/// F3: at Hyprland 0.57 a bare `hl.dsp.global(…)` is a dispatcher object `hl.gesture` refuses,
/// so the mock refuses it too; a misspelt field or direction is refused as well.
#[test]
pub(crate) fn mock_rejects_bad_gestures() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("gesture.lua");
    for (text, want) in [
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = hl.dsp.global(\"uq:lookseedo\") })\n",
            "dispatcher object",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = \"close\", modifier = \"SUPER\" })\n",
            "unknown hl.gesture field",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = \"close\", [\"workspace-name\"] = \"x\" })\n",
            "unknown hl.gesture field",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"pinch_in\", action = \"close\" })\n",
            "unknown hl.gesture direction",
        ),
        (
            "hl.gesture({ fingers = 1, direction = \"up\", action = \"close\" })\n",
            "fingers",
        ),
    ] {
        std::fs::write(&f, text).unwrap();
        let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
        assert!(!ok && err.contains(want), "{text}: {out}{err}");
    }
    // The shape uQ writes is accepted.
    std::fs::write(
        &f,
        "hl.gesture({ fingers = 4, direction = \"up\", action = function() hl.dispatch(hl.dsp.global(\"uq:lookseedo\")) end })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(ok, "{out}{err}");
}

/// R20: `base.lua` has no gesture of its own; a second copy would shadow uQ's.
#[test]
pub(crate) fn base_lua_has_no_gesture() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    let code: Vec<&str> = base
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect();
    assert!(!code.iter().any(|l| l.contains("hl.gesture")), "{code:?}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
pub(crate) fn gestures_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &gesture_settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("gestures"), &gen_);
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
