//! App compatibility: scaling and the elsewhere fix.

use super::*;

#[tokio::test]
async fn compat_section_renders_zero_scaling_with_main_scale() {
    let mut r = rig();
    dp1(&r, 1.5);
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains("force_zero_scaling = true"), "{g}");
    assert!(g.contains(r#"hl.env("GDK_SCALE", "1", true)"#), "{g}");
    assert!(
        g.contains(r#"hl.env("STEAM_FORCE_DESKTOPUI_SCALING", "1.5", true)"#),
        "{g}"
    );
    assert_eq!(r.desktop.main_scale, Some(1.5));
    let child = r.desktop.child_env();
    assert_eq!(child[0].0, "XCURSOR_THEME");
    assert!(child.contains(&("GDK_SCALE".to_string(), "1".to_string())));
    // Scale lines never reach environment.d (F18).
    assert!(env_file(&r).is_none_or(|f| !f.contains("GDK_SCALE")));
}

#[tokio::test]
async fn compat_without_hyprland_leaves_scale_lines_out() {
    let mut r = rig();
    dp1(&r, 2.0);
    *r.comp.running.lock().unwrap() = false;
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains("force_zero_scaling = true"), "{g}");
    assert!(
        !g.contains("GDK_SCALE") && !g.contains("STEAM_FORCE"),
        "{g}"
    );
    assert_eq!(r.desktop.main_scale, None);
}

#[tokio::test]
async fn compat_elsewhere_refused_for_own_file() {
    let mut r = rig();
    put_source(&r);
    put(&override_path(&r), SOURCE);
    assert_refused(&mut r, OWN_FILE).await;
    assert_eq!(std::fs::read_to_string(override_path(&r)).unwrap(), SOURCE);
}

#[tokio::test]
async fn compat_elsewhere_refused_without_source() {
    let mut r = rig();
    assert_refused(&mut r, NO_SOURCE).await;
}

#[tokio::test]
async fn compat_elsewhere_refused_in_dev_session() {
    let mut r = rig_dev(true);
    put_source(&r);
    assert_refused(&mut r, DEV_SESSION).await;
    assert!(!files::present(&override_path(&r)));
}

#[tokio::test]
async fn undo_compat_removes_override() {
    let mut r = rig();
    put_source(&r);
    write(&mut r, "compat", ELSEWHERE).await.unwrap();
    assert!(files::is_ours(&override_path(&r), MarkerKind::Override));
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert!(r.desktop.state().compat.apps.is_empty());
    assert!(!files::present(&override_path(&r)));
}

#[tokio::test]
async fn explain_compat_sharp_x11_runtime() {
    let mut r = rig();
    r.comp
        .set_option(uq_core::compat::hypr::XWAYLAND_ZERO_SCALING, json!(false));
    write(&mut r, "compat", r#"{"sharp_x11":true}"#)
        .await
        .unwrap();
    let rows = explain(&r, "compat").await;
    assert_eq!(rows["sharp_x11"]["source"], "runtime");
    assert_eq!(rows["electron_wayland"]["source"], "uq");
}

#[tokio::test]
async fn explain_compat_sharp_x11_off_is_default() {
    let r = rig();
    let rows = explain(&r, "compat").await;
    assert_eq!(rows["sharp_x11"]["source"], "default");
    assert!(rows.get("qt_wayland").is_some());
}
