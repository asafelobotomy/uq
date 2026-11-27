//! Monitors and the main scale.

use super::*;

#[tokio::test]
async fn main_scale_follows_primary() {
    let mut r = rig();
    two_screens(&r);
    write(&mut r, "compat", r#"{"sharp_x11":true}"#)
        .await
        .unwrap();
    assert_eq!(r.desktop.main_scale, Some(1.25), "unset: the screen at 0,0");
    r.desktop.state.settings.monitors.primary = Some(id("B"));
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    assert_eq!(r.desktop.main_scale, Some(1.5));
    let g = generated(&r);
    assert!(
        g.contains(r#"hl.env("STEAM_FORCE_DESKTOPUI_SCALING", "1.5", true)"#),
        "{g}"
    );
    assert!(
        g.contains(r#"cursor = { default_monitor = "DP-2" },"#),
        "{g}"
    );
}

#[tokio::test]
async fn render_reads_monitors_once() {
    let mut r = rig();
    two_screens(&r);
    let m = &mut r.desktop.state.settings.monitors;
    m.primary = Some(id("B"));
    m.workspaces = WorkspacePlan {
        strategy: Strategy::Sequential,
        count: 4,
        ..WorkspacePlan::default()
    };
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    assert_eq!(monitor_reads(&r), 1, "{:?}", r.comp.calls());
    let g = generated(&r);
    assert!(
        g.contains(r#"hl.workspace_rule({ workspace = "1", monitor = "DP-1", default = true })"#),
        "{g}"
    );
    assert!(
        g.contains(r#"hl.workspace_rule({ workspace = "3", monitor = "DP-2", default = true })"#),
        "{g}"
    );
}

#[tokio::test]
async fn render_skips_monitors_when_unused() {
    let mut r = rig();
    two_screens(&r);
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#)
        .await
        .unwrap();
    assert_eq!(monitor_reads(&r), 0, "{:?}", r.comp.calls());
    let g = generated(&r);
    assert!(
        !g.contains("cursor =") && !g.contains("workspace_rule"),
        "{g}"
    );
    // The planner alone is enough to read the screens, once.
    r.desktop.state.settings.monitors.workspaces.strategy = Strategy::Interleave;
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"baz"}]}"#)
        .await
        .unwrap();
    assert_eq!(monitor_reads(&r), 1);
}

#[tokio::test]
async fn stored_monitor_scale_wins() {
    let mut r = rig();
    dp1(&r, 1.5);
    r.desktop.state.settings.monitors.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            scale: Some(2.0),
            ..OutputConfig::default()
        },
    );
    write(&mut r, "compat", r#"{"sharp_x11":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("GDK_SCALE", "2", true)"#), "{g}");
}
