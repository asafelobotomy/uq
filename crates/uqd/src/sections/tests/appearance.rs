//! Appearance, wallpaper and shell sections.

use super::*;

#[tokio::test]
async fn set_appearance_renders_overrides_and_sets_cursor() {
    let mut r = rig();
    r.desktop
        .set_section(
            "appearance",
            r#"{"ui_font":"Cantarell","cursor_theme":"Adwaita","cursor_size":32}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let tokens = std::fs::read_to_string(r.desktop.paths().theme_dir.join("tokens.json")).unwrap();
    assert!(tokens.contains("Cantarell"), "{tokens}");
    assert!(r.comp.calls().contains(&"setcursor Adwaita 32".to_string()));
    assert!(kvconfig(&r).is_file());
    assert_eq!(
        State::load(&r.desktop.paths().state_file)
            .settings
            .appearance
            .ui_font
            .as_deref(),
        Some("Cantarell")
    );
    let evs = drain(&mut r.events);
    assert!(evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "appearance"))
    );
}

#[tokio::test]
async fn appearance_commit_survives_gsettings_failure() {
    let mut r = rig();
    r.sys.fail.store(true, Ordering::SeqCst);
    r.desktop
        .set_section("appearance", r#"{"mono_font":"Hack"}"#, Origin::Settings)
        .await
        .unwrap();
    let got: serde_json::Value =
        serde_json::from_str(&r.desktop.get_section("appearance").unwrap()).unwrap();
    assert_eq!(got["mono_font"], "Hack");
    assert!(r.sys.applied.lock().unwrap().is_empty());
}

#[tokio::test]
async fn kvantum_off_removes_the_kvconfig() {
    let mut r = rig();
    r.desktop
        .set_section("appearance", "{}", Origin::Settings)
        .await
        .unwrap();
    assert!(kvconfig(&r).is_file());
    r.desktop
        .set_section("appearance", r#"{"kvantum":false}"#, Origin::Settings)
        .await
        .unwrap();
    assert!(!kvconfig(&r).exists());
}

#[tokio::test]
async fn set_wallpaper_saves_and_emits_without_render() {
    let mut r = rig();
    r.desktop.startup().await;
    let gen_path = r.desktop.paths().generated_lua.clone();
    let before = std::fs::metadata(&gen_path).unwrap().modified().unwrap();
    r.comp.calls.lock().unwrap().clear();
    drain(&mut r.events);
    let json = r#"{"outputs":{"*":{"image":"/a.png"}}}"#;
    r.desktop
        .set_section("wallpaper", json, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(&gen_path).unwrap().modified().unwrap(),
        before
    );
    assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
    let evs = drain(&mut r.events);
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "wallpaper"))
    );
    assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
    assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
    let saved = State::load(&r.desktop.paths().state_file);
    assert!(saved.settings.wallpaper.outputs.contains_key("*"));
}

#[tokio::test]
async fn set_shell_saves_and_emits_without_render() {
    let mut r = rig();
    r.desktop.startup().await;
    let gen_path = r.desktop.paths().generated_lua.clone();
    let before = std::fs::metadata(&gen_path).unwrap().modified().unwrap();
    r.comp.calls.lock().unwrap().clear();
    drain(&mut r.events);
    let json = r#"{"islands":{"work":false},"clock_24h":false,"terminal":"foot.desktop","providers":[{"id":"run"},{"id":"org.x.Y","enabled":false}]}"#;
    r.desktop
        .set_section("shell", json, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(&gen_path).unwrap().modified().unwrap(),
        before
    );
    assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
    let evs = drain(&mut r.events);
    assert!(
        evs.iter()
            .any(|e| matches!(e, Event::SectionChanged { domain, .. } if domain == "shell"))
    );
    assert!(evs.iter().any(|e| matches!(e, Event::SettingsChanged(_))));
    assert!(!evs.iter().any(|e| matches!(e, Event::ThemeChanged { .. })));
    let saved = State::load(&r.desktop.paths().state_file);
    let sh = &saved.settings.shell;
    assert!(!sh.islands.work && !sh.clock_24h && sh.islands.time);
    assert_eq!(sh.terminal.as_deref(), Some("foot.desktop"));
    assert_eq!(sh.providers.len(), 2);
    let got: serde_json::Value =
        serde_json::from_str(&r.desktop.get_section("shell").unwrap()).unwrap();
    assert_eq!(got["providers"][1]["enabled"], false);
    for bad in [r#"{"terminal":"kitty"}"#, r#"{"nope":1}"#] {
        assert!(matches!(
            r.desktop.set_section("shell", bad, Origin::Settings).await,
            Err(DesktopError::InvalidSetting(_))
        ));
    }
}

#[tokio::test]
async fn invalid_section_is_rejected_and_state_unchanged() {
    let mut r = rig();
    let before = r.desktop.get_section("appearance").unwrap();
    for (d, j) in [
        ("appearance", r#"{"font_size":5}"#),
        ("appearance", "not json"),
        ("wallpaper", r#"{"outputs":{"*":{"image":"../x"}}}"#),
        ("nope", "{}"),
    ] {
        assert!(
            matches!(
                r.desktop.set_section(d, j, Origin::Settings).await,
                Err(DesktopError::InvalidSetting(_))
            ),
            "{d} {j}"
        );
    }
    assert_eq!(r.desktop.get_section("appearance").unwrap(), before);
    assert!(r.desktop.get_section("nope").is_err());
    assert!(drain(&mut r.events).is_empty());
}

#[tokio::test]
async fn rejected_config_keeps_previous_appearance() {
    let mut r = rig();
    r.desktop
        .set_section("appearance", r#"{"ui_font":"Inter"}"#, Origin::Settings)
        .await
        .unwrap();
    let before = r.desktop.get_section("appearance").unwrap();
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("bad".into());
    let res = r
        .desktop
        .set_section("appearance", r#"{"ui_font":"Other"}"#, Origin::Settings)
        .await;
    assert!(matches!(res, Err(DesktopError::ConfigInvalid(_))));
    assert_eq!(r.desktop.get_section("appearance").unwrap(), before);
    let saved = State::load(&r.desktop.paths().state_file);
    assert_eq!(saved.settings.appearance.ui_font.as_deref(), Some("Inter"));
    let evs = drain(&mut r.events);
    assert!(evs.iter().all(|e| matches!(e, Event::ConfigError(_))));
}
