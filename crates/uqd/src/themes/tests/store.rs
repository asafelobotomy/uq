//! Loading the store, exporting, and what it never touches.

use super::*;

#[tokio::test]
async fn startup_loads_user_themes_and_keeps_active() {
    let dir = tempfile::tempdir().unwrap();
    let root = paths(dir.path()).user_themes_dir;
    write_theme(&root, "user-ocean");
    std::fs::create_dir_all(dir.path().join("x")).ok();
    let state_file = paths(dir.path()).state_file;
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    std::fs::write(&state_file, "theme = \"user-ocean\"\nvariant = \"light\"\n").unwrap();
    let d = desktop_on(dir.path(), Arc::new(FakeCompositor::running()));
    assert_eq!(d.get_theme(), ("user-ocean".to_string(), Variant::Light));
}

#[tokio::test]
async fn startup_survives_broken_user_themes() {
    let dir = tempfile::tempdir().unwrap();
    let root = paths(dir.path()).user_themes_dir;
    std::fs::create_dir_all(root.join("notuser")).unwrap();
    std::fs::create_dir_all(root.join("user-onlydark")).unwrap();
    std::fs::write(root.join("user-onlydark/dark.toml"), "x").unwrap();
    std::fs::create_dir_all(root.join("user-junk")).unwrap();
    std::fs::write(root.join("user-junk/dark.toml"), "not a theme").unwrap();
    std::fs::write(root.join("user-junk/light.toml"), "not a theme").unwrap();
    std::os::unix::fs::symlink("/tmp", root.join("user-link")).unwrap();
    write_theme(&root, "user-fine");
    let mut d = desktop_on(dir.path(), Arc::new(FakeCompositor::running()));
    let info = d.theme_info();
    assert_eq!(info["skipped"].as_array().unwrap().len(), 4, "{info}");
    assert!(
        info["themes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "user-fine")
    );
}

#[tokio::test]
async fn export_matches_registry_and_never_holds_overrides() {
    let mut r = rig().await;
    r.desktop
        .set_section("appearance", r#"{"ui_font":"Comic"}"#, Origin::Settings)
        .await
        .unwrap();
    r.desktop
        .set_section("look", r#"{"gaps_in":4}"#, Origin::Settings)
        .await
        .unwrap();
    let plain = uqtheme::parse(&r.desktop.export_theme("gruvbox", false).unwrap()).unwrap();
    let stored = r.desktop.themes.get("gruvbox", Variant::Dark).unwrap();
    assert_eq!(plain.dark.fonts, stored.fonts);
    assert!(plain.look.is_none());
    let full = uqtheme::parse(&r.desktop.export_theme("gruvbox", true).unwrap()).unwrap();
    let look = full.look.unwrap();
    assert_eq!(look["gaps_in"], 4);
    assert!(look.values().all(|v| !v.is_null()));
    assert!(matches!(
        r.desktop.export_theme("nope", false),
        Err(DesktopError::UnknownTheme(_))
    ));
}

#[tokio::test]
async fn refresh_without_change_reads_nothing() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.events();
    assert!(!r.desktop.refresh_themes());
    assert_eq!(r.themes_changed(), 0);
    write_theme(&r.root(), "user-hand");
    assert!(r.desktop.refresh_themes());
    assert_eq!(r.themes_changed(), 1);
    assert!(r.user_ids().contains(&"user-hand".to_string()));
}

#[tokio::test]
async fn user_lua_untouched() {
    let mut r = rig().await;
    let lua = r.desktop.paths().user_lua.clone();
    std::fs::create_dir_all(lua.parent().unwrap()).unwrap();
    std::fs::write(&lua, "-- mine\n").unwrap();
    let (bytes, time) = (read(&lua), mtime(&lua));
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop.export_theme("user-ocean", true).unwrap();
    r.desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert_eq!((read(&lua), mtime(&lua)), (bytes, time));
}

#[tokio::test]
async fn no_state_key_from_theme_store() {
    let mut r = rig().await;
    r.desktop
        .set_section("look", r#"{"gaps_in":6}"#, Origin::Settings)
        .await
        .unwrap();
    let state_file = r.desktop.paths().state_file.clone();
    let before = read(&state_file);
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    assert_eq!(read(&state_file), before);
}
