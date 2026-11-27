//! What a profile turns off: rules and shortcuts.

use super::*;

#[tokio::test]
async fn schedule_holds_the_variant_a_profile_sets() {
    let mut r = rig().await;
    add(&mut r, NIGHT).await;
    let day = chrono::Local::now().date_naive();
    let tomorrow = day.succ_opt().unwrap();
    let at = |d: chrono::NaiveDate, h, m| {
        d.and_hms_opt(h, m, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
    };
    let sched = Schedule::Fixed {
        light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
    };
    r.desktop
        .set_schedule_at(sched.clone(), at(day, 20, 0), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    r.desktop.apply_profile("night").await.unwrap();
    // Sunrise while the profile holds dark: still dark.
    assert!(
        !r.desktop
            .tick_schedule_at(at(tomorrow, 7, 1))
            .await
            .unwrap()
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    // Storing the schedule again doesn't flip it either.
    r.desktop
        .set_schedule_at(sched, at(tomorrow, 8, 0), Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    r.desktop.profile_off().await.unwrap();
    assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    // The first tick after Back to normal applies the scheduled variant.
    assert!(
        r.desktop
            .tick_schedule_at(at(tomorrow, 8, 1))
            .await
            .unwrap()
    );
    assert_eq!(r.desktop.get_theme().1, Variant::Light);
}

#[tokio::test]
async fn profile_turns_a_rule_off_and_back() {
    let mut r = rig().await;
    let id = add_rule(&mut r).await;
    add(&mut r, &rules_profile(&format!("\"{id}\" = false\n"))).await;
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert!(report.notices.is_empty(), "{report:?}");
    assert!(!r.desktop.state().rules.window[0].enabled);
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].domain, "rules");
    assert_eq!(new[0].origin, "profile:quiet");
    assert_eq!(
        new[0].summary,
        "Window rule “Pavucontrol floats” turned off"
    );
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(
        g.contains(&format!("name = \"uq:{id}\", enabled = false")),
        "{g}"
    );
    assert!(r.comp.calls().iter().any(|c| c.starts_with("verify")));
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert!(r.desktop.state().rules.window[0].enabled);
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(
        g.contains(&format!("name = \"uq:{id}\", enabled = true")),
        "{g}"
    );
}

#[tokio::test]
async fn unknown_rule_id_is_a_notice() {
    let mut r = rig().await;
    let id = add_rule(&mut r).await;
    let other = if id == "r-ffff" { "r-fffe" } else { "r-ffff" };
    add(
        &mut r,
        &rules_profile(&format!("\"{id}\" = false\n\"{other}\" = true\n")),
    )
    .await;
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(report.active, "quiet");
    assert_eq!(
        report.notices,
        ["1 rule in this profile isn't on this computer, so it was left out."]
    );
    assert!(!r.desktop.state().rules.window[0].enabled);
    // The file keeps the id.
    assert!(r.desktop.get_profile("quiet").unwrap().contains(other));
}

#[tokio::test]
async fn profile_turns_a_shortcut_off_and_back() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[keybinds]\n\"uq.close\" = false\n",
    )
    .await;
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(g.contains("uq:uq.close"), "{g}");
    let before = journal_len(&r);
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert!(report.notices.is_empty(), "{report:?}");
    let entries = r.desktop.history(1000);
    let new = &entries[..entries.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(new[0].domain, "keybinds");
    assert_eq!(new[0].origin, "profile:quiet");
    assert_eq!(new[0].summary, "“Close the window” turned off");
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(!g.contains("uq:uq.close"), "{g}");
    let rows = r.desktop.explain_keybinds().await;
    assert_eq!(rows["binds.uq.close"]["source"], "profile");
    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    let g = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(g.contains("uq:uq.close"), "{g}");
}

#[tokio::test]
async fn unknown_shortcut_id_is_a_notice() {
    let mut r = rig().await;
    add(
        &mut r,
        "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[keybinds]\n\"uq.close\" = false\n\"k-ffff\" = true\n",
    )
    .await;
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(report.notices.len(), 1, "{report:?}");
    assert!(report.notices[0].contains("shortcut"), "{report:?}");
}
