use super::*;
use serde_json::json;
use std::collections::BTreeSet;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn profile(id: &str, body: &str) -> Profile {
    Profile::parse(&format!(
        "format = 1\nid = \"{id}\"\nname = \"{id}\"\n{body}"
    ))
    .unwrap()
}

fn base() -> State {
    let mut st = State::default();
    write_leaves(&mut st, "look", &[(s(&["gaps_in"]), Some(json!(5)))]).unwrap();
    st
}

fn gaming() -> Profile {
    profile("gaming", "[look]\ngaps_in = 0\nblur = false\n")
}

#[test]
fn apply_sets_only_profile_keys_and_remembers_before() {
    let cur = base();
    let (next, active) = apply(&cur, &gaming(), &BTreeSet::new(), 7).unwrap();
    assert_eq!(read_leaf(&next, "look", &s(&["gaps_in"])), Some(json!(0)));
    assert_eq!(read_leaf(&next, "look", &s(&["blur"])), Some(json!(false)));
    assert_eq!(
        (active.id.as_str(), active.since, &active.power),
        ("gaming", 7, &None)
    );
    let memo = |p: &str| {
        active
            .keys
            .iter()
            .find(|m| m.path == s(&[p]))
            .unwrap()
            .clone()
    };
    assert_eq!(memo("gaps_in").before, Some(json!(5)));
    assert_eq!(memo("gaps_in").set, json!(0));
    assert_eq!(memo("blur").before, None);
    let mut look_cur = cur.section("look").unwrap();
    let mut look_next = next.section("look").unwrap();
    for v in [&mut look_cur, &mut look_next] {
        let o = v.as_object_mut().unwrap();
        o.remove("gaps_in");
        o.remove("blur");
    }
    assert_eq!(look_cur, look_next);
    assert_eq!(
        cur.section("shell").unwrap(),
        next.section("shell").unwrap()
    );
}

#[test]
fn apply_records_normalised_value() {
    let p = profile("slow", "[look]\nanimation_speed = 1.234\n");
    let (next, active) = apply(&State::default(), &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(active.keys[0].set, json!(1.23));
    assert_eq!(
        read_leaf(&next, "look", &s(&["animation_speed"])),
        Some(json!(1.23))
    );
}

#[test]
fn apply_theme_and_variant_through_appearance() {
    let p = profile(
        "night",
        "[appearance]\ntheme = \"nord\"\nvariant = \"light\"\n",
    );
    let (next, active) = apply(&State::default(), &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(next.theme, "nord");
    assert_eq!(next.variant, crate::theme::Variant::Light);
    let memo = |p: &str| {
        active
            .keys
            .iter()
            .find(|m| m.path == s(&[p]))
            .unwrap()
            .clone()
    };
    assert_eq!(memo("theme").set, json!("nord"));
    assert_eq!(memo("variant").before, Some(json!("dark")));
}

#[test]
fn turn_off_restores_untouched_keys() {
    let cur = base();
    let (on, active) = apply(&cur, &gaming(), &BTreeSet::new(), 0).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty());
    assert_eq!(off, cur);
}

#[test]
fn turn_off_keeps_hand_changed_keys_and_reports_them() {
    let (mut on, active) = apply(&base(), &gaming(), &BTreeSet::new(), 0).unwrap();
    write_leaves(&mut on, "look", &[(s(&["gaps_in"]), Some(json!(3)))]).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(read_leaf(&off, "look", &s(&["gaps_in"])), Some(json!(3)));
    assert_eq!(read_leaf(&off, "look", &s(&["blur"])), None);
    assert_eq!(
        kept,
        vec![Kept {
            domain: "look".into(),
            path: s(&["gaps_in"]),
            label: "Gaps between windows".into()
        }]
    );
}

#[test]
fn turn_off_skips_keys_already_back() {
    let cur = base();
    let (_, active) = apply(&cur, &gaming(), &BTreeSet::new(), 0).unwrap();
    // An undo already put both keys back.
    let (off, kept) = turn_off(&cur, &active).unwrap();
    assert!(kept.is_empty());
    assert_eq!(off, cur);
}

#[test]
fn switch_turns_first_off_then_applies_second() {
    let a = profile("a", "[look]\ngaps_in = 0\nblur = false\n");
    let b = profile("b", "[look]\nblur = true\n");
    let (mut on_a, active_a) = apply(&base(), &a, &BTreeSet::new(), 0).unwrap();
    write_leaves(&mut on_a, "look", &[(s(&["gaps_in"]), Some(json!(3)))]).unwrap();
    let (off, kept) = turn_off(&on_a, &active_a).unwrap();
    let (on_b, active_b) = apply(&off, &b, &BTreeSet::new(), 1).unwrap();
    assert_eq!(read_leaf(&on_b, "look", &s(&["gaps_in"])), Some(json!(3)));
    assert_eq!(read_leaf(&on_b, "look", &s(&["blur"])), Some(json!(true)));
    assert_eq!(
        kept.iter().map(|k| k.path.clone()).collect::<Vec<_>>(),
        vec![s(&["gaps_in"])]
    );
    assert_eq!(active_b.keys.len(), 1);
    assert_eq!(active_b.keys[0].before, None);
}

fn wallpaper_base() -> State {
    let mut st = State::default();
    write_leaves(
        &mut st,
        "wallpaper",
        &[(s(&["outputs", "*", "image"]), Some(json!("/w/a.png")))],
    )
    .unwrap();
    st
}

#[test]
fn turn_off_removes_a_map_entry_the_profile_created() {
    // The review's probe: the profile holds only one leaf of a display's entry, so applying
    // it fills in `fit` and `interval` by default. Back to normal must remove all of it.
    let cur = wallpaper_base();
    let p = profile(
        "work",
        "[wallpaper.outputs.HDMI-A-1]\nimage = \"/w/b.png\"\n",
    );
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(
        read_leaf(&on, "wallpaper", &s(&["outputs", "HDMI-A-1", "fit"])),
        Some(json!("fill"))
    );
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert_eq!(
        off.section("wallpaper").unwrap(),
        cur.section("wallpaper").unwrap()
    );
    assert_eq!(off, cur);
}

#[test]
fn turn_off_keeps_a_hand_changed_leaf_of_a_created_entry() {
    let cur = wallpaper_base();
    let p = profile(
        "work",
        "[wallpaper.outputs.HDMI-A-1]\nimage = \"/w/b.png\"\n",
    );
    let (mut on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    let fit = s(&["outputs", "HDMI-A-1", "fit"]);
    write_leaves(&mut on, "wallpaper", &[(fit.clone(), Some(json!("tile")))]).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(read_leaf(&off, "wallpaper", &fit), Some(json!("tile")));
    assert_eq!(
        read_leaf(&off, "wallpaper", &s(&["outputs", "HDMI-A-1", "image"])),
        None
    );
    assert_eq!(
        kept.iter().map(|k| k.path.clone()).collect::<Vec<_>>(),
        vec![fit]
    );
}

#[test]
fn apply_remembers_nothing_extra_for_an_existing_entry() {
    let cur = wallpaper_base();
    let p = profile("work", "[wallpaper.outputs.\"*\"]\nimage = \"/w/b.png\"\n");
    let (_, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(active.keys.len(), 1);
}

#[test]
fn holds_only_while_value_unchanged() {
    let (_, active) = apply(&base(), &gaming(), &BTreeSet::new(), 0).unwrap();
    assert!(holds(&active, "look", &s(&["gaps_in"]), &json!(0)));
    assert!(!holds(&active, "look", &s(&["gaps_in"]), &json!(3)));
    assert!(!holds(&active, "look", &s(&["gaps_out"]), &json!(0)));
    assert!(!holds(&active, "shell", &s(&["gaps_in"]), &json!(0)));
}

fn entry(time: i64, domain: &str, before: Value, after: Value) -> Entry {
    Entry::new(time, "settings", domain, "", before, after)
}

#[test]
fn changed_since_maps_theme_and_skips_other_domains() {
    let entries = vec![
        entry(30, "look", json!({"blur": true}), json!({"blur": false})),
        entry(
            20,
            "theme",
            json!({"theme": "graphite", "variant": "dark"}),
            json!({"theme": "graphite", "variant": "light"}),
        ),
        entry(
            15,
            "settings",
            json!({"idle": {"dim": 1}}),
            json!({"idle": {"dim": 2}}),
        ),
        entry(5, "look", json!({"gaps_in": 1}), json!({"gaps_in": 2})),
    ];
    let (keys, skipped) = changed_since(&entries, 10);
    assert_eq!(
        keys,
        vec![
            ("look".to_string(), s(&["blur"])),
            ("appearance".to_string(), s(&["variant"])),
        ]
    );
    assert_eq!(skipped, 1);
}

#[test]
fn from_journal_maps_domains() {
    assert_eq!(
        from_journal("theme", &s(&["theme"])),
        Some(("appearance".into(), s(&["theme"])))
    );
    assert_eq!(
        from_journal("appearance", &s(&["ui_font"])).unwrap().0,
        "appearance"
    );
    for d in [
        "schedule",
        "settings",
        "monitors",
        "input",
        "env",
        "compat",
        "autostart",
    ] {
        assert_eq!(from_journal(d, &s(&["x"])), None, "{d}");
    }
    // Journal leaves for rules are whole lists; `rule_toggles` maps them.
    assert_eq!(from_journal("rules", &s(&["window"])), None);
}

fn rule(id: &str, name: &str, on: bool) -> Value {
    json!({"id": id, "name": name, "enabled": on, "match": {"class": "a"},
        "effects": {"float": true}})
}

fn rules_state(rules: Vec<Value>) -> State {
    let mut st = State::default();
    st.set_section("rules", json!({ "window": rules })).unwrap();
    st
}

fn enabled(st: &State, id: &str) -> Option<bool> {
    super::super::enabled_of(st, id)
}

#[test]
fn apply_skips_unknown_rule_ids() {
    let cur = rules_state(vec![rule("r-000a", "A", true)]);
    let p = profile("rules", "[rules]\n\"r-000a\" = false\n\"r-ffff\" = true\n");
    let text = p.to_toml();
    let (next, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(enabled(&next, "r-000a"), Some(false));
    assert_eq!(enabled(&next, "r-ffff"), None);
    assert_eq!(
        active.keys,
        vec![KeyMemo {
            domain: "rules".into(),
            path: s(&["r-000a"]),
            before: Some(json!(true)),
            set: json!(false),
        }]
    );
    assert_eq!(super::super::missing_rules(&cur, &p), 1);
    assert_eq!(p.to_toml(), text);
    assert!(p.values["rules"].get("r-ffff").is_some());
}

#[test]
fn turn_off_rules_k3() {
    let cur = rules_state(vec![
        rule("r-000a", "A", true),
        rule("r-000b", "B", true),
        rule("r-000c", "C", true),
    ]);
    let p = profile(
        "rules",
        "[rules]\n\"r-000a\" = false\n\"r-000b\" = true\n\"r-000c\" = false\n",
    );
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    assert_eq!(enabled(&on, "r-000a"), Some(false));
    assert_eq!(enabled(&on, "r-000c"), Some(false));
    // By hand: B switched off, C deleted, A renamed.
    let mut hand = on.section("rules").unwrap();
    hand["window"][1]["enabled"] = json!(false);
    hand["window"][0]["name"] = json!("A renamed");
    hand["window"].as_array_mut().unwrap().remove(2);
    let mut on = on;
    on.set_section("rules", hand).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(enabled(&off, "r-000a"), Some(true));
    assert_eq!(off.rules.window[0].name, "A renamed");
    assert_eq!(enabled(&off, "r-000b"), Some(false));
    assert_eq!(enabled(&off, "r-000c"), None);
    assert_eq!(
        kept,
        vec![Kept {
            domain: "rules".into(),
            path: s(&["r-000b"]),
            label: "Window rule \u{201c}B\u{201d}".into(),
        }]
    );
    // The literal K3 case: a rule the profile turned off, turned on and off again by hand,
    // still holds what the profile set, so it goes back on and nothing is kept.
    let cur = rules_state(vec![rule("r-000a", "A", true)]);
    let p = profile("rules", "[rules]\n\"r-000a\" = false\n");
    let (on, active) = apply(&cur, &p, &BTreeSet::new(), 0).unwrap();
    let mut on = on;
    for by_hand in [true, false] {
        let mut hand = on.section("rules").unwrap();
        hand["window"][0]["enabled"] = json!(by_hand);
        on.set_section("rules", hand).unwrap();
    }
    assert_eq!(enabled(&on, "r-000a"), Some(false));
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(enabled(&off, "r-000a"), Some(true));
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn changed_since_offers_rule_toggles() {
    let sect = |rules: Vec<Value>| json!({"window": rules, "layer": [], "workspace": []});
    let entries = vec![
        entry(
            30,
            "rules",
            sect(vec![rule("r-000a", "A", true)]),
            sect(vec![rule("r-000a", "A", false)]),
        ),
        entry(
            20,
            "rules",
            sect(vec![]),
            sect(vec![rule("r-000a", "A", true)]),
        ),
    ];
    let (keys, skipped) = changed_since(&entries, 10);
    assert_eq!(keys, vec![("rules".to_string(), s(&["r-000a"]))]);
    assert_eq!(skipped, 1);
}

#[test]
fn next_after_cycles_through_then_back_to_normal() {
    let ids = s(&["a", "b", "c"]);
    assert_eq!(next_after(&ids, None), Some("a".into()));
    assert_eq!(next_after(&ids, Some("gone")), Some("a".into()));
    assert_eq!(next_after(&ids, Some("a")), Some("b".into()));
    assert_eq!(next_after(&ids, Some("c")), None);
    assert_eq!(next_after(&[], None), None);
}
