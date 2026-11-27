//! Applying and turning off a profile.

use super::*;

#[tokio::test]
async fn apply_sets_keys_with_profile_origin() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(report.active, "gaming");
    assert!(
        report.kept.is_empty() && report.notices.is_empty(),
        "{report:?}"
    );
    assert_eq!(look(&r, "gaps_in"), Some(json!(0)));
    assert_eq!(look(&r, "blur"), Some(json!(false)));
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert!(!new.is_empty());
    assert!(new.iter().all(|e| e.origin == "profile:gaming"), "{new:?}");
    assert_eq!(r.desktop.active_profile().unwrap().id, "gaming");
}

#[tokio::test]
async fn apply_saves_before_profile_point() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let had: Vec<_> = r
        .desktop
        .restore_points()
        .into_iter()
        .map(|p| p.id)
        .collect();
    r.desktop.apply_profile("gaming").await.unwrap();
    // Points made in the same second sort by a random suffix, so look for the new one.
    let new: Vec<_> = r
        .desktop
        .restore_points()
        .into_iter()
        .filter(|p| !had.contains(&p.id))
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].kind, PointKind::Auto(AutoReason::BeforeProfile));
    assert_eq!(new[0].name, "Before the Gaming profile");
    // The point holds the settings from before the profile.
    let saved = r.desktop.restore.load(&new[0].id).unwrap();
    assert_eq!(read_leaf(&saved, "look", &s(&["gaps_in"])), Some(json!(5)));
}

#[tokio::test]
async fn off_restores_untouched_and_reports_hand_changed() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    set_look(&mut r, "gaps_in", json!(3)).await;
    let before = journal_len(&r);
    let report = r.desktop.profile_off().await.unwrap();
    assert_eq!(look(&r, "blur"), Some(json!(true)));
    assert_eq!(look(&r, "gaps_in"), Some(json!(3)));
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Gaps between windows"]);
    assert_eq!(report.active, "");
    assert!(r.desktop.active_profile().is_none());
    assert!(!r.desktop.paths().profile_active.exists());
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert!(!new.is_empty());
    assert!(new.iter().all(|e| e.origin == "profile:gaming"), "{new:?}");
}

#[tokio::test]
async fn off_removes_a_display_entry_the_profile_created() {
    let mut r = rig().await;
    r.desktop
        .set_section(
            "wallpaper",
            r#"{"outputs":{"*":{"image":"/w/a.png"}}}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let before = r.desktop.get_section("wallpaper").unwrap();
    add(
        &mut r,
        "format = 1\nid = \"hdmi\"\nname = \"HDMI\"\n[wallpaper.outputs.HDMI-A-1]\nimage = \"/w/b.png\"\n",
    )
    .await;
    r.desktop.apply_profile("hdmi").await.unwrap();
    assert_ne!(r.desktop.get_section("wallpaper").unwrap(), before);
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert_eq!(r.desktop.get_section("wallpaper").unwrap(), before);
}

#[tokio::test]
async fn switching_profiles_returns_untouched_keys_then_applies_next() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    add(&mut r, WORK).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    set_look(&mut r, "gaps_in", json!(3)).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("work").await.unwrap();
    assert_eq!(report.active, "work");
    let labels: Vec<_> = report.kept.iter().map(|k| k.label.as_str()).collect();
    assert_eq!(labels, ["Gaps between windows"]);
    // Gaming's untouched key is back, Work's keys are on.
    assert_eq!(look(&r, "blur"), Some(json!(true)));
    assert_eq!(look(&r, "gaps_in"), Some(json!(8)));
    assert_eq!(look(&r, "rounding"), Some(json!(2)));
    // One write, journalled as Work.
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].origin, "profile:work");
    // Work remembers the hand value, so Back to normal goes to it.
    r.desktop.profile_off().await.unwrap();
    assert_eq!(look(&r, "gaps_in"), Some(json!(3)));
    assert_eq!(look(&r, "rounding"), None);
    assert_eq!(look(&r, "blur"), Some(json!(true)));
}

#[tokio::test]
async fn active_profile_survives_restart() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    assert_eq!(again.active_profile().unwrap().id, "gaming");
    again.profile_off().await.unwrap();
    let st = again.state();
    assert_eq!(read_leaf(st, "look", &s(&["gaps_in"])), Some(json!(5)));
    assert_eq!(read_leaf(st, "look", &s(&["blur"])), Some(json!(true)));
}

#[tokio::test]
async fn active_file_written_but_state_not_still_turns_off_cleanly() {
    let r = rig().await;
    // uqd stopped after writing the active file and before the commit.
    let p = Profile::parse(GAMING).unwrap();
    let (_, memo) = uq_core::profile::apply(r.desktop.state(), &p, &Default::default(), 1).unwrap();
    save_active(&r.desktop.paths().profile_active, &memo).unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    let state_file = read(&again.paths().state_file);
    let entries = again.history(1000).len();
    let report = again.profile_off().await.unwrap();
    assert_eq!(report, ProfileReport::default());
    assert_eq!(read(&again.paths().state_file), state_file);
    assert_eq!(again.history(1000).len(), entries);
    assert!(again.active_profile().is_none());
    assert!(!again.paths().profile_active.exists());
}

#[tokio::test]
async fn failed_verify_changes_nothing_and_keeps_previous_profile() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    add(&mut r, WORK).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let state = r.desktop.state().clone();
    let active_file = read(&r.desktop.paths().profile_active);
    let points = r.desktop.restore_points().len();
    let entries = journal_len(&r);
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    let err = r.desktop.apply_profile("work").await.unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(*r.desktop.state(), state);
    assert_eq!(read(&r.desktop.paths().profile_active), active_file);
    assert_eq!(r.desktop.active_profile().unwrap().id, "gaming");
    assert_eq!(r.desktop.restore_points().len(), points);
    assert_eq!(journal_len(&r), entries);
    assert!(profile_events(&mut r.events).is_empty());
    // With no profile on before, a failed apply leaves no active file.
    *r.comp.verify_error.lock().unwrap() = None;
    r.desktop.profile_off().await.unwrap();
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    r.desktop.apply_profile("work").await.unwrap_err();
    assert!(!r.desktop.paths().profile_active.exists());
    assert!(r.desktop.active_profile().is_none());
}

#[tokio::test]
async fn apply_with_uninstalled_theme_changes_nothing() {
    let mut r = rig().await;
    // Saved while the theme was there; set_profile would refuse it now.
    let text = "format = 1\nid = \"gone\"\nname = \"Gone\"\n[appearance]\ntheme = \"nosuch\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";
    let dir = &r.desktop.paths().profiles_dir;
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("gone.toml"), text).unwrap();
    let state = r.desktop.state().clone();
    let points = r.desktop.restore_points().len();
    let err = r.desktop.apply_profile("gone").await.unwrap_err();
    assert_eq!(err, DesktopError::UnknownTheme("nosuch".into()));
    assert_eq!(*r.desktop.state(), state);
    assert_eq!(r.desktop.restore_points().len(), points);
    assert!(!r.desktop.paths().profile_active.exists());
    assert!(r.desktop.active_profile().is_none());
    assert!(r.power.sets.lock().unwrap().is_empty());
    assert!(profile_events(&mut r.events).is_empty());
}

#[tokio::test]
async fn off_with_the_old_theme_uninstalled_uses_the_default_theme() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"red\"\nname = \"Red\"\n[appearance]\ntheme = \"rosewood\"\n[look]\ngaps_in = 0\n",
    )
    .await;
    r.desktop.apply_profile("red").await.unwrap();
    // The theme in use before the profile has been uninstalled since.
    let mut memo = r.desktop.active_profile().unwrap().clone();
    let theme = memo.keys.iter_mut().find(|m| m.path == ["theme"]).unwrap();
    theme.before = Some(json!("nosuch"));
    save_active(&r.desktop.paths().profile_active, &memo).unwrap();
    let mut again = desktop_on(r.dir.path(), r.comp.clone(), r.power.clone());
    let report = again.profile_off().await.unwrap();
    assert_eq!(again.state().theme, uq_core::state::State::default().theme);
    assert_eq!(
        read_leaf(again.state(), "look", &s(&["gaps_in"])),
        Some(json!(5))
    );
    assert!(again.active_profile().is_none());
    assert_eq!(report.notices, [THEME_GONE], "{report:?}");
}

#[tokio::test]
async fn restore_ends_the_active_profile() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    let id = r.desktop.save_restore_point("mine").unwrap();
    r.desktop.apply_profile("gaming").await.unwrap();
    drain(&mut r.events);
    r.desktop.restore(&id).await.unwrap();
    assert!(r.desktop.active_profile().is_none());
    assert!(!r.desktop.paths().profile_active.exists());
    assert_eq!(profile_events(&mut r.events), [""]);
    assert_eq!(look(&r, "gaps_in"), Some(json!(5)));
}
