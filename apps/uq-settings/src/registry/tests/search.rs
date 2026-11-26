//! Search entries: keywords, ids and what search finds.

use super::*;

#[test]
fn provenance_rows_name_registry_settings() {
    let ids: HashSet<_> = core_pages()
        .iter()
        .flat_map(|p| p.settings.iter().map(|s| s.id))
        .collect();
    for row in uq_core::settings::provenance::OPTIONS {
        assert!(
            ids.contains(row.setting_id),
            "{} names setting {:?}, which no page lists",
            row.key,
            row.setting_id
        );
    }
}

#[test]
fn theme_entries_on_appearance_page() {
    let page = find("appearance").unwrap();
    assert_eq!(page.settings.len(), 14);
    for (id, label) in [
        ("theme-new", "New theme from this one"),
        ("theme-import", "Import a theme"),
        ("theme-export", "Export a theme"),
    ] {
        let s = page.settings.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s.label, label);
    }
    let entries: Vec<_> = core_pages().iter().map(PageEntry::from).collect();
    let found = |q: &str, id: &str| {
        crate::search::search(&entries, q)
            .iter()
            .any(|h| h.page_id == "appearance" && h.setting_id == id)
    };
    assert!(found("import a theme", "theme-import"));
    assert!(found("backup", "theme-export"));
    assert!(found("create", "theme-new"));
}

#[test]
fn display_settings_have_keywords() {
    let page = find("displays").expect("the displays page is registered");
    assert_eq!(page.settings.len(), 14);
    for id in [
        "display-main",
        "display-bar",
        "display-arrangements-auto",
        "display-remember",
        "display-workspaces",
        "display-workspaces-back",
    ] {
        let s = page.settings.iter().find(|s| s.id == id).unwrap();
        assert!(!s.keywords.is_empty(), "{id}");
    }
}

#[test]
fn search_finds_main_display() {
    let entries: Vec<_> = core_pages().iter().map(PageEntry::from).collect();
    let found = |q: &str, id: &str| {
        crate::search::search(&entries, q)
            .iter()
            .any(|h| h.page_id == "displays" && h.setting_id == id)
    };
    assert!(found("main display", "display-main"));
    assert!(found("primary", "display-main"));
    assert!(found("panel", "display-bar"));
    assert!(found("dock", "display-arrangements-auto"));
    assert!(found("remember", "display-remember"));
    assert!(found("planner", "display-workspaces"));
    assert!(found("return", "display-workspaces-back"));
}

#[test]
fn gesture_settings_have_keywords() {
    let page = find("input").expect("the input page is registered");
    assert_eq!(page.settings.len(), 19);
    for (id, label) in [
        ("gestures", "Touchpad gestures"),
        ("gesture-swipe-distance", "Swipe distance"),
        ("gesture-swipe-invert", "Reverse swipe direction"),
        (
            "gesture-swipe-create",
            "Swipe past the last workspace to make a new one",
        ),
        (
            "gesture-swipe-cancel",
            "How far to swipe before it switches",
        ),
    ] {
        let s = page.settings.iter().find(|s| s.id == id).unwrap();
        assert_eq!(s.label, label);
        assert!(!s.keywords.is_empty(), "{id}");
    }
}

#[test]
fn search_finds_gestures() {
    let entries: Vec<_> = core_pages().iter().map(PageEntry::from).collect();
    let found = |q: &str, id: &str| {
        crate::search::search(&entries, q)
            .iter()
            .any(|h| h.page_id == "input" && h.setting_id == id)
    };
    assert!(found("gesture", "gestures"));
    assert!(found("swipe", "gestures"));
    assert!(found("pinch", "gestures"));
    assert!(found("three fingers", "gestures"));
    assert!(found("swipe distance", "gesture-swipe-distance"));
    assert!(found("natural", "gesture-swipe-invert"));
    assert!(found("new workspace", "gesture-swipe-create"));
    assert!(found("cancel", "gesture-swipe-cancel"));
}

#[test]
fn every_setting_has_keywords() {
    for p in core_pages() {
        for s in p.settings {
            assert!(
                !s.keywords.is_empty() && s.keywords.iter().all(|k| !k.trim().is_empty()),
                "{} on {} has no search keywords",
                s.id,
                p.id
            );
        }
    }
}

#[test]
fn no_duplicate_setting_ids() {
    let mut seen = HashSet::new();
    for p in core_pages() {
        for s in p.settings {
            assert!(
                seen.insert(s.id),
                "setting id {:?} is on more than one page (second: {})",
                s.id,
                p.id
            );
        }
    }
}

#[test]
fn default_apps_rows_match_the_kinds() {
    let page = find("default-apps").unwrap();
    let mut ids: Vec<String> = crate::pages::defaultapps::KINDS
        .iter()
        .map(|k| format!("default-{}", k.id))
        .collect();
    ids.push("default-terminal".into());
    let listed: Vec<String> = page.settings.iter().map(|s| s.id.to_string()).collect();
    assert_eq!(listed, ids);
}
