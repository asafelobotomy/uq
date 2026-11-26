//! Startup and keeping a layout.

use super::*;

#[tokio::test]
async fn startup_runs_the_matcher_once() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    // What `main` does after serving: the first match.
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 1.5);
    assert_eq!(count(&r, "verify"), 1);
    // And nothing again for the same screens.
    r.desktop.outputs_changed(false).await;
    assert_eq!(count(&r, "verify"), 1);
}

#[tokio::test]
async fn kept_layout_survives_same_set_event() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    // The matcher has handled this set (it applied "Desk"); the user then keeps another layout.
    r.desktop.outputs_changed(true).await;
    keep(&mut r, r#"{"outputs":{"DP-1":{"scale":2.0}}}"#).await;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(true).await;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 2.0);
    assert_eq!(count(&r, "verify"), 0);
    // An undo of that Keep is also left alone by the burst it causes.
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    let kept = scale_of(&r);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), kept);
}

#[tokio::test]
async fn startup_keeps_covering_layout() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    keep(&mut r, ALL3).await;
    r.desktop.hotplug.last_set = None;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 2.0);
    assert_eq!(count(&r, "verify"), 0);
}

#[tokio::test]
async fn startup_with_a_new_set_switches() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 1.5);
}

#[tokio::test]
async fn connector_swap_is_noticed_without_switching() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    r.desktop.outputs_changed(false).await;
    drain(&mut r.events);
    // The same two screens, each now on the other's connector.
    let mut all: Vec<Value> = serde_json::from_str(&screens(&["eDP-1", "DP-1"])).unwrap();
    all[0]["name"] = "DP-1".into();
    all[1]["name"] = "eDP-1".into();
    *r.comp.monitors_json.lock().unwrap() = serde_json::to_string(&all).unwrap();
    r.desktop.outputs_changed(false).await;
    assert!(
        drain(&mut r.events)
            .iter()
            .any(|e| matches!(e, Event::MonitorsChanged))
    );
}

#[tokio::test]
async fn keep_saves_the_layout_only() {
    let mut r = rig().await;
    let (token, _) = r
        .desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    // Something else in the section changes during the window (a restore, say).
    r.desktop.state.settings.monitors.auto_profiles = false;
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(scale_of(&r), 1.5);
    assert!(!r.desktop.state.settings.monitors.auto_profiles);
}

#[tokio::test]
async fn undo_of_a_main_display_change_tells_x11() {
    let mut r = rig().await;
    set(
        &mut r,
        json!({"outputs": {}, "primary": serde_json::from_str::<Value>(DELL).unwrap()}),
    )
    .await;
    let mut section = stored(&r);
    section["primary"] = json!({"make":"BOE","model":"0x0BCA","serial":""});
    set(&mut r, section).await;
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1", "eDP-1"]);
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1", "eDP-1", "DP-1"]);
}

#[tokio::test]
async fn a_later_successful_switch_clears_the_failure() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    *r.comp.verify_error.lock().unwrap() = Some("monitor line is wrong".into());
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["arrangement"]["failed"], true);
    *r.comp.verify_error.lock().unwrap() = None;
    show(&r, &["eDP-1", "DP-1"]);
    r.desktop.outputs_changed(false).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), 1.5);
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_ne!(v["arrangement"]["failed"], true, "{v}");
}
