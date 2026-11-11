use super::*;
use crate::profile::{bind_enabled_of, read_leaf};
use serde_json::json;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn profile(id: &str, body: &str) -> Profile {
    Profile::parse(&format!(
        "format = 1\nid = \"{id}\"\nname = \"{id}\"\n{body}"
    ))
    .unwrap()
}

fn custom(name: &str, combo: &str) -> Value {
    json!({"name": name, "combo": combo, "action": {"kind": "uq", "name": "terminal"}})
}

fn with_custom(items: Vec<Value>) -> State {
    let mut st = State::default();
    st.set_section("keybinds", json!({ "custom": items }))
        .unwrap();
    st
}

#[test]
fn apply_skips_unknown_custom_ids() {
    let cur = with_custom(vec![custom("Mine", "SUPER + F5")]);
    let id = cur.keybinds.custom[0].id.clone();
    let p = profile(
        "quiet",
        &format!("[keybinds]\n\"uq.close\" = false\n\"{id}\" = false\n\"k-ffff\" = true\n"),
    );
    let text = p.to_toml();
    let (next, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(bind_enabled_of(&next, "uq.close"), Some(false));
    assert_eq!(bind_enabled_of(&next, &id), Some(false));
    assert_eq!(bind_enabled_of(&next, "k-ffff"), None);
    // No write and no memo for the one that isn't here; the file keeps it.
    let mut memos: Vec<_> = active.keys.iter().map(|m| m.path[0].clone()).collect();
    memos.sort();
    let mut want = vec!["uq.close".to_string(), id];
    want.sort();
    assert_eq!(memos, want);
    assert_eq!(super::super::missing_binds(&cur, &p), 1);
    assert!(p.values["keybinds"].get("k-ffff").is_some());
    assert!(text.contains("k-ffff"), "{text}");
    // `Reserved` is gone from the table.
    assert!(
        crate::profile::PROFILE_DOMAINS
            .iter()
            .all(|(_, k)| *k != crate::profile::DomainKind::Reserved)
    );
}

#[test]
fn profile_conflicts_refuse_the_apply() {
    // Spotlight is off and one of your own holds Super + Space; the profile turns Spotlight on.
    let mut cur = State::default();
    cur.set_section(
        "keybinds",
        json!({"overrides": [{"id": "uq.spotlight", "enabled": false}],
               "custom": [custom("Mine", "SUPER + Space")]}),
    )
    .unwrap();
    let before = cur.clone();
    let p = profile("loud", "[keybinds]\n\"uq.spotlight\" = true\n");
    let e = apply(&cur, &p, &BTreeSet::new(), 0).unwrap_err();
    let text = e.to_string();
    assert!(
        text.contains("Mine") && text.contains("Open Spotlight"),
        "{text}"
    );
    assert!(text.contains("Super + Space"), "{text}");
    assert!(!text.contains('\n'), "{text:?}");
    assert_eq!(cur, before, "nothing changed");
}

#[test]
fn profile_conflicts_turn_off_keeps_bind_off() {
    let cur = State::default();
    let p = profile("quiet", "[keybinds]\n\"uq.close\" = false\n");
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(bind_enabled_of(&on, "uq.close"), Some(false));
    // The person then puts one of their own on Super + Q, which is free while Close is off.
    let mut hand = on.section("keybinds").unwrap();
    hand["custom"] = json!([custom("Quit it", "SUPER + Q")]);
    let mut on = on;
    on.set_section("keybinds", hand).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(bind_enabled_of(&off, "uq.close"), Some(false), "stays off");
    assert_eq!(off.keybinds, on.keybinds, "nothing else changed");
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].domain, "keybinds");
    assert_eq!(kept[0].path, s(&["uq.close"]));
    assert!(
        kept[0].label.contains("Close the window"),
        "{}",
        kept[0].label
    );
    assert!(
        kept[0]
            .label
            .contains("left off: its keys are now used by \u{201c}Quit it\u{201d}"),
        "{}",
        kept[0].label
    );
}

#[test]
fn turn_off_puts_a_free_shortcut_back_and_skips_deleted_ones() {
    let cur = with_custom(vec![custom("Mine", "SUPER + F5")]);
    let id = cur.keybinds.custom[0].id.clone();
    let p = profile(
        "quiet",
        &format!("[keybinds]\n\"uq.close\" = false\n\"{id}\" = false\n"),
    );
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    // Yours is deleted while the profile is on: nothing to put back, nothing kept.
    let mut hand = on.section("keybinds").unwrap();
    hand["custom"] = json!([]);
    let mut on = on;
    on.set_section("keybinds", hand).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert_eq!(bind_enabled_of(&off, "uq.close"), Some(true));
    assert!(off.keybinds.overrides.is_empty());
    assert_eq!(read_leaf(&off, "keybinds", &s(&[&id])), None);
}

#[test]
fn turn_off_keeps_a_switch_changed_by_hand() {
    let cur = State::default();
    let p = profile("quiet", "[keybinds]\n\"uq.close\" = false\n");
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    // The person turned it back on themselves: nothing to do and nothing to report.
    let mut hand = on.section("keybinds").unwrap();
    hand["overrides"] = json!([]);
    let mut on = on;
    on.set_section("keybinds", hand).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert_eq!(bind_enabled_of(&off, "uq.close"), Some(true));
}

#[test]
fn changed_since_offers_shortcut_switches() {
    let sect = |close: bool, extra: Vec<Value>| {
        let o: Vec<Value> = if close {
            vec![]
        } else {
            vec![json!({"id": "uq.close", "enabled": false})]
        };
        json!({"overrides": o, "custom": extra})
    };
    let entry = |time, before, after| Entry::new(time, "settings", "keybinds", "x", before, after);
    let entries = vec![
        entry(30, sect(true, vec![]), sect(false, vec![])),
        entry(
            20,
            sect(true, vec![]),
            sect(
                true,
                vec![json!({"id": "k-0001", "name": "A", "combo": "SUPER + F5"})],
            ),
        ),
    ];
    let (keys, skipped) = changed_since(&entries, 10);
    assert_eq!(keys, vec![("keybinds".to_string(), s(&["uq.close"]))]);
    assert_eq!(skipped, 1, "an added shortcut can't be in a profile");
}

#[test]
fn from_journal_has_no_keybinds_leaf() {
    // Journal leaves for shortcuts are whole lists; `bind_toggles` maps them.
    assert_eq!(from_journal("keybinds", &s(&["overrides"])), None);
    assert_eq!(from_journal("keybinds", &s(&["custom"])), None);
}
