//! Keyboard shortcuts: Hyprland's names, uQ's key list, the closed action set, the default
//! table and the `keybinds` domain (overrides of the defaults, and shortcuts of your own).

pub mod actions;
pub mod check;
pub mod combo;
pub mod conflict;
pub mod defaults;
pub mod explain;
pub mod hypr;
pub mod keys;
pub mod list;
mod summary;

pub use actions::Action;
pub use combo::{Combo, ComboError};
pub use explain::{LiveBind, explain, live_binds};
pub use summary::summary;

use crate::rules::fresh_prefixed;
use crate::settings::{SettingError, check_known, invalid};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MAX_CUSTOM: usize = 100;
pub const MAX_NAME_CHARS: usize = 100;

fn yes() -> bool {
    true
}

/// A change to one of uQ's own shortcuts: other keys, or turned off. Absent fields mean the
/// default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// A shortcut of your own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomBind {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub combo: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub repeating: bool,
    pub action: Action,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Keybinds {
    pub overrides: Vec<Override>,
    pub custom: Vec<CustomBind>,
}

/// One shortcut as it renders: a default with its override applied, or one of your own.
#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    pub id: String,
    pub label: String,
    pub combo: Option<Combo>,
    pub enabled: bool,
    pub lua_action: String,
    pub locked: bool,
    pub repeating: bool,
    pub release: bool,
    pub mouse: bool,
    pub is_default: bool,
}

/// Loads entry by entry: one that doesn't parse (a newer uQ's, or a hand edit) is dropped with a
/// log line and the others are kept.
impl<'de> Deserialize<'de> for Keybinds {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Keybinds, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(Keybinds {
            overrides: lenient(&v, "overrides"),
            custom: lenient(&v, "custom"),
        })
    }
}

fn lenient<T: DeserializeOwned>(v: &Value, list: &str) -> Vec<T> {
    match v.get(list) {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match serde_json::from_value(e.clone()) {
                Ok(x) => Some(x),
                Err(_) => {
                    tracing::warn!(
                        "state: dropping keybinds.{list}[{i}]: it doesn't fit this version"
                    );
                    None
                }
            })
            .collect(),
        Some(_) => {
            tracing::warn!("state: keybinds.{list} is not a list; ignoring it");
            Vec::new()
        }
    }
}

/// `^k-[0-9a-f]{4,8}$`
pub fn valid_custom_id(id: &str) -> bool {
    id.strip_prefix("k-").is_some_and(|h| {
        (4..=8).contains(&h.len()) && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn list<T: DeserializeOwned>(
    v: &Value,
    name: &str,
    max: usize,
    too_many: &str,
    known: &[&str],
) -> Result<Vec<T>, SettingError> {
    let key = format!("keybinds.{name}");
    let items = match v.get(name) {
        None => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(invalid(&key, "expected a list")),
    };
    if items.len() > max {
        return Err(invalid(&key, too_many));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let k = format!("{key}[{i}]");
            check_known(&k, e, known)?;
            if let Some(a) = e.get("action") {
                let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
                let fields: &[&str] = match kind {
                    "shell" => &["kind", "name"],
                    "window" => &["kind", "name", "direction"],
                    "workspace" => &["kind", "name", "workspace"],
                    "uq" => &["kind", "name", "layout", "media"],
                    "launch" => &["kind", "app"],
                    "command" => &["kind", "command"],
                    _ => &["kind"],
                };
                check_known(&format!("{k}.action"), a, fields)?;
            }
            serde_json::from_value(e.clone())
                .map_err(|_| invalid(&k, "a value is missing or of the wrong kind"))
        })
        .collect()
}

impl Keybinds {
    /// The ids of your own shortcuts.
    pub fn ids(&self) -> Vec<&str> {
        self.custom.iter().map(|b| b.id.as_str()).collect()
    }

    /// A whole-section write: strict. `current` only steers fresh ids; ids that exist now are
    /// accepted again (every write resends them), and ids must be unique within `v`.
    pub fn from_section(v: &Value, current: &Keybinds) -> Result<Keybinds, SettingError> {
        const KEY: &str = "keybinds";
        if !v.is_object() {
            return Err(invalid(KEY, "expected an object"));
        }
        check_known(KEY, v, &["overrides", "custom"])?;
        let raw: Vec<Override> = list(
            v,
            "overrides",
            defaults::defaults().len(),
            "too many changes to uQ's shortcuts",
            &["id", "combo", "enabled"],
        )?;
        let mut overrides = Vec::new();
        // Where each kept override sat in the payload: a no-op before it shifts the list, but
        // an error key must name the entry the caller wrote.
        let mut at = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, o) in raw.iter().enumerate() {
            if !seen.insert(o.id.clone()) {
                return Err(invalid(
                    &format!("keybinds.overrides[{i}].id"),
                    "this shortcut is already changed",
                ));
            }
            if let Some(o) = check::override_(i, o)? {
                overrides.push(o);
                at.push(i);
            }
        }
        let mut custom: Vec<CustomBind> = list(
            v,
            "custom",
            MAX_CUSTOM,
            "at most 100 shortcuts of your own",
            &[
                "id",
                "name",
                "combo",
                "enabled",
                "locked",
                "repeating",
                "action",
            ],
        )?;
        let mut ids = BTreeSet::new();
        for (i, b) in custom.iter_mut().enumerate() {
            check::custom(i, b)?;
            if b.id.is_empty() {
                continue;
            }
            let key = format!("keybinds.custom[{i}].id");
            if !valid_custom_id(&b.id) {
                return Err(invalid(&key, "an id looks like k-1a2b"));
            }
            if !ids.insert(b.id.clone()) {
                return Err(invalid(&key, "another shortcut has this id"));
            }
        }
        ids.extend(current.ids().into_iter().map(String::from));
        for b in &mut custom {
            if b.id.is_empty() {
                b.id = fresh_prefixed("k-", &ids);
                ids.insert(b.id.clone());
            }
        }
        let out = Keybinds { overrides, custom };
        conflict::check_at(&out, &at)?;
        Ok(out)
    }

    /// Load-time repair, so the section stays writable: entries that break a rule are dropped
    /// with a log line, a bad or repeated id is made again, and of two shortcuts on the same
    /// keys the earlier keeps them.
    pub fn repaired(mut self) -> Keybinds {
        let mut seen = BTreeSet::new();
        let mut kept = Vec::new();
        for (i, o) in self.overrides.iter().enumerate() {
            let mut o = o.clone();
            if o.id == defaults::LOCK_RECOVERY {
                o.enabled = None;
            }
            match check::override_(i, &o) {
                Ok(Some(n)) if seen.insert(n.id.clone()) => kept.push(n),
                Ok(_) => {}
                Err(e) => tracing::warn!("state: dropping keybinds.overrides[{i}]: {e}"),
            }
        }
        self.overrides = kept;
        let mut custom = Vec::new();
        for (i, mut b) in std::mem::take(&mut self.custom).into_iter().enumerate() {
            match check::custom(i, &mut b) {
                Ok(()) => custom.push(b),
                Err(e) => tracing::warn!("state: dropping keybinds.custom[{i}]: {e}"),
            }
        }
        custom.truncate(MAX_CUSTOM);
        let mut ids = BTreeSet::new();
        for b in &mut custom {
            if !b.id.is_empty() && !(valid_custom_id(&b.id) && ids.insert(b.id.clone())) {
                tracing::warn!("state: a shortcut has a bad or repeated id; it gets a new one");
                b.id.clear();
            }
        }
        for b in &mut custom {
            if b.id.is_empty() {
                b.id = fresh_prefixed("k-", &ids);
                ids.insert(b.id.clone());
            }
        }
        self.custom = custom;
        // Drop the later of two shortcuts on the same keys until none are left. Dropping an
        // override puts a default back on its own keys, so go round again.
        while let Some(c) = conflict::conflicts(&self).into_iter().next() {
            tracing::warn!("state: dropping a shortcut: {}", conflict::message(&c));
            if let Some(i) = self.custom.iter().position(|b| b.id == c.second) {
                self.custom.remove(i);
            } else if let Some(i) = self.overrides.iter().position(|o| o.id == c.second) {
                self.overrides.remove(i);
            } else if let Some(i) = self.overrides.iter().position(|o| o.id == c.first) {
                self.overrides.remove(i);
            } else {
                break;
            }
        }
        self
    }

    /// Every shortcut as rendered: the defaults in table order with overrides applied, then your
    /// own in list order.
    pub fn effective(&self) -> Vec<Effective> {
        let mut out = Vec::new();
        for d in defaults::defaults() {
            let o = self.overrides.iter().find(|o| o.id == d.id);
            let text = o.and_then(|o| o.combo.as_deref()).or(d.combo);
            out.push(Effective {
                id: d.id.into(),
                label: d.label.into(),
                combo: text.and_then(|t| combo::parse(t).ok()),
                enabled: o.and_then(|o| o.enabled).unwrap_or(true),
                lua_action: d.action.lua(),
                locked: d.locked,
                repeating: d.repeating,
                release: d.release,
                mouse: d.mouse,
                is_default: true,
            });
        }
        for b in &self.custom {
            out.push(Effective {
                id: b.id.clone(),
                label: b.name.clone(),
                combo: combo::parse(&b.combo).ok(),
                enabled: b.enabled,
                lua_action: b.action.lua(),
                locked: b.locked,
                repeating: b.repeating,
                release: false,
                mouse: false,
                is_default: false,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests;
