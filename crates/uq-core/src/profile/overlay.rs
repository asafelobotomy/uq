//! Applying a profile and turning it off again (spec §4, K3).
//!
//! `apply` and `turn_off` work on a `State` in memory and return the next one; the caller commits
//! it. A profile remembers, per key, what it replaced (`before`) and what it wrote (`set`), so
//! "Back to normal" only puts back keys that still hold what the profile set.

use super::{
    DomainKind, Profile, ProfileError, key_label, kind_of, leaves, read_leaf, write_leaves,
};
use crate::history::Entry;
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
    let keys: Vec<_> = profile
        .keys()
        .into_iter()
        .filter(|k| kind_of(&k.domain) != Some(DomainKind::Power))
        .collect();
    let mut next = current.clone();
    write_by_domain(
        &mut next,
        keys.iter()
            .map(|k| (k.domain.clone(), (k.path.clone(), Some(k.value.clone()))))
            .collect(),
    )?;
    let memos = keys
        .into_iter()
        .map(|k| KeyMemo {
            before: read_leaf(current, &k.domain, &k.path),
            set: read_leaf(&next, &k.domain, &k.path).unwrap_or(k.value),
            domain: k.domain,
            path: k.path,
        })
        .collect();
    let active = ActiveProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        since: now,
        keys: memos,
        power: None,
    };
    Ok((next, active))
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
        if cur.as_ref() == Some(&m.set) {
            writes.push((m.domain.clone(), (m.path.clone(), m.before.clone())));
        } else if cur != m.before {
            kept.push(Kept {
                label: key_label(&m.domain, &m.path),
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
