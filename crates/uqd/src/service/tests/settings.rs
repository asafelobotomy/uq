//! Settings writes.

use super::*;

#[tokio::test]
async fn set_setting_persists_and_emits() {
    let mut r = rig();
    // Defaults are 240/300/360/1200, so lock = 400 needs screen_off moved up first.
    r.desktop
        .set_setting("idle.screen_off", serde_json::json!(500), Origin::Settings)
        .await
        .unwrap();
    drain(&mut r.events);
    r.desktop
        .set_setting("idle.lock", serde_json::json!(400), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(
        State::load(&r.desktop.paths().state_file)
            .settings
            .idle
            .lock,
        Some(400)
    );
    let evs = drain(&mut r.events);
    assert_eq!(evs.len(), 1, "{evs:?}");
    let Event::SettingsChanged(s) = &evs[0] else {
        panic!("{evs:?}")
    };
    assert_eq!(s.idle.lock, Some(400));
    let json = serde_json::to_string(s).unwrap();
    assert!(!json.contains("launch_counts"), "{json}");
    // The light path never touches generated.lua or the compositor.
    assert!(!r.desktop.paths().generated_lua.exists());
    assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
}

#[tokio::test]
async fn set_setting_invalid_rejected() {
    let mut r = rig();
    let before = r.desktop.state().clone();
    let err = r
        .desktop
        .set_setting("idle.lock", serde_json::json!(1), Origin::Settings)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
    let err = r
        .desktop
        .set_setting("nope", serde_json::json!(1), Origin::Settings)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
    assert_eq!(r.desktop.state(), &before);
    assert!(drain(&mut r.events).is_empty());
    assert!(!r.desktop.paths().state_file.exists());
}

#[tokio::test]
async fn mouse_setting_renders_verifies_and_reloads() {
    let mut r = rig();
    r.desktop
        .set_setting(
            "mouse.sensitivity",
            serde_json::json!(-0.5),
            Origin::Settings,
        )
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    let generated = fs::read_to_string(&p.generated_lua).unwrap();
    assert!(generated.contains("sensitivity = -0.5,"), "{generated}");
    assert_eq!(State::load(&p.state_file).settings.mouse.sensitivity, -0.5);
    let calls = r.comp.calls();
    assert!(calls.iter().any(|c| c.starts_with("verify ")), "{calls:?}");
    assert!(calls.contains(&"reload".to_string()), "{calls:?}");
    let evs = drain(&mut r.events);
    assert_eq!(evs.len(), 1, "{evs:?}");
    let Event::SettingsChanged(s) = &evs[0] else {
        panic!("{evs:?}")
    };
    assert_eq!(s.mouse.sensitivity, -0.5);
    // Input changes never re-theme the desktop.
    assert!(r.sys.applied.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mouse_setting_rejected_by_verify_keeps_old_value() {
    let mut r = rig();
    *r.comp.verify_error.lock().unwrap() = Some("input: bad value".into());
    let err = r
        .desktop
        .set_setting(
            "mouse.left_handed",
            serde_json::json!(true),
            Origin::Settings,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert!(!r.desktop.settings().mouse.left_handed);
    assert!(!r.desktop.paths().state_file.exists());
    assert_eq!(
        drain(&mut r.events),
        vec![Event::ConfigError("input: bad value".into())]
    );
}

#[tokio::test]
async fn invalid_mouse_value_never_renders() {
    let mut r = rig();
    let err = r
        .desktop
        .set_setting(
            "mouse.scroll_factor",
            serde_json::json!(99),
            Origin::Settings,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
    assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
    assert!(!r.desktop.paths().generated_lua.exists());
}

#[tokio::test]
async fn several_settings_apply_in_an_order_that_validates() {
    let mut r = rig();
    // Defaults are dim 240 < lock 300 < screen_off 360: dim 310 alone would break the order.
    let values = vec![
        ("idle.dim".to_string(), serde_json::json!(310)),
        ("idle.lock".to_string(), serde_json::json!(320)),
    ];
    r.desktop
        .set_settings(values, Origin::Restore)
        .await
        .unwrap();
    let idle = &r.desktop.settings().idle;
    assert_eq!((idle.dim, idle.lock), (Some(310), Some(320)));
    assert_eq!(journal_of(&r).len(), 1);

    let bad = vec![("idle.dim".to_string(), serde_json::json!(999))];
    let res = r.desktop.set_settings(bad, Origin::Restore).await;
    assert!(
        matches!(res, Err(DesktopError::InvalidSetting(_))),
        "{res:?}"
    );
    assert_eq!(r.desktop.settings().idle.dim, Some(310));
}
