//! Importing and replacing themes.

use super::*;

#[tokio::test]
async fn import_saves_point_then_installs() {
    let mut r = rig().await;
    let before = r.desktop.state().clone();
    let n = r.desktop.restore_points().len();
    let text = file(&r.desktop, "ocean", "Ocean");
    assert_eq!(r.import(&text, false, false).await.unwrap(), "user-ocean");
    let points = r.desktop.restore_points();
    assert_eq!(points.len(), n + 1, "{points:?}");
    let point = points
        .iter()
        .find(|p| p.kind == PointKind::Auto(AutoReason::BeforeImport))
        .unwrap();
    assert_eq!(r.desktop.restore.load(&point.id).unwrap(), before);
    let dark = std::fs::read_to_string(r.root().join("user-ocean/dark.toml")).unwrap();
    let light = std::fs::read_to_string(r.root().join("user-ocean/light.toml")).unwrap();
    let got = uq_core::theme::user::scan(&r.root());
    assert_eq!(got.themes.len(), 1);
    assert!(dark.contains("Ocean") && light.contains("Ocean"));
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
    assert_eq!(r.themes_changed(), 1);
    assert_ne!(r.desktop.get_theme().0, "user-ocean");
}

#[tokio::test]
async fn refused_import_changes_nothing() {
    let mut r = rig().await;
    let n = r.desktop.restore_points().len();
    let good = file(&r.desktop, "ocean", "Ocean");
    let bad = [
        "not a theme".to_string(),
        good.replacen("format = 1", "format = 2", 1),
        good.replacen("name = \"Ocean\"", "name = \"X\\u2028Y\"", 1),
        good.replacen("id = \"ocean\"", "id = \"../x\"", 1),
        format!("{good}\nextra = 1\n"),
        " ".repeat(70_000),
    ];
    for text in bad {
        assert!(matches!(
            r.import(&text, false, false).await,
            Err(DesktopError::InvalidArgument(_))
        ));
    }
    assert_eq!(r.desktop.restore_points().len(), n);
    assert!(!r.root().exists());
    assert_eq!(r.themes_changed(), 0);
}

#[tokio::test]
async fn import_conflict_then_replace() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    match r.import(&text, false, false).await {
        Err(DesktopError::Conflict(m)) => assert!(m.ends_with("Replace it?"), "{m}"),
        other => panic!("{other:?}"),
    }
    r.import(&text, true, false).await.unwrap();
    assert_eq!(r.user_ids(), ["user-ocean"]);
}

#[tokio::test]
async fn replace_refuses_folder_with_extra_files() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    let notes = r.root().join("user-ocean/notes.txt");
    std::fs::write(&notes, "mine").unwrap();
    match r.import(&text, true, false).await {
        Err(DesktopError::InvalidArgument(m)) => assert!(m.contains("other files"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "mine");
}

#[tokio::test]
async fn import_sees_folder_removed_by_hand() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    std::fs::remove_dir_all(r.root().join("user-ocean")).unwrap();
    r.import(&text, false, false).await.unwrap();
}

#[tokio::test]
async fn replacing_active_theme_rerenders_without_history() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let tokens = r.desktop.paths().theme_dir.join("tokens.json");
    let old = read(&tokens).unwrap();
    let len = r.desktop.history(1000).len();
    let changed = text.replacen("radius = 8", "radius = 9", 1);
    let changed = if changed == text {
        // Whatever the fixture's radius is, move it.
        let t = uqtheme::parse(&text).unwrap();
        text.replacen(
            &format!("radius = {}", t.dark.ui.radius),
            &format!("radius = {}", t.dark.ui.radius + 1),
            1,
        )
    } else {
        changed
    };
    assert_ne!(changed, text);
    r.import(&changed, true, false).await.unwrap();
    assert_ne!(read(&tokens).unwrap(), old);
    assert_eq!(r.desktop.history(1000).len(), len);
}

#[tokio::test]
async fn import_with_look_is_one_undoable_entry() {
    let mut r = rig().await;
    r.desktop
        .set_section("look", r#"{"gaps_in":5}"#, Origin::Settings)
        .await
        .unwrap();
    let n = r.desktop.history(1000).len();
    let text = with_look(&file(&r.desktop, "ocean", "Ocean"));
    r.import(&text, false, true).await.unwrap();
    let h = r.desktop.history(1000);
    assert_eq!(h.len(), n + 1, "{h:?}");
    let last = h.first().unwrap(); // newest first
    assert_eq!(
        (last.domain.as_str(), last.origin.as_str()),
        ("look", "settings")
    );
    assert_eq!(r.desktop.state().section("look").unwrap()["gaps_in"], 3);
    let id = last.id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert_eq!(r.desktop.state().section("look").unwrap()["gaps_in"], 5);
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
}

#[tokio::test]
async fn import_look_rejected_by_verify_keeps_theme() {
    let mut r = rig().await;
    *r.comp.verify_error.lock().unwrap() = Some("nope".into());
    let text = with_look(&file(&r.desktop, "ocean", "Ocean"));
    let err = r.import(&text, false, true).await.unwrap_err();
    assert!(
        err.to_string().starts_with("The theme was added, but"),
        "{err}"
    );
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
    assert_eq!(r.themes_changed(), 1);
}

#[tokio::test]
async fn import_refuses_linked_root() {
    let mut r = rig().await;
    let target = r.dir.path().join("elsewhere");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(r.root().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, r.root()).unwrap();
    let text = file(&r.desktop, "ocean", "Ocean");
    let err = r.import(&text, false, false).await.unwrap_err();
    assert!(err.to_string().contains("is a link"), "{err}");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}

#[tokio::test]
async fn import_refuses_in_dev_session() {
    let mut r = rig().await;
    r.desktop.dev_session = true;
    let text = file(&r.desktop, "ocean", "Ocean");
    let err = r.import(&text, false, false).await.unwrap_err();
    assert!(err.to_string().contains("test session"), "{err}");
    assert!(r.desktop.export_theme("gruvbox", false).is_ok());
    assert!(!r.root().exists());
}

#[tokio::test]
async fn import_caps_at_fifty() {
    let mut r = rig().await;
    for n in 0..MAX_USER_THEMES {
        let text = file(&r.desktop, &format!("t{n}"), &format!("T {n}"));
        r.import(&text, false, false).await.unwrap();
    }
    let text = file(&r.desktop, "one-more", "One more");
    assert!(matches!(
        r.import(&text, false, false).await,
        Err(DesktopError::InvalidArgument(_))
    ));
    // A replace doesn't count.
    let again = file(&r.desktop, "t3", "T 3");
    r.import(&again, true, false).await.unwrap();
    assert_eq!(r.user_ids().len(), MAX_USER_THEMES);
}

#[tokio::test]
async fn replace_refuses_a_directory_named_like_a_variant() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    let dark = r.root().join("user-ocean/dark.toml");
    std::fs::remove_file(&dark).unwrap();
    std::fs::create_dir(&dark).unwrap();
    std::fs::write(dark.join("keep.txt"), "mine").unwrap();
    assert!(r.import(&text, true, false).await.is_err());
    assert!(dark.join("keep.txt").exists());
}
