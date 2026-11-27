//! uQ's base.lua: its binds and settings.

use super::*;

/// Spec §2.12: the launcher binds never work over the lock screen.
#[test]
fn base_lua_launcher_binds_are_not_locked() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for name in [
        "uq:lookseedo",
        "uq:emoji",
        "uq:appmenu",
        "uq:quick-settings",
        "uq:notif-centre",
        "uq:lock",
        "uq:power-menu",
    ] {
        let lines: Vec<&str> = base.lines().filter(|l| l.contains(name)).collect();
        assert!(!lines.is_empty(), "base.lua has no {name} bind");
        for line in lines {
            assert!(
                !line.contains("locked"),
                "{name} bind must not be locked: {line}"
            );
        }
    }
}

/// Spec §2.12: the OSD keys go through the shell, locked (level keys repeating, mute toggles not),
/// with no exec binds left.
#[test]
fn base_lua_osd_binds_are_locked() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for (key, name) in [
        ("XF86AudioRaiseVolume", "uq:vol-up"),
        ("XF86AudioLowerVolume", "uq:vol-down"),
        ("XF86AudioMute", "uq:mute"),
        ("XF86AudioMicMute", "uq:mic-mute"),
        ("XF86MonBrightnessUp", "uq:bright-up"),
        ("XF86MonBrightnessDown", "uq:bright-down"),
    ] {
        let lines: Vec<&str> = base.lines().filter(|l| l.contains(name)).collect();
        assert_eq!(lines.len(), 1, "expected exactly one {name} bind");
        let line = lines[0];
        assert!(line.contains(key), "{name} is not on {key}: {line}");
        assert!(
            line.contains("locked = true"),
            "{name} bind must be locked: {line}"
        );
        // Level keys repeat while held; the mute toggles must not, or holding one flaps it.
        let toggle = name.ends_with("mute");
        assert_eq!(
            line.contains("repeating = true"),
            !toggle,
            "{name} bind: repeating must be {}: {line}",
            !toggle
        );
    }
    for line in base.lines() {
        assert!(
            !(line.contains("wpctl") || line.contains("brightnessctl")),
            "exec bind left in base.lua: {line}"
        );
    }
}

#[test]
fn base_lua_has_no_animation_lines() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for needle in ["hl.animation(", "hl.curve("] {
        assert!(!base.contains(needle), "base.lua still has {needle}");
    }
}

#[test]
fn base_lua_enables_session_lock_restore() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for needle in [
        "allow_session_lock_restore = true",
        "session_lock_xray",
        "key_press_enables_dpms",
        "mouse_move_enables_dpms",
    ] {
        assert!(base.contains(needle), "base.lua lacks {needle}");
    }
}

/// R1, the spec's migration test, permanent: `base.lua`'s fallback copies are exactly what
/// uQ's default table renders. Both are run through the mock; the `(key, dispatcher, options)`
/// lists must be equal, in order.
#[test]
fn base_lua_binds_equal_the_default_table() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let rendered = dir.path().join("rendered.lua");
    std::fs::write(&rendered, uq_core::lua::emit_binds(&Keybinds::default())).unwrap();
    let base = repo().join("config/hypr/base.lua");
    let (ok, from_base, err) = run_mock(&lua, dir.path(), true, &[&base]);
    assert!(ok, "{from_base}{err}");
    let (ok, from_table, err) = run_mock(&lua, dir.path(), true, &[&rendered]);
    assert!(ok, "{from_table}{err}");
    // The capture submap's own bind (it only leaves the submap) isn't one of the shortcuts.
    let (a, b): (Vec<&str>, Vec<&str>) = (
        bind_lines(&from_base),
        bind_lines(&from_table)
            .into_iter()
            .filter(|l| !l.contains("XF86Launch9"))
            .collect(),
    );
    assert_eq!(a.len(), 68, "{from_base}");
    assert_eq!(a, b, "base.lua and the default table have drifted apart");
}
