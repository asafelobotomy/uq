//! Two shortcuts that use the same keys (R6). The finder is generic, so slice F's gestures can
//! reuse it with their own key.

use super::{Keybinds, combo};
use crate::settings::{SettingError, invalid};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// The earlier item's id, and the later one's.
    pub first: String,
    pub second: String,
    /// Their labels, in the same order.
    pub labels: (String, String),
    /// The shared keys in words. [`find`] can't know them, so it leaves this empty; the caller
    /// that knows the key fills it in.
    pub combo_text: String,
}

/// Every later item that shares a key with an earlier one, as `(key, id, label)`. A group of
/// three gives two conflicts, each against the first. Callers pass only items that are on and
/// have keys.
pub fn find<K: Ord + Clone>(items: &[(K, &str, &str)]) -> Vec<Conflict> {
    let mut first: BTreeMap<K, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (i, (key, id, label)) in items.iter().enumerate() {
        match first.get(key) {
            None => {
                first.insert(key.clone(), i);
            }
            Some(&f) => out.push(Conflict {
                first: items[f].1.to_string(),
                second: id.to_string(),
                labels: (items[f].2.to_string(), label.to_string()),
                combo_text: String::new(),
            }),
        }
    }
    out
}

/// `“Screenshot” and “Open Spotlight” both use Super + K.` (the later one first).
pub fn message(c: &Conflict) -> String {
    format!(
        "“{}” and “{}” both use {}.",
        c.labels.1, c.labels.0, c.combo_text
    )
}

/// Every conflict among the enabled, bound shortcuts of `kb`, with the keys filled in.
pub fn conflicts(kb: &Keybinds) -> Vec<Conflict> {
    let eff = kb.effective();
    let items: Vec<(combo::Combo, &str, &str)> = eff
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| Some((e.combo?, e.id.as_str(), e.label.as_str())))
        .collect();
    find(&items)
        .into_iter()
        .map(|mut c| {
            let shared = eff.iter().find(|e| e.id == c.first).and_then(|e| e.combo);
            c.combo_text = shared.map(combo::text).unwrap_or_default();
            c
        })
        .collect()
}

/// Refuses the first conflict, naming both shortcuts. The key is the later shortcut's combo, or
/// the earlier one's when the later is an untouched default.
pub fn check(kb: &Keybinds) -> Result<(), SettingError> {
    let at: Vec<usize> = (0..kb.overrides.len()).collect();
    check_at(kb, &at)
}

/// [`check`] for a list whose overrides sat at other places in what was written: `at[i]` is the
/// payload index of `kb.overrides[i]`.
pub fn check_at(kb: &Keybinds, at: &[usize]) -> Result<(), SettingError> {
    let Some(c) = conflicts(kb).into_iter().next() else {
        return Ok(());
    };
    let custom = |id: &str| kb.custom.iter().position(|b| b.id == id);
    let over = |id: &str| kb.overrides.iter().position(|o| o.id == id);
    let key = [&c.second, &c.first]
        .into_iter()
        .find_map(|id| {
            custom(id)
                .map(|i| format!("keybinds.custom[{i}].combo"))
                .or_else(|| {
                    over(id).map(|i| {
                        let i = at.get(i).copied().unwrap_or(i);
                        format!("keybinds.overrides[{i}].combo")
                    })
                })
        })
        .unwrap_or_else(|| "keybinds".into());
    Err(invalid(&key, message(&c)))
}
