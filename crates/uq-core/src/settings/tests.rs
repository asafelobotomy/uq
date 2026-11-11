use super::*;
use serde_json::json;

#[test]
fn defaults_match_spec() {
    let s = Settings::default();
    assert!(s.pinned.is_empty());
    assert_eq!(s.weather_location, None);
    assert_eq!(
        (
            s.idle.dim,
            s.idle.lock,
            s.idle.screen_off,
            s.idle.suspend_battery
        ),
        (Some(240), Some(300), Some(360), Some(1200))
    );
    assert_eq!(s.idle.locked_screen_off, 30);
    assert!(!s.idle.media_blocks_lock);
    assert_eq!(s.notif.backend, NotifBackend::Auto);
    assert!(!s.notif.show_summary);
    assert!(s.privacy.public_ip_lookup);
    assert!(s.privacy.record_launches);
    assert_eq!(s.mouse, MouseSettings::default());
    assert_eq!((s.mouse.sensitivity, s.mouse.scroll_factor), (0.0, 1.0));
    assert_eq!(s.mouse.accel_profile, AccelProfile::Default);
    assert!(!s.mouse.natural_scroll && !s.mouse.left_handed);
}

#[test]
fn rejects_unknown_key() {
    let mut s = Settings::default();
    assert_eq!(
        s.apply("theme.x", json!(1)),
        Err(SettingError::UnknownKey("theme.x".into()))
    );
}

#[test]
fn pinned_validation() {
    let mut s = Settings::default();
    let many: Vec<String> = (0..33).map(|i| format!("app{i}.desktop")).collect();
    assert!(s.apply("pinned", json!(many)).is_err());
    assert!(s.apply("pinned", json!(["../evil"])).is_err());
    assert!(s.apply("pinned", json!([1])).is_err());
    s.apply("pinned", json!(["firefox.desktop", "a", "firefox.desktop"]))
        .unwrap();
    assert_eq!(s.pinned, ["firefox.desktop", "a"]);
    let long = "a".repeat(129);
    assert!(s.apply("pinned", json!([long])).is_err());
    let ok = format!("{}.desktop", "a".repeat(128));
    assert!(s.apply("pinned", json!([ok])).is_ok());
}

#[test]
fn idle_order_enforced() {
    let mut s = Settings::default();
    assert!(s.apply("idle.lock", json!(200)).is_err());
    s.apply("idle.dim", Value::Null).unwrap();
    s.apply("idle.lock", json!(200)).unwrap();
    assert_eq!(s.idle.lock, Some(200));
    assert!(s.apply("idle.lock", json!(29)).is_err());
    assert!(s.apply("idle.lock", json!(86401)).is_err());
    assert!(s.apply("idle.lock", json!(60.5)).is_err());
    assert!(s.apply("idle.screen_off", json!(200)).is_err());
}

#[test]
fn locked_screen_off_range() {
    let mut s = Settings::default();
    assert!(s.apply("idle.locked_screen_off", json!(9)).is_err());
    assert!(s.apply("idle.locked_screen_off", json!(601)).is_err());
    s.apply("idle.locked_screen_off", json!(10)).unwrap();
    assert_eq!(s.idle.locked_screen_off, 10);
}

#[test]
fn location_validation() {
    let mut s = Settings::default();
    assert!(
        s.apply("weather_location", json!({"name":"x","lat":91,"lon":0}))
            .is_err()
    );
    assert!(
        s.apply("weather_location", json!({"name":"x","lat":0,"lon":181}))
            .is_err()
    );
    let long = "n".repeat(101);
    assert!(
        s.apply("weather_location", json!({"name":long,"lat":0,"lon":0}))
            .is_err()
    );
    s.apply(
        "weather_location",
        json!({"name":"London","lat":51.50735,"lon":-0.12776}),
    )
    .unwrap();
    let loc = s.weather_location.clone().unwrap();
    assert_eq!((loc.lat, loc.lon), (51.51, -0.13));
    s.apply("weather_location", Value::Null).unwrap();
    assert_eq!(s.weather_location, None);
}

#[test]
fn backend_enum() {
    let mut s = Settings::default();
    s.apply("notif.backend", json!("notifi")).unwrap();
    assert_eq!(s.notif.backend, NotifBackend::Notifi);
    assert_eq!(
        serde_json::to_value(s.notif.backend).unwrap(),
        json!("notifi")
    );
    assert!(s.apply("notif.backend", json!("growl")).is_err());
}

#[test]
fn bools_apply() {
    let mut s = Settings::default();
    s.apply("privacy.public_ip_lookup", json!(false)).unwrap();
    s.apply("notif.show_summary", json!(true)).unwrap();
    assert!(!s.privacy.public_ip_lookup && s.notif.show_summary);
    assert!(s.apply("privacy.record_launches", json!("yes")).is_err());
}

#[test]
fn apply_is_atomic() {
    let mut s = Settings::default();
    s.apply("pinned", json!(["a.desktop"])).unwrap();
    let before = s.clone();
    assert!(s.apply("idle.lock", json!(200)).is_err());
    assert!(s.apply("pinned", json!(["ok", "../bad"])).is_err());
    assert!(s.apply("nope", json!(1)).is_err());
    assert_eq!(s, before);
}

#[test]
fn every_setting_key_is_known() {
    for k in SETTING_KEYS {
        let mut s = Settings::default();
        let r = s.apply(k, json!("zzz"));
        assert!(!matches!(r, Err(SettingError::UnknownKey(_))), "{k}");
    }
}

#[test]
fn mouse_numbers_ranged_and_rounded() {
    let mut s = Settings::default();
    s.apply("mouse.sensitivity", json!(-0.456)).unwrap();
    assert_eq!(s.mouse.sensitivity, -0.46);
    s.apply("mouse.sensitivity", json!(1)).unwrap();
    assert_eq!(s.mouse.sensitivity, 1.0);
    for bad in [json!(1.01), json!(-1.5), json!("fast"), Value::Null] {
        assert!(s.apply("mouse.sensitivity", bad.clone()).is_err(), "{bad}");
    }
    s.apply("mouse.scroll_factor", json!(2.5)).unwrap();
    assert_eq!(s.mouse.scroll_factor, 2.5);
    assert!(s.apply("mouse.scroll_factor", json!(0.0)).is_err());
    assert!(s.apply("mouse.scroll_factor", json!(0.09)).is_err());
    assert!(s.apply("mouse.scroll_factor", json!(10.5)).is_err());
    assert_eq!(s.mouse.scroll_factor, 2.5);
}

#[test]
fn mouse_profile_and_bools() {
    let mut s = Settings::default();
    s.apply("mouse.accel_profile", json!("flat")).unwrap();
    assert_eq!(s.mouse.accel_profile, AccelProfile::Flat);
    assert_eq!(
        serde_json::to_value(s.mouse.accel_profile).unwrap(),
        json!("flat")
    );
    assert!(s.apply("mouse.accel_profile", json!("custom")).is_err());
    s.apply("mouse.natural_scroll", json!(true)).unwrap();
    s.apply("mouse.left_handed", json!(true)).unwrap();
    assert!(s.mouse.natural_scroll && s.mouse.left_handed);
    assert!(s.apply("mouse.left_handed", json!(1)).is_err());
}

#[test]
fn only_mouse_keys_render_config() {
    assert!(renders_config("mouse.sensitivity"));
    assert!(!renders_config("idle.lock"));
    assert!(!renders_config("pinned"));
}

#[test]
fn validated_keeps_mouse_and_rejects_bad_files() {
    let mut s = Settings::default();
    s.apply("mouse.sensitivity", json!(0.3)).unwrap();
    s.apply("mouse.accel_profile", json!("adaptive")).unwrap();
    assert_eq!(s.validated().0, s);
    let mut bad = s.clone();
    bad.mouse.scroll_factor = 50.0;
    bad.pinned = vec!["a.desktop".into()];
    let (fixed, errors) = bad.validated();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        fixed.mouse,
        MouseSettings::default(),
        "the bad domain resets"
    );
    assert_eq!(fixed.pinned, ["a.desktop"], "other domains are kept");
    // A state file from before mouse settings existed loads with the defaults.
    let old: Settings = serde_json::from_value(json!({"pinned": ["a"]})).unwrap();
    assert_eq!(old.mouse, MouseSettings::default());
}

#[test]
fn section_round_trip() {
    let mut s = Settings::default();
    s.set_section(
        "appearance",
        json!({"cursor_theme": "Adwaita", "kvantum": false}),
    )
    .unwrap();
    s.set_section(
        "wallpaper",
        json!({"outputs": {"*": {"image": "/a.png", "fit": "tile"}}}),
    )
    .unwrap();
    let a = s.section("appearance").unwrap();
    assert_eq!(a, json!({"cursor_theme": "Adwaita", "kvantum": false}));
    let w = s.section("wallpaper").unwrap();
    let mut t = Settings::default();
    t.set_section("appearance", a).unwrap();
    t.set_section("wallpaper", w).unwrap();
    assert_eq!(t, s);
    assert_eq!(s.validated().0, s);
    assert!(renders_section("appearance") && !renders_section("wallpaper"));
    assert!(renders_section("look") && renders_section("layouts"));
    assert_eq!(
        SECTION_DOMAINS,
        [
            "appearance",
            "wallpaper",
            "look",
            "layouts",
            "shell",
            "monitors",
            "input",
            "rules",
            "env",
            "compat",
            "autostart",
            "keybinds",
            "integration_rules"
        ]
    );
    assert!(!renders_section("monitors") && renders_section("input"));
    assert!(renders_section("rules") && renders_section("env") && renders_section("compat"));
    assert!(!renders_section("autostart"));
    assert!(renders_section("keybinds"));
}

#[test]
fn unknown_domain_rejected() {
    let mut s = Settings::default();
    assert!(s.set_section("bogus", json!({})).is_err());
    assert!(s.section("bogus").is_err());
}

#[test]
fn invalid_section_leaves_settings_unchanged() {
    let mut s = Settings::default();
    s.set_section("appearance", json!({"ui_font": "Inter"}))
        .unwrap();
    let before = s.clone();
    assert!(
        s.set_section("appearance", json!({"ui_font": "Inter", "font_size": 5}))
            .is_err()
    );
    assert!(
        s.set_section("wallpaper", json!({"outputs": {"*": {"image": "x"}}}))
            .is_err()
    );
    assert_eq!(s, before);
}

#[test]
fn look_section_round_trip() {
    let mut s = Settings::default();
    s.set_section(
        "look",
        json!({"gaps_in": 4, "blur": true, "blur_passes": 2, "animation_speed": 1.5}),
    )
    .unwrap();
    let l = s.section("look").unwrap();
    assert_eq!(
        l,
        json!({"gaps_in": 4, "blur": true, "blur_passes": 2, "animation_speed": 1.5})
    );
    let mut t = Settings::default();
    t.set_section("look", l).unwrap();
    assert_eq!(t, s);
    assert_eq!(s.validated().0, s);
    // An empty write clears every override.
    s.set_section("look", json!({})).unwrap();
    assert_eq!(s.look, Look::default());
}

#[test]
fn unknown_look_field_rejected() {
    let mut s = Settings::default();
    s.set_section("look", json!({"gaps_in": 4})).unwrap();
    let before = s.clone();
    assert!(
        s.set_section("look", json!({"gaps_in": 5, "bogus": 1}))
            .is_err()
    );
    assert_eq!(s, before);
    let bad = Settings {
        look: Look {
            blur_passes: Some(9),
            ..Look::default()
        },
        pinned: vec!["a.desktop".into()],
        ..Settings::default()
    };
    let (fixed, errors) = bad.validated();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!((fixed.look, fixed.pinned), (Look::default(), bad.pinned));
}
