//! Applying a profile and turning it off again (spec §4, K3).
//!
//! `apply` and `turn_off` work on a `State` in memory and return the next one; the caller commits
//! it. A profile remembers, per key, what it replaced (`before`) and what it wrote (`set`), so
//! "Back to normal" only puts back keys that still hold what the profile set.

use super::{
    DomainKind, Profile, ProfileError, enabled_of, key_label, kind_of, leaves, read_leaf,
    rule_label, rule_toggles, write_leaves,
};
use crate::history::{self, Entry};
use crate::settings::SettingError;
use crate::state::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// One key a profile set: what it held before (`None` = unset) and what the profile wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyMemo {
    pub domain: String,
    pub path: Vec<String>,
    pub before: Option<Value>,
    pub set: Value,
}

/// The power mode before the profile and the one it set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerMemo {
    pub before: String,
    pub set: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveProfile {
    pub id: String,
    pub name: String,
    pub since: i64,
    pub keys: Vec<KeyMemo>,
    pub power: Option<PowerMemo>,
}

/// A key "Back to normal" left alone because it was changed by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub domain: String,
    pub path: Vec<String>,
    pub label: String,
}

fn refused(e: SettingError) -> ProfileError {
    match e {
        SettingError::Invalid { key, reason } => ProfileError::Invalid { key, reason },
        SettingError::UnknownKey(key) => ProfileError::Invalid {
            key,
            reason: "not a setting uQ knows".into(),
        },
    }
}

type Write = (Vec<String>, Option<Value>);

/// Write each domain's leaves with one `write_leaves` call, domains in first-seen order.
fn write_by_domain(state: &mut State, writes: Vec<(String, Write)>) -> Result<(), ProfileError> {
    let mut grouped: Vec<(String, Vec<Write>)> = Vec::new();
    for (domain, w) in writes {
        match grouped.iter_mut().find(|(d, _)| *d == domain) {
            Some((_, list)) => list.push(w),
            None => grouped.push((domain, vec![w])),
        }
    }
    for (domain, list) in grouped {
        write_leaves(state, &domain, &list).map_err(refused)?;
    }
    Ok(())
}

/// Set every non-power key of `profile` over `current`. `power` is left for the caller.
pub fn apply(
    current: &State,
    profile: &Profile,
    now: i64,
) -> Result<(State, ActiveProfile), ProfileError> {
    // A rule switch whose rule isn't here is left out: no write and no memo.
    let keys: Vec<_> = profile
        .keys()
        .into_iter()
        .filter(|k| match kind_of(&k.domain) {
            Some(DomainKind::Power) => false,
            Some(DomainKind::Rules) => {
                matches!(k.path.as_slice(), [id] if enabled_of(current, id).is_some())
            }
            _ => true,
        })
        .collect();
    let mut next = current.clone();
    write_by_domain(
        &mut next,
        keys.iter()
            .map(|k| (k.domain.clone(), (k.path.clone(), Some(k.value.clone()))))
            .collect(),
    )?;
    let mut memos = created_leaves(current, &next, &keys);
    memos.splice(
        0..0,
        keys.into_iter().map(|k| KeyMemo {
            before: read_leaf(current, &k.domain, &k.path),
            set: read_leaf(&next, &k.domain, &k.path).unwrap_or(k.value),
            domain: k.domain,
            path: k.path,
        }),
    );
    let active = ActiveProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        since: now,
        keys: memos,
        power: None,
    };
    Ok((next, active))
}

/// Leaves the write created that the profile doesn't hold itself: the other fields of a map
/// entry it added (for example `fit` and `interval` of a new `wallpaper.outputs.<name>`), which
/// were filled in by default. They're remembered as unset, so "Back to normal" removes the whole
/// entry, and a field changed by hand since is kept (K3).
fn created_leaves(current: &State, next: &State, keys: &[super::Key]) -> Vec<KeyMemo> {
    let mut domains: Vec<&str> = Vec::new();
    for k in keys {
        if !domains.contains(&k.domain.as_str()) {
            domains.push(&k.domain);
        }
    }
    let mut out = Vec::new();
    for domain in domains {
        // A rule switch creates nothing.
        if kind_of(domain) == Some(DomainKind::Rules) {
            continue;
        }
        let (Ok(before), Ok(after)) = (current.section(domain), next.section(domain)) else {
            continue;
        };
        for (path, value) in leaves(&after) {
            let own = keys.iter().any(|k| k.domain == domain && k.path == path);
            if !own && history::get_path(&before, &path).is_none() {
                out.push(KeyMemo {
                    domain: domain.to_string(),
                    path,
                    before: None,
                    set: value,
                });
            }
        }
    }
    out
}

/// "Back to normal" (K3): put `before` back for keys that still hold what the profile set, keep
/// the rest. A key already back at `before` is neither written nor reported. Power isn't looked at.
pub fn turn_off(
    current: &State,
    active: &ActiveProfile,
) -> Result<(State, Vec<Kept>), ProfileError> {
    let mut writes = Vec::new();
    let mut kept = Vec::new();
    for m in &active.keys {
        let cur = read_leaf(current, &m.domain, &m.path);
        let rule = kind_of(&m.domain) == Some(DomainKind::Rules);
        // A rule deleted while the profile was on: nothing to put back, nothing kept.
        if rule && cur.is_none() {
            continue;
        }
        // A created leaf can be null, which `read_leaf` reads as unset.
        let still_set = cur.as_ref() == Some(&m.set) || (m.set.is_null() && cur.is_none());
        if still_set {
            writes.push((m.domain.clone(), (m.path.clone(), m.before.clone())));
        } else if cur != m.before {
            kept.push(Kept {
                label: match (rule, m.path.as_slice()) {
                    (true, [id]) => rule_label(&current.rules, id),
                    _ => key_label(&m.domain, &m.path),
                },
                domain: m.domain.clone(),
                path: m.path.clone(),
            });
        }
    }
    let mut next = current.clone();
    write_by_domain(&mut next, writes)?;
    Ok((next, kept))
}

/// True when the profile set this key and it still has that value.
pub fn holds(active: &ActiveProfile, domain: &str, path: &[String], current: &Value) -> bool {
    active
        .keys
        .iter()
        .any(|m| m.domain == domain && m.path == path && m.set == *current)
}

/// The profile key a journal entry's leaf belongs to; `None` when a profile can't hold it.
pub fn from_journal(domain: &str, path: &[String]) -> Option<(String, Vec<String>)> {
    match (domain, path) {
        ("theme", [leaf]) if leaf == "theme" || leaf == "variant" => {
            Some(("appearance".into(), path.to_vec()))
        }
        _ => matches!(
            kind_of(domain),
            Some(DomainKind::Section | DomainKind::Appearance)
        )
        .then(|| (domain.to_string(), path.to_vec())),
    }
}

/// The profile keys changed in entries at or after `since`, in first-seen order, and how many
/// changed keys a profile can't hold.
pub fn changed_since(newest_first: &[Entry], since: i64) -> (Vec<(String, Vec<String>)>, usize) {
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    let mut skipped: HashSet<(String, Vec<String>)> = HashSet::new();
    for entry in newest_first.iter().filter(|e| e.time >= since) {
        if entry.domain == "rules" {
            // Journal leaves for rules are whole lists: only a rule's switch can be a key.
            let (toggles, other) = rule_toggles(&entry.before, &entry.after);
            for id in toggles {
                let key = ("rules".to_string(), vec![id]);
                if !found.contains(&key) {
                    found.push(key);
                }
            }
            skipped.extend(other.into_iter().map(|t| ("rules".to_string(), vec![t])));
            continue;
        }
        let (before, after) = (leaves(&entry.before), leaves(&entry.after));
        let value_in = |all: &[(Vec<String>, Value)], p: &Vec<String>| {
            all.iter().find(|(q, _)| q == p).map(|(_, v)| v.clone())
        };
        let mut paths: Vec<&Vec<String>> = Vec::new();
        for (p, _) in before.iter().chain(after.iter()) {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        for p in paths {
            if value_in(&before, p) == value_in(&after, p) {
                continue;
            }
            match from_journal(&entry.domain, p) {
                Some(key) if !found.contains(&key) => found.push(key),
                Some(_) => {}
                None => {
                    skipped.insert((entry.domain.clone(), p.clone()));
                }
            }
        }
    }
    (found, skipped.len())
}

/// The profile after `active` in `ids_in_order`; `None` means "back to normal".
pub fn next_after(ids_in_order: &[String], active: Option<&str>) -> Option<String> {
    match active.and_then(|a| ids_in_order.iter().position(|i| i == a)) {
        None => ids_in_order.first().cloned(),
        Some(i) => ids_in_order.get(i + 1).cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let (next, active) = apply(&cur, &gaming(), 7).unwrap();
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
        let (next, active) = apply(&State::default(), &p, 0).unwrap();
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
        let (next, active) = apply(&State::default(), &p, 0).unwrap();
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
        let (on, active) = apply(&cur, &gaming(), 0).unwrap();
        let (off, kept) = turn_off(&on, &active).unwrap();
        assert!(kept.is_empty());
        assert_eq!(off, cur);
    }

    #[test]
    fn turn_off_keeps_hand_changed_keys_and_reports_them() {
        let (mut on, active) = apply(&base(), &gaming(), 0).unwrap();
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
        let (_, active) = apply(&cur, &gaming(), 0).unwrap();
        // An undo already put both keys back.
        let (off, kept) = turn_off(&cur, &active).unwrap();
        assert!(kept.is_empty());
        assert_eq!(off, cur);
    }

    #[test]
    fn switch_turns_first_off_then_applies_second() {
        let a = profile("a", "[look]\ngaps_in = 0\nblur = false\n");
        let b = profile("b", "[look]\nblur = true\n");
        let (mut on_a, active_a) = apply(&base(), &a, 0).unwrap();
        write_leaves(&mut on_a, "look", &[(s(&["gaps_in"]), Some(json!(3)))]).unwrap();
        let (off, kept) = turn_off(&on_a, &active_a).unwrap();
        let (on_b, active_b) = apply(&off, &b, 1).unwrap();
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
        let (on, active) = apply(&cur, &p, 0).unwrap();
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
        let (mut on, active) = apply(&cur, &p, 0).unwrap();
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
        let (_, active) = apply(&cur, &p, 0).unwrap();
        assert_eq!(active.keys.len(), 1);
    }

    #[test]
    fn holds_only_while_value_unchanged() {
        let (_, active) = apply(&base(), &gaming(), 0).unwrap();
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
        for d in ["schedule", "settings", "monitors", "input"] {
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
        let (next, active) = apply(&cur, &p, 0).unwrap();
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
        let (on, active) = apply(&cur, &p, 0).unwrap();
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
        let (on, active) = apply(&cur, &p, 0).unwrap();
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
}
