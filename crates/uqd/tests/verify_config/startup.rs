//! Startup: environment, switches and the monitor plan.

use super::*;

pub(crate) fn startup_all() -> StartupRender {
    StartupRender {
        zero_scaling: true,
        env: vec![
            ("GDK_SCALE".into(), "2".into()),
            ("MOZ_ENABLE_WAYLAND".into(), "1".into()),
            ("QUOTED".into(), "\") hl.exec_cmd(\"x".into()),
        ],
        export: true,
        ..StartupRender::default()
    }
}

/// The workspace plan and the default monitor on top of the rules' own workspace rules: a
/// user rule for workspace 3 with its own fields, two screens, a sequential plan.
pub(crate) fn monitor_plan() -> StartupRender {
    use uq_core::settings::plan::Planned;
    let planned = |workspace: u8, output: &str, default: bool| Planned {
        workspace,
        output: output.into(),
        default,
    };
    StartupRender {
        monitors: MonitorRender {
            planned: vec![
                planned(1, "DP-1", true),
                planned(2, "DP-1", false),
                planned(3, "DP-2", true),
                planned(4, "DP-2", false),
            ],
            default_monitor: Some("DP-1".into()),
        },
        ..StartupRender::default()
    }
}

/// `rules()` plus a rule for workspace 3 with `gaps_in` and no monitor: the plan gives it a
/// monitor and it keeps `gaps_in`.
pub(crate) fn plan_rules() -> Rules {
    let mut r = rules();
    r.workspace.push(WorkspaceRule {
        workspace: "3".into(),
        monitor: None,
        default: None,
        persistent: None,
        gaps_in: Some(4),
        gaps_out: None,
        border_size: None,
    });
    r
}

/// Review Focus 1: a hostile variable value is one `hl.env` call with the literal value, and
/// the payload never runs as code. Runs in CI only (K4).
#[test]
pub(crate) fn hostile_env_runs_as_one_call_each() {
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
        &startup_all(),
    );
    let (p, _) = tree(&dir.path().join("env"), &gen_);
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
    // The mock prints "ok N", then one line per call and per string argument. Our three
    // variables are three `hl.env` calls with the literal values, back to back, and the payload
    // never reaches an `hl.exec_cmd`.
    assert!(text.starts_with("ok "), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let want = [
        "hl.env",
        "hl.env:GDK_SCALE",
        "hl.env:2",
        "hl.env",
        "hl.env:MOZ_ENABLE_WAYLAND",
        "hl.env:1",
        "hl.env",
        "hl.env:QUOTED",
        "hl.env:\") hl.exec_cmd(\"x",
    ];
    assert!(lines.windows(want.len()).any(|w| w == want), "{text}");
    assert_eq!(
        lines.iter().filter(|l| **l == "hl.env").count(),
        9,
        "{text}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("hl.exec_cmd:") && l.contains("\"x")),
        "{text}"
    );
    assert!(!lines.contains(&"hl.exec_cmd:x"), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
pub(crate) fn startup_switches_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &settings(),
        &startup_all(),
    );
    let (_, main) = tree(&dir.path().join("startup"), &gen_);
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

/// The plan renders one `hl.workspace_rule` per selector and the cursor table runs. Runs
/// under the Lua mock (CI, or locally with a `lua` on PATH).
#[test]
pub(crate) fn monitor_plan_runs_under_the_lua_mock() {
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
        &plan_rules(),
        &keybinds(),
        &settings(),
        &monitor_plan(),
    );
    assert!(
        gen_.contains(r#"cursor = { default_monitor = "DP-1" },"#),
        "{gen_}"
    );
    let selectors: Vec<&str> = gen_
        .lines()
        .filter(|l| l.starts_with("hl.workspace_rule("))
        .filter_map(|l| l.split("workspace = \"").nth(1)?.split('"').next())
        .collect();
    let unique: std::collections::BTreeSet<_> = selectors.iter().collect();
    assert_eq!(
        selectors.len(),
        unique.len(),
        "one call per selector: {gen_}"
    );
    for ws in ["1", "2", "3", "4"] {
        assert!(selectors.contains(&ws), "{ws} in {selectors:?}");
    }
    assert!(
        gen_.contains(r#"hl.workspace_rule({ workspace = "3", monitor = "DP-2", default = true, gaps_in = 4 })"#),
        "{gen_}"
    );
    let (p, _) = tree(&dir.path().join("monitor-plan"), &gen_);
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
    let calls = text.lines().filter(|l| *l == "hl.workspace_rule").count();
    assert_eq!(calls, selectors.len(), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
pub(crate) fn monitor_plan_passes_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &plan_rules(),
        &keybinds(),
        &settings(),
        &monitor_plan(),
    );
    let (_, main) = tree(&dir.path().join("monitor-plan"), &gen_);
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
