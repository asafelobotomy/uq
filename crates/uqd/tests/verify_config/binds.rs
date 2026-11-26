//! Keybinds and every theme variant.

use super::*;

/// Review Focus 1: hostile names and commands are one `hl.bind` call each after the unbind, and
/// never run as code.
#[test]
fn hostile_binds_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let hostile = [
        "x'; $(id) \"; ]] \\ \") hl.exec_cmd(\"x",
        "a'b\"c\\d]]e$(id);f",
        "''",
    ];
    // Built past validation on purpose: the renderer must hold up on its own.
    let mut kb = Keybinds::default();
    for (i, h) in hostile.iter().enumerate() {
        kb.custom.push(uq_core::keybinds::CustomBind {
            id: format!("k-{:04x}", 2 * i + 1),
            name: (*h).into(),
            combo: format!("SUPER + F{}", i + 1),
            enabled: true,
            locked: false,
            repeating: false,
            action: uq_core::keybinds::Action::Command {
                command: (*h).into(),
            },
        });
        kb.custom.push(uq_core::keybinds::CustomBind {
            id: format!("k-{:04x}", 2 * i + 2),
            name: (*h).into(),
            combo: format!("SUPER + F{}", i + 11),
            enabled: true,
            locked: false,
            repeating: false,
            action: uq_core::keybinds::Action::Launch {
                app: format!("{h}.desktop; rm -rf ~"),
            },
        });
    }
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &kb,
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("hostile-binds"), &gen_);
    let (ok, text, err) = run_mock(
        &lua,
        dir.path(),
        false,
        &[&p.base_lua, &p.generated_lua, &p.user_lua],
    );
    assert!(ok, "{text}{err}");
    let lines: Vec<&str> = text.lines().collect();
    let unbind = lines
        .iter()
        .position(|l| *l == "hl.unbind:all")
        .expect(&text);
    let after = &lines[unbind..];
    let calls = after.iter().filter(|l| **l == "hl.bind").count();
    // 58 defaults, two mouse hooks, the capture submap's placeholder bind and two binds per hostile name.
    assert_eq!(calls, 69 + 2 * hostile.len(), "{text}");
    assert_eq!(bind_lines(&text).len(), calls, "{text}");
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
    assert!(!text.contains("hl.exec_cmd:\"x"), "{text}");
    assert!(!text.contains("rm -rf ~\n"), "{text}");
}

/// Spec §2.12 for the rendered file: no launcher bind works over the lock screen, your own
/// included. The mock's own checks run over `generated.lua`'s binds, since it clears
/// `base.lua`'s at `hl.unbind("all")`; this one reads the printed list.
#[test]
fn rendered_launcher_binds_are_not_locked() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (n, kb) in fixtures().iter().enumerate() {
        let gen_ = emit_generated(
            t,
            &layouts(),
            &rules(),
            kb,
            &settings(),
            &StartupRender::default(),
        );
        let f = dir.path().join(format!("gen{n}.lua"));
        std::fs::write(&f, gen_).unwrap();
        // The generated file alone, so base.lua can't be what is being checked.
        let (ok, text, err) = run_mock(&lua, dir.path(), true, &[&f]);
        assert!(ok, "{text}{err}");
        let mut seen = 0;
        for l in bind_lines(&text) {
            let launcher = [
                "lookseedo",
                "appmenu",
                "quick-settings",
                "notif-centre",
                "lock",
                "power-menu",
            ]
            .iter()
            .any(|n| l.contains(&format!("hl.dsp.global(uq:{n})")));
            if launcher {
                seen += 1;
                assert!(!l.contains("locked=true"), "{l}");
            }
        }
        assert!(seen >= 6, "{text}");
    }
}

/// `SUPER + SUPER_R` is the one key only a default may take, so it has its own check.
#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn super_key_and_mouse_buttons_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let kb = Keybinds::from_section(
        &serde_json::json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + SUPER_R"}]}),
        &Keybinds::default(),
    )
    .unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &kb,
        &settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("modkey"), &gen_);
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

#[test]
fn lua_mock_runs_every_variant() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    for (t, kb) in reg
        .iter()
        .flat_map(|t| fixtures().into_iter().map(move |k| (t, k)))
    {
        let root = dir
            .path()
            .join(format!("{}-{}-{}", t.id, t.variant, kb.custom.len()));
        let (p, _) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
                &kb,
                &settings(),
                &StartupRender::default(),
            ),
        );
        let out = Command::new(&lua)
            .arg(&mock)
            .args([&p.base_lua, &p.generated_lua, &p.user_lua])
            .output()
            .unwrap();
        assert!(
            out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("ok "),
            "{} {}: {}{}",
            t.id,
            t.variant,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn all_variants_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (t, kb) in reg
        .iter()
        .flat_map(|t| fixtures().into_iter().map(move |k| (t, k)))
    {
        let root = dir
            .path()
            .join(format!("{}-{}-{}", t.id, t.variant, kb.custom.len()));
        let (_, main) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
                &kb,
                &settings(),
                &StartupRender::default(),
            ),
        );
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
        assert!(out.status.success(), "{} {}: {text}", t.id, t.variant);
    }
}

#[test]
fn greeter_config_has_no_binds() {
    let text = std::fs::read_to_string(repo().join("greeter/hyprland.lua")).unwrap();
    for needle in ["hl.bind", "hl.dsp", "hl.animation(", "hl.curve("] {
        assert!(!text.contains(needle), "greeter/hyprland.lua has {needle}");
    }
}
