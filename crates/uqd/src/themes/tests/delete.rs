//! Deleting themes and themes that went away.

use super::*;

#[tokio::test]
async fn delete_active_switches_then_deletes() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let out = r
        .desktop
        .delete_theme("user-ocean", Origin::Uqctl)
        .await
        .unwrap();
    assert_eq!(out, json!({"switched": true, "profiles": []}));
    assert!(!r.root().join("user-ocean").exists());
    assert_eq!(r.desktop.get_theme().0, DEFAULT_THEME);
    let h = r.desktop.history(1000);
    assert!(
        h.iter().any(|e| e.domain == "theme" && e.origin == "uqctl"),
        "{h:?}"
    );
    assert!(!r.desktop.theme_ids().contains(&"user-ocean".to_string()));
}

#[tokio::test]
async fn delete_reports_profiles_using_it() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_profile(
            "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[appearance]\ntheme = \"user-ocean\"\n",
            false,
        )
        .unwrap();
    let out = r
        .desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(out, json!({"switched": false, "profiles": ["Gaming"]}));
}

#[tokio::test]
async fn delete_refuses_system_and_unknown() {
    let mut r = rig().await;
    assert!(matches!(
        r.desktop.delete_theme("gruvbox", Origin::Settings).await,
        Err(DesktopError::InvalidArgument(_))
    ));
    assert!(matches!(
        r.desktop.delete_theme("user-nope", Origin::Settings).await,
        Err(DesktopError::UnknownTheme(_))
    ));
    r.desktop.dev_session = true;
    assert!(matches!(
        r.desktop.delete_theme("user-nope", Origin::Settings).await,
        Err(DesktopError::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn delete_never_follows_links() {
    let mut r = rig().await;
    let target = r.dir.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    write_theme(&target.parent().unwrap().join("t"), "user-x");
    std::fs::rename(target.parent().unwrap().join("t/user-x"), &target).unwrap();
    std::fs::create_dir_all(r.root()).unwrap();
    std::os::unix::fs::symlink(&target, r.root().join("user-x")).unwrap();
    assert!(
        r.desktop
            .delete_theme("user-x", Origin::Settings)
            .await
            .is_err()
    );
    assert!(target.join("dark.toml").exists() && target.join("light.toml").exists());
    // Swapped for a link after it was loaded: still refused, target untouched.
    r.desktop
        .import_theme(&file(&r.desktop, "y", "Y"), false, false, Origin::Settings)
        .await
        .unwrap();
    std::fs::remove_dir_all(r.root().join("user-y")).unwrap();
    std::os::unix::fs::symlink(&target, r.root().join("user-y")).unwrap();
    assert!(
        r.desktop
            .delete_theme("user-y", Origin::Settings)
            .await
            .is_err()
    );
    assert!(target.join("dark.toml").exists());
}

#[tokio::test]
async fn delete_is_not_recursive() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    let notes = r.root().join("user-ocean/notes.txt");
    std::fs::write(&notes, "mine").unwrap();
    r.desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert!(notes.exists());
    assert!(r.user_ids().is_empty());
}

#[tokio::test]
async fn active_theme_kept_when_folder_removed() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    std::fs::remove_dir_all(r.root().join("user-ocean")).unwrap();
    assert!(r.desktop.refresh_themes());
    let info = r.desktop.theme_info();
    let t = info["themes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "user-ocean")
        .unwrap();
    assert_eq!(t["missing"], true);
    r.desktop
        .set_section("look", r#"{"gaps_in":6}"#, Origin::Settings)
        .await
        .unwrap();
}

#[tokio::test]
async fn missing_theme_goes_once_another_is_picked() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    std::fs::remove_dir_all(r.root().join("user-ocean")).unwrap();
    assert!(
        r.desktop.theme_info()["themes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "user-ocean")
    );
    r.desktop
        .set_theme("graphite", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let info = r.desktop.theme_info();
    assert!(
        info["themes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"] != "user-ocean"),
        "{info}"
    );
}

#[tokio::test]
async fn unknown_theme_reads_as_a_sentence() {
    let mut r = rig().await;
    let err = r
        .desktop
        .set_theme("user-nope", Variant::Dark, Origin::Settings)
        .await
        .unwrap_err();
    assert_eq!(err, DesktopError::UnknownTheme("user-nope".into()));
    assert_eq!(err.to_string(), "That theme isn't installed any more.");
}
