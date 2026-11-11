//! Profiles and keyboard shortcuts (R15): a profile switches existing shortcuts on and off, by
//! id. It can't set keys, add a shortcut or carry a command, so an imported profile never adds a
//! command (spec §4).
//!
//! `[keybinds]` holds `"<id>" = true|false`, one leaf per shortcut: one of uQ's (`uq.spotlight`)
//! or one of your own (`k-3f9c`). Ids of your own are random per computer, so a profile's id may
//! have no shortcut here: `apply` skips it (with [`missing_binds_notice`]) and the file keeps it.

use super::{ActiveProfile, Key, Profile, ProfileError, holds, invalid, plain};
use crate::keybinds::check::RECOVERY_REFUSED;
use crate::keybinds::defaults::{self, LOCK_RECOVERY};
use crate::keybinds::{Keybinds, Override, conflict, valid_custom_id};
use crate::settings::SettingError;
use crate::state::State;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Why a `[keybinds]` leaf that isn't a plain on/off is refused.
pub(super) const ONLY_ON_OFF: &str = "a profile can only turn a shortcut on or off";
/// Why a `[keybinds]` key that isn't a shortcut id is refused.
pub(super) const NOT_AN_ID: &str = "isn't one of uQ's shortcuts";

/// Whether the shortcut `id` is on: one of uQ's unless an override turned it off, or one of your
/// own by its switch. `None` when no shortcut has `id`.
pub fn enabled_of(state: &State, id: &str) -> Option<bool> {
    enabled_in(&state.keybinds, id)
}

fn enabled_in(kb: &Keybinds, id: &str) -> Option<bool> {
    if defaults::default_of(id).is_some() {
        let o = kb.overrides.iter().find(|o| o.id == id);
        return Some(o.and_then(|o| o.enabled).unwrap_or(true));
    }
    kb.custom.iter().find(|b| b.id == id).map(|b| b.enabled)
}

/// Turn a shortcut on or off; false (and nothing changed) when no shortcut has `id`. For one of
/// uQ's it writes or removes the override's `enabled`, and an override left with nothing in it
/// goes. This doesn't check the result: [`write_leaves`] does.
pub fn set_enabled(state: &mut State, id: &str, on: bool) -> bool {
    set_enabled_in(&mut state.keybinds, id, on)
}

pub(super) fn set_enabled_in(kb: &mut Keybinds, id: &str, on: bool) -> bool {
    if defaults::default_of(id).is_some() {
        match kb.overrides.iter().position(|o| o.id == id) {
            Some(i) => {
                kb.overrides[i].enabled = (!on).then_some(false);
                if kb.overrides[i].combo.is_none() && kb.overrides[i].enabled.is_none() {
                    kb.overrides.remove(i);
                }
            }
            None if !on => kb.overrides.push(Override {
                id: id.into(),
                combo: None,
                enabled: Some(false),
            }),
            None => {}
        }
        return true;
    }
    match kb.custom.iter_mut().find(|b| b.id == id) {
        Some(b) => {
            b.enabled = on;
            true
        }
        None => false,
    }
}

/// A profile's `[keybinds]` table: every entry is a shortcut id (one of uQ's, or `k-` and 4 to 8
/// hex digits) set to `true` or `false`. The lock recovery shortcut can't be set to `false`
/// (R8). The key in an error is `keybinds.<id>`, with control characters left out.
pub fn check_tree(tree: &Value) -> Result<(), ProfileError> {
    let Some(map) = tree.as_object() else {
        return Err(invalid("keybinds", ONLY_ON_OFF));
    };
    for (id, v) in map {
        let key = format!("keybinds.{}", plain(id));
        if defaults::default_of(id).is_none() && !valid_custom_id(id) {
            return Err(invalid(key, NOT_AN_ID));
        }
        if !v.is_boolean() {
            return Err(invalid(key, ONLY_ON_OFF));
        }
        if id == LOCK_RECOVERY && *v == Value::Bool(false) {
            return Err(invalid(key, RECOVERY_REFUSED));
        }
    }
    Ok(())
}

/// Set shortcut switches through the same checks a write makes ([`Keybinds::from_section`]), so a
/// switch that would put two shortcuts on the same keys is refused with both names. A non-bool,
/// a removal, or an id with no shortcut is refused too (callers leave out ids that aren't
/// here); on an error `state` is unchanged.
pub(super) fn write_leaves(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut next = state.keybinds.clone();
    let mut first = None;
    for (path, value) in leaves {
        let key = format!("keybinds.{}", plain(&path.join(".")));
        let refuse = |reason: &str| SettingError::Invalid {
            key: key.clone(),
            reason: reason.into(),
        };
        let ([id], Some(Value::Bool(on))) = (path.as_slice(), value) else {
            return Err(refuse(ONLY_ON_OFF));
        };
        if !set_enabled_in(&mut next, id, *on) {
            return Err(refuse("isn't a shortcut on this computer"));
        }
        first.get_or_insert_with(|| id.clone());
    }
    let section = serde_json::to_value(&next).expect("shortcuts serialise");
    match Keybinds::from_section(&section, &state.keybinds) {
        Ok(checked) => {
            state.keybinds = checked;
            Ok(())
        }
        // Named by the shortcut the profile set, not by where it sits in the list.
        Err(SettingError::Invalid { reason, .. }) => Err(SettingError::Invalid {
            key: format!("keybinds.{}", plain(&first.unwrap_or_default())),
            reason,
        }),
        Err(e) => Err(e),
    }
}

/// Every shortcut's switch as a profile key: uQ's in table order, then your own.
pub fn bind_keys(kb: &Keybinds) -> Vec<Key> {
    kb.effective()
        .into_iter()
        .map(|e| Key {
            domain: "keybinds".into(),
            path: vec![e.id],
            value: Value::Bool(e.enabled),
        })
        .collect()
}

/// The name of a shortcut: uQ's own label, `Shortcut “<name>”` for one of yours, or
/// "A shortcut that isn't on this computer".
pub fn bind_label(kb: &Keybinds, id: &str) -> String {
    if let Some(d) = defaults::default_of(id) {
        return d.label.to_string();
    }
    match kb.custom.iter().find(|b| b.id == id) {
        Some(b) => format!("Shortcut \u{201c}{}\u{201d}", plain(&b.name)),
        None => "A shortcut that isn't on this computer".into(),
    }
}

/// How many of the profile's shortcut switches name a shortcut `state` doesn't have.
pub fn missing_binds(state: &State, profile: &Profile) -> usize {
    profile
        .keys()
        .iter()
        .filter(|k| k.domain == "keybinds")
        .filter(|k| matches!(k.path.as_slice(), [id] if enabled_of(state, id).is_none()))
        .count()
}

/// The apply notice for `n` shortcuts that were left out.
pub fn missing_binds_notice(n: usize) -> String {
    if n == 1 {
        "1 shortcut in this profile isn't on this computer, so it was left out.".into()
    } else {
        format!("{n} shortcuts in this profile aren't on this computer, so they were left out.")
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

fn on(item: &Value) -> bool {
    item.get("enabled").and_then(Value::as_bool) != Some(false)
}

/// A journal entry's `keybinds` change, split for "Create from history": the ids of shortcuts
/// whose switch changed (one of uQ's, or one of your own present on both sides), and one tag for
/// each other change: an id (new keys for one of uQ's, one of yours added, removed or edited) or
/// `order:custom` when yours were moved.
pub fn bind_toggles(before: &Value, after: &Value) -> (Vec<String>, Vec<String>) {
    let (mut toggles, mut other) = (Vec::new(), Vec::new());
    let mut tag = |t: String| {
        if !other.contains(&t) {
            other.push(t);
        }
    };
    let (bo, ao) = (list(before, "overrides"), list(after, "overrides"));
    let ids: BTreeSet<&str> = bo.iter().chain(ao).map(|o| text(o, "id")).collect();
    for id in ids {
        let (b, a) = (find(bo, id), find(ao, id));
        let was = b.is_none_or(on);
        let now = a.is_none_or(on);
        let keys = |o: Option<&Value>| o.and_then(|o| o.get("combo")).cloned();
        if keys(b) != keys(a) {
            tag(id.to_string());
        }
        if was != now {
            toggles.push(id.to_string());
        }
    }
    let (bc, ac) = (list(before, "custom"), list(after, "custom"));
    for c in ac {
        let id = text(c, "id");
        match find(bc, id) {
            None => tag(id.to_string()),
            Some(old) if on(old) != on(c) => toggles.push(id.to_string()),
            Some(old) if old != c => tag(id.to_string()),
            Some(_) => {}
        }
    }
    for c in bc {
        if find(ac, text(c, "id")).is_none() {
            tag(text(c, "id").to_string());
        }
    }
    let ids = |side: &[Value], other: &[Value]| -> Vec<String> {
        side.iter()
            .map(|r| text(r, "id"))
            .filter(|id| other.iter().any(|o| text(o, "id") == *id))
            .map(str::to_string)
            .collect()
    };
    if ids(bc, ac) != ids(ac, bc) {
        tag("order:custom".into());
    }
    (toggles, other)
}

/// `Explain("keybinds")` rows whose switch still holds what the active profile set: `source`
/// becomes `profile`, with the profile's name. Only rows that say `uq` (changed) or `default`
/// are marked; `error`, `user_lua` and `runtime` rows are left as they are.
pub fn mark_binds(rows: &mut Value, kb: &Keybinds, active: &ActiveProfile) {
    for e in kb.effective() {
        let Some(row) = rows.get_mut(format!("binds.{}", e.id)) else {
            continue;
        };
        if matches!(row["source"].as_str(), Some("uq" | "default"))
            && holds(
                active,
                "keybinds",
                std::slice::from_ref(&e.id),
                &Value::Bool(e.enabled),
            )
        {
            row["source"] = json!("profile");
            row["detail"] = json!({ "name": active.name });
        }
    }
}

/// What turning shortcut `id` back on would collide with in `kb`: the label of the other shortcut
/// that already uses its keys, or `None` when it is free.
pub(super) fn clash_if_on(kb: &Keybinds, id: &str) -> Option<String> {
    let mut next = kb.clone();
    set_enabled_in(&mut next, id, true);
    conflict::conflicts(&next)
        .into_iter()
        .find_map(|c| match (c.first == id, c.second == id) {
            (true, _) => Some(c.labels.1),
            (_, true) => Some(c.labels.0),
            _ => None,
        })
}

#[cfg(test)]
mod tests;
