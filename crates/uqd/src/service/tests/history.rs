//! History and restore points.

use super::*;

#[tokio::test]
async fn section_write_appends_one_history_entry() {
    let mut r = rig();
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    let entries = journal_of(&r);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].origin, "settings");
    assert_eq!(entries[0].domain, "look");
    assert_eq!(r.desktop.last_entry_ids(), [entries[0].id.clone()]);
}

#[tokio::test]
async fn light_setting_write_is_journalled() {
    let mut r = rig();
    r.desktop
        .set_setting("idle.dim", serde_json::json!(200), Origin::Uqctl)
        .await
        .unwrap();
    let entries = journal_of(&r);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].origin, "uqctl");
    assert_eq!(entries[0].domain, "settings");
}

#[tokio::test]
async fn rejected_verify_writes_no_entry() {
    let mut r = rig();
    *r.comp.verify_error.lock().unwrap() = Some("bad".into());
    let res = r.desktop.set_layout("1", "master", Origin::Settings).await;
    assert!(matches!(res, Err(DesktopError::ConfigInvalid(_))));
    assert!(journal_of(&r).is_empty());
    assert!(r.desktop.restore.list().is_empty());
}

#[tokio::test]
async fn first_change_of_the_day_saves_a_daily_point() {
    let mut r = rig();
    r.desktop
        .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
        .await
        .unwrap();
    let points = r.desktop.restore.list();
    assert_eq!(points.len(), 1, "{points:?}");
    assert_eq!(points[0].kind, PointKind::Auto(AutoReason::Daily));
    // The point holds the state from before the change.
    let saved = r.desktop.restore.load(&points[0].id).unwrap();
    assert_ne!(saved.settings.idle.dim, Some(200));
    r.desktop
        .set_setting("idle.dim", serde_json::json!(100), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.restore.list().len(), 1);
}

#[tokio::test]
async fn journal_failure_does_not_fail_the_write() {
    let mut r = rig_with(true, |p| {
        fs::create_dir_all(p.history_dir.parent().unwrap()).unwrap();
        fs::write(&p.history_dir, "not a directory").unwrap();
    });
    r.desktop
        .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    assert_eq!(State::load(&p.state_file).settings.idle.dim, Some(200));
    assert!(r.desktop.last_entry_ids().is_empty());
}

#[tokio::test]
async fn history_changed_event_sent() {
    let mut r = rig();
    r.desktop
        .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
        .await
        .unwrap();
    assert!(drain_all(&mut r.events).contains(&Event::HistoryChanged));
    // A no-op write journals nothing and sends nothing.
    r.desktop
        .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
        .await
        .unwrap();
    assert!(!drain_all(&mut r.events).contains(&Event::HistoryChanged));
    assert!(r.desktop.last_entry_ids().is_empty());
}

#[tokio::test]
async fn restore_after_rejected_write_clears_error() {
    let mut r = rig();
    let id = r.desktop.save_restore_point("before").unwrap();
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap_err();
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "error", "{v}");
    *r.comp.verify_error.lock().unwrap() = None;
    r.desktop.restore(&id).await.unwrap();
    let v = r.desktop.explain("look").await.unwrap();
    assert!(
        v.as_object()
            .unwrap()
            .values()
            .all(|k| k["source"] != "error"),
        "{v}"
    );
    // A theme change re-renders and verifies every section too.
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    r.desktop
        .set_section("layouts", r#"{"default":"master"}"#, Origin::Settings)
        .await
        .unwrap_err();
    *r.comp.verify_error.lock().unwrap() = None;
    r.desktop
        .set_theme("forest", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let v = r.desktop.explain("layouts").await.unwrap();
    assert_ne!(v["default"]["source"], "error", "{v}");
}

#[tokio::test]
async fn reapply_writes_no_history() {
    let mut r = rig();
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(journal_of(&r).len(), 1);
    r.comp.calls.lock().unwrap().clear();
    for domain in ["look", "layouts", "appearance", "wallpaper", "shell"] {
        r.desktop.reapply(domain).await.unwrap();
    }
    assert_eq!(journal_of(&r).len(), 1, "{:?}", journal_of(&r));
    assert!(r.desktop.last_entry_ids().is_empty());
    assert!(r.comp.calls().contains(&"reload".to_string()));
    assert_eq!(r.desktop.state().settings.look.gaps_in, Some(9));
    assert!(matches!(
        r.desktop.reapply("bogus").await,
        Err(DesktopError::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn history_summary_names_settings_by_label() {
    let mut r = rig();
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    let summary = &journal_of(&r)[0].summary;
    assert!(summary.starts_with("Gaps between windows:"), "{summary}");
}
