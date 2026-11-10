//! Profiles and rules (D1): a profile switches existing window and panel rules on and off, by
//! the rule's id. It can't add, delete, edit or reorder rules; workspace rules have no switch.
//!
//! `[rules]` holds `"<id>" = true|false`, one leaf per rule. Ids are random per computer, so a
//! profile's id may have no rule here: `apply` skips it (with [`missing_rules_notice`]) and the
//! file keeps it.

use super::{ActiveProfile, Key, Profile, ProfileError, holds, invalid, plain};
use crate::rules::{Rules, valid_id};
use crate::state::State;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Why a `[rules]` leaf that isn't a plain on/off is refused.
pub(super) const ONLY_ON_OFF: &str = "a profile can only turn a rule on or off";
/// Why a `[rules]` key that isn't a rule id is refused.
pub(super) const NOT_AN_ID: &str = "isn't a rule id uQ makes";

/// The rule's on/off switch: window rules first, then panel rules. `None` when no rule has `id`.
pub fn enabled_of(state: &State, id: &str) -> Option<bool> {
    let w = state.rules.window.iter().find(|r| r.id == id);
    w.map(|r| r.enabled).or_else(|| {
        state
            .rules
            .layer
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.enabled)
    })
}

/// Turn a rule on or off; false (and nothing changed) when no rule has `id`.
pub fn set_enabled(state: &mut State, id: &str, on: bool) -> bool {
    if let Some(r) = state.rules.window.iter_mut().find(|r| r.id == id) {
        r.enabled = on;
        return true;
    }
    if let Some(r) = state.rules.layer.iter_mut().find(|r| r.id == id) {
        r.enabled = on;
        return true;
    }
    false
}

/// A profile's `[rules]` table: every entry is a rule id (one segment, `rules::valid_id`) set to
/// `true` or `false`. The key in the error is `rules.<id>`, with control characters left out.
pub fn check_tree(tree: &Value) -> Result<(), ProfileError> {
    let Some(map) = tree.as_object() else {
        return Err(invalid("rules", ONLY_ON_OFF));
    };
    for (id, v) in map {
        let key = format!("rules.{}", plain(id));
        if !valid_id(id) {
            return Err(invalid(key, NOT_AN_ID));
        }
        if !v.is_boolean() {
            return Err(invalid(key, ONLY_ON_OFF));
        }
    }
    Ok(())
}

/// Every window and panel rule's switch as a profile key, in list order, window rules first.
pub fn rule_keys(rules: &Rules) -> Vec<Key> {
    let w = rules.window.iter().map(|r| (&r.id, r.enabled));
    w.chain(rules.layer.iter().map(|r| (&r.id, r.enabled)))
        .map(|(id, on)| Key {
            domain: "rules".into(),
            path: vec![id.clone()],
            value: Value::Bool(on),
        })
        .collect()
}

/// "Window rule “Name”", "Panel rule “Name”", or "A rule that isn't on this computer".
pub fn rule_label(rules: &Rules, id: &str) -> String {
    if let Some(r) = rules.window.iter().find(|r| r.id == id) {
        return format!("Window rule \u{201c}{}\u{201d}", r.name);
    }
    if let Some(r) = rules.layer.iter().find(|r| r.id == id) {
        return format!("Panel rule \u{201c}{}\u{201d}", r.name);
    }
    "A rule that isn't on this computer".into()
}

/// How many of the profile's rule switches name a rule `state` doesn't have.
pub fn missing_rules(state: &State, profile: &Profile) -> usize {
    profile
        .keys()
        .iter()
        .filter(|k| k.domain == "rules")
        .filter(|k| matches!(k.path.as_slice(), [id] if enabled_of(state, id).is_none()))
        .count()
}

/// The apply notice for `n` rules that were left out.
pub fn missing_rules_notice(n: usize) -> String {
    if n == 1 {
        "1 rule in this profile isn't on this computer, so it was left out.".into()
    } else {
        format!("{n} rules in this profile aren't on this computer, so they were left out.")
    }
}

fn list<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    v.get(name)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
}

fn text<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or("")
}

fn find<'a>(side: &'a [Value], id: &str) -> Option<&'a Value> {
    side.iter().find(|r| text(r, "id") == id)
}

/// A journal entry's `rules` change, split for "Create from history": the ids of window and
/// panel rules on both sides whose switch changed (other edits to the same rule go with it), and
/// one tag for each other change: an id (added, removed or edited), `workspace:<selector>`, or
/// `order:<list>` when rules on both sides were moved.
pub fn rule_toggles(before: &Value, after: &Value) -> (Vec<String>, Vec<String>) {
    let (mut toggles, mut other) = (Vec::new(), Vec::new());
    let mut tag = |t: String| {
        if !other.contains(&t) {
            other.push(t);
        }
    };
    for name in ["window", "layer"] {
        let (b, a) = (list(before, name), list(after, name));
        for r in a {
            let id = text(r, "id");
            match find(b, id) {
                None => tag(id.to_string()),
                Some(old) if old.get("enabled") != r.get("enabled") => toggles.push(id.to_string()),
                Some(old) if old != r => tag(id.to_string()),
                Some(_) => {}
            }
        }
        for r in b {
            if find(a, text(r, "id")).is_none() {
                tag(text(r, "id").to_string());
            }
        }
        if order_of_common(b, a, "id") {
            tag(format!("order:{name}"));
        }
    }
    let (b, a) = (list(before, "workspace"), list(after, "workspace"));
    let sel = |r: &Value| text(r, "workspace").to_string();
    let selectors: BTreeSet<String> = b.iter().chain(a).map(sel).collect();
    for s in selectors {
        let find = |side: &[Value]| side.iter().find(|r| text(r, "workspace") == s).cloned();
        if find(b) != find(a) {
            tag(format!("workspace:{s}"));
        }
    }
    if order_of_common(b, a, "workspace") {
        tag("order:workspace".into());
    }
    (toggles, other)
}

/// True when the entries on both sides are in a different order.
fn order_of_common(before: &[Value], after: &[Value], key: &str) -> bool {
    let ids = |side: &[Value], other: &[Value]| -> Vec<String> {
        side.iter()
            .map(|r| text(r, key))
            .filter(|id| other.iter().any(|o| text(o, key) == *id))
            .map(str::to_string)
            .collect()
    };
    ids(before, after) != ids(after, before)
}

/// `Explain("rules")` rows of window and panel rules whose switch still holds what the active
/// profile set: `source` becomes `profile`, with the profile's name. `error` and `user_lua` rows
/// are left as they are.
pub fn mark_rules(rows: &mut Value, rules: &Rules, active: &ActiveProfile) {
    let w = rules.window.iter().map(|r| ("window", &r.id, r.enabled));
    let all = w.chain(rules.layer.iter().map(|r| ("layer", &r.id, r.enabled)));
    for (list, id, on) in all {
        let Some(row) = rows.get_mut(format!("{list}.{id}")) else {
            continue;
        };
        if row["source"] == "uq"
            && holds(active, "rules", std::slice::from_ref(id), &Value::Bool(on))
        {
            row["source"] = json!("profile");
            row["detail"] = json!({ "name": active.name });
        }
    }
}

#[cfg(test)]
mod tests;
