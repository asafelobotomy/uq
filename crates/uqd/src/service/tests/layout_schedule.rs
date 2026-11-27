//! Layouts and the light/dark schedule.

use super::*;

#[tokio::test]
async fn named_workspace_layout_uses_name_selector() {
    let mut r = rig();
    r.desktop
        .set_layout("web", "scrolling", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(
        r.desktop.get_layouts().get("name:web"),
        Some(&Layout::Scrolling)
    );
    assert!(
        r.comp.calls().contains(
            &r#"eval hl.workspace_rule({ workspace = "name:web", layout = "scrolling" })"#
                .to_string()
        ),
        "{:?}",
        r.comp.calls()
    );
}

#[tokio::test]
async fn manual_override_holds_until_next_boundary() {
    let mut r = rig();
    let day = chrono::Local::now().date_naive();
    let at = |d: chrono::NaiveDate, h, m| {
        d.and_hms_opt(h, m, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
    };
    let s = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    r.desktop
        .set_schedule_at(s, at(day, 20, 0), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    r.desktop
        .set_theme("graphite", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert!(
        !r.desktop.tick_schedule_at(at(day, 22, 1)).await.unwrap(),
        "override reverted"
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Light);
    let tomorrow = day.succ_opt().unwrap();
    r.desktop
        .tick_schedule_at(at(tomorrow, 7, 1))
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Light);
    assert!(
        r.desktop
            .tick_schedule_at(at(tomorrow, 19, 1))
            .await
            .unwrap()
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
}

#[tokio::test]
async fn set_layout_persists_and_evals() {
    let mut r = rig();
    r.desktop
        .set_layout("2", "scrolling", Origin::Settings)
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    assert!(
        r.comp.calls().contains(
            &r#"eval hl.workspace_rule({ workspace = "2", layout = "scrolling" })"#.to_string()
        ),
        "{:?}",
        r.comp.calls()
    );
    assert_eq!(
        State::load(&p.state_file).layouts.get("2"),
        Some(&Layout::Scrolling)
    );
    assert!(
        fs::read_to_string(&p.generated_lua)
            .unwrap()
            .contains(r#"workspace = "2", layout = "scrolling""#)
    );
    assert_eq!(r.desktop.get_layouts().get("2"), Some(&Layout::Scrolling));
    let evs = drain(&mut r.events);
    assert_eq!(evs.len(), 2, "{evs:?}");
    assert_eq!(
        evs[0],
        Event::LayoutChanged {
            workspace: "2".into(),
            layout: Layout::Scrolling
        }
    );
    assert!(matches!(&evs[1], Event::SectionChanged { domain, .. } if domain == "layouts"));
    // A layout change doesn't re-theme the session.
    assert!(r.sys.applied.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_layout_rejected() {
    let mut r = rig();
    let err = r
        .desktop
        .set_layout("1", "monocle", Origin::Settings)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::InvalidLayout(ref l) if l == "monocle"));
}

#[tokio::test]
async fn set_layout_enforces_section_limits() {
    let mut r = rig();
    let long = "a".repeat(65);
    assert!(matches!(
        r.desktop
            .set_layout(&long, "master", Origin::Settings)
            .await,
        Err(DesktopError::InvalidSetting(_))
    ));
    assert!(matches!(
        r.desktop
            .set_layout("a\u{1}b", "master", Origin::Settings)
            .await,
        Err(DesktopError::InvalidSetting(_))
    ));
    for i in 1..=64 {
        r.desktop
            .set_layout(&i.to_string(), "master", Origin::Settings)
            .await
            .unwrap();
    }
    let err = r
        .desktop
        .set_layout("65", "master", Origin::Settings)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("64"), "{err}");
    // Changing an existing rule at the limit is fine.
    r.desktop
        .set_layout("64", "scrolling", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.state.layouts.len(), 64);
    let v = r.desktop.state.section("layouts").unwrap();
    r.desktop.state.clone().set_section("layouts", v).unwrap();
}

#[tokio::test]
async fn empty_workspace_rejected() {
    let mut r = rig();
    assert!(matches!(
        r.desktop.set_layout("  ", "master", Origin::Settings).await,
        Err(DesktopError::EmptyWorkspace)
    ));
}

#[tokio::test]
async fn set_schedule_applies_current_variant() {
    let mut r = rig();
    r.desktop
        .set_theme("graphite", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let noon = chrono::Local::now()
        .date_naive()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let now = noon.and_local_timezone(chrono::Local).earliest().unwrap();
    let s = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    r.desktop
        .set_schedule_at(s.clone(), now, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(
        r.desktop.get_theme(),
        ("graphite".to_string(), Variant::Light)
    );
    assert_eq!(State::load(&r.desktop.paths().state_file).schedule, s);
}

#[tokio::test]
async fn schedule_tick_switches_variant_when_due() {
    let mut r = rig();
    let at = |h| {
        chrono::Local::now()
            .date_naive()
            .and_hms_opt(h, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
    };
    let s = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    r.desktop
        .set_schedule_at(s, at(12), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Light);
    drain(&mut r.events);
    assert!(
        !r.desktop.tick_schedule_at(at(13)).await.unwrap(),
        "nothing due at 13:00"
    );
    assert!(drain(&mut r.events).is_empty());
    // The wake-up is the 19:00 boundary, and at that time the switch is due.
    let next = r.desktop.next_schedule_event(at(13)).unwrap();
    assert_eq!(next, at(19).with_timezone(&Utc));
    assert!(
        r.desktop.tick_schedule_at(at(20)).await.unwrap(),
        "dark due at 20:00"
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
}

#[tokio::test]
async fn schedule_tick_off_does_nothing() {
    let mut r = rig();
    assert_eq!(r.desktop.next_schedule_event(chrono::Utc::now()), None);
    assert!(
        !r.desktop
            .tick_schedule_at(chrono::Utc::now())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn invalid_schedule_rejected() {
    let mut r = rig();
    let t = chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap();
    let err = r
        .desktop
        .set_schedule(
            Schedule::Fixed {
                light_at: t,
                dark_at: t,
            },
            Origin::Settings,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSchedule(_)));
}

#[tokio::test]
async fn schedule_tick_is_not_journalled() {
    let mut r = rig();
    let day = chrono::Local::now().date_naive();
    let at = |d: chrono::NaiveDate, h| {
        d.and_hms_opt(h, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
    };
    let s = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    // At 20:00 the schedule wants dark, the default, so setting it journals one entry.
    r.desktop
        .set_schedule_at(s, at(day, 20), Origin::Settings)
        .await
        .unwrap();
    let entries = journal_of(&r);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let points = r.desktop.restore.list().len();
    let next_morning = at(day.succ_opt().unwrap(), 8);
    assert!(r.desktop.tick_schedule_at(next_morning).await.unwrap());
    assert_eq!(journal_of(&r), entries);
    // The tick leaves the undo target alone.
    assert_eq!(r.desktop.last_entry_ids(), [entries[0].id.clone()]);
    assert_eq!(r.desktop.restore.list().len(), points);
}
