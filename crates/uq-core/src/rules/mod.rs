//! Window, panel and workspace rules.
//!
//! The domain model (`Rules`), its strict write-time validation and its lenient load-time
//! repair. Hyprland's names live in [`hypr`]; patterns and value checks in `check`.

mod check;
pub mod hypr;
mod matcher;
mod templates;
mod words;

pub use check::check_pattern;
pub use matcher::{Compiled, Verdict, WindowInfo, matching, window_at, windows_from_clients};
pub use templates::{Needs, TEMPLATES, Template, build as build_template};
pub use words::{effects_text, has_static, layer_effects_text, match_text, workspace_text};

use crate::settings::{SettingError, check_known, invalid};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::hash::{BuildHasher, Hasher};

pub const MAX_WINDOW_RULES: usize = 200;
pub const MAX_LAYER_RULES: usize = 50;
pub const MAX_WORKSPACE_RULES: usize = 64;
pub const MAX_NAME_CHARS: usize = 100;
pub use check::MAX_PATTERN_CHARS;

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(rename = "match", default)]
    pub matches: WindowMatch,
    #[serde(default)]
    pub effects: WindowEffects,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xwayland: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floating: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modal: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowEffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub float: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_initial_focus: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_blur: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_shadow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_anim: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_aspect_ratio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    #[serde(rename = "move", skip_serializing_if = "Option::is_none")]
    pub position: Option<[i32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_inhibit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerRule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(rename = "match", default)]
    pub matches: LayerMatch,
    #[serde(default)]
    pub effects: LayerEffects,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerMatch {
    #[serde(default)]
    pub namespace: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blur: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_anim: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_alpha: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRule {
    pub workspace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gaps_in: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gaps_out: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_size: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Rules {
    pub window: Vec<WindowRule>,
    pub layer: Vec<LayerRule>,
    pub workspace: Vec<WorkspaceRule>,
}

/// Rules load one by one: an entry that doesn't parse (a newer uQ's, or a hand edit) is dropped
/// with a log line and the others are kept; a list that isn't a list is empty.
impl<'de> Deserialize<'de> for Rules {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Rules, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(Rules {
            window: lenient(&v, "window"),
            layer: lenient(&v, "layer"),
            workspace: lenient(&v, "workspace"),
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
                Ok(rule) => Some(rule),
                Err(_) => {
                    tracing::warn!(
                        "state: dropping rules.{list}[{i}]: it doesn't fit this version"
                    );
                    None
                }
            })
            .collect(),
        Some(_) => {
            tracing::warn!("state: rules.{list} is not a list; ignoring it");
            Vec::new()
        }
    }
}

/// `^r-[0-9a-f]{4,8}$`
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix("r-").is_some_and(|h| {
        (4..=8).contains(&h.len()) && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// `r-` and 4 random hex digits, not in `taken`.
pub fn fresh_id(taken: &BTreeSet<String>) -> String {
    loop {
        let n = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish() as u16;
        let id = format!("r-{n:04x}");
        if !taken.contains(&id) {
            return id;
        }
    }
}

/// A pattern that matches exactly `s`.
pub fn exact_pattern(s: &str) -> String {
    format!("^{}$", regex::escape(s))
}

impl Rules {
    /// Every window and layer rule id, window rules first.
    pub fn ids(&self) -> Vec<&str> {
        let w = self.window.iter().map(|r| r.id.as_str());
        w.chain(self.layer.iter().map(|r| r.id.as_str())).collect()
    }

    fn id_slots(&mut self) -> impl Iterator<Item = (&'static str, usize, &mut String)> {
        let w = self
            .window
            .iter_mut()
            .enumerate()
            .map(|(i, r)| ("window", i, &mut r.id));
        let l = self
            .layer
            .iter_mut()
            .enumerate()
            .map(|(i, r)| ("layer", i, &mut r.id));
        w.chain(l)
    }

    /// A whole-section write: strict. `current` only steers [`fresh_id`]; ids that exist now are
    /// accepted again (every write resends them), and ids must be unique within `v`.
    pub fn from_section(v: &Value, current: &Rules) -> Result<Rules, SettingError> {
        const KEY: &str = "rules";
        let Some(obj) = v.as_object() else {
            return Err(invalid(KEY, "expected an object"));
        };
        check_known(KEY, v, &["window", "layer", "workspace"])?;
        let mut out = Rules {
            window: list(obj, "window", MAX_WINDOW_RULES, "at most 200 window rules")?,
            layer: list(obj, "layer", MAX_LAYER_RULES, "at most 50 panel rules")?,
            workspace: list(
                obj,
                "workspace",
                MAX_WORKSPACE_RULES,
                "at most 64 workspace rules",
            )?,
        };
        for (i, r) in out.window.iter_mut().enumerate() {
            check::window(i, r)?;
        }
        for (i, r) in out.layer.iter_mut().enumerate() {
            check::layer(i, r)?;
        }
        let mut selectors = BTreeSet::new();
        for (i, r) in out.workspace.iter_mut().enumerate() {
            check::workspace(i, r)?;
            if !selectors.insert(r.workspace.clone()) {
                return Err(invalid(
                    &format!("rules.workspace[{i}].workspace"),
                    "this workspace already has a rule",
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for (list, i, id) in out.id_slots() {
            if id.is_empty() {
                continue;
            }
            let key = format!("rules.{list}[{i}].id");
            if !valid_id(id) {
                return Err(invalid(&key, "an id looks like r-1a2b"));
            }
            if !seen.insert(id.clone()) {
                return Err(invalid(&key, "another rule has this id"));
            }
        }
        seen.extend(current.ids().into_iter().map(String::from));
        out.fill_ids(&mut seen);
        Ok(out)
    }

    fn fill_ids(&mut self, taken: &mut BTreeSet<String>) {
        for (_, _, id) in self.id_slots() {
            if id.is_empty() {
                *id = fresh_id(taken);
                taken.insert(id.clone());
            }
        }
    }

    /// Load-time repair: entries that break a rule, repeat an id or a workspace, or pass the
    /// limit are dropped with a log line, so the section stays writable. Values are normalised
    /// as a write would; a missing id is made.
    pub fn repaired(mut self) -> Rules {
        for r in &mut self.window {
            if r.effects.workspace.is_none() {
                r.effects.silent = None;
            }
        }
        self.window = keep(self.window, "window", check::window);
        self.layer = keep(self.layer, "layer", check::layer);
        self.workspace = keep(self.workspace, "workspace", check::workspace);
        // A bad or repeated id never costs the rule: it is cleared and made again below. The
        // first holder of an id keeps it.
        let mut seen = BTreeSet::new();
        for (list, i, id) in self.id_slots() {
            if !id.is_empty() && !(valid_id(id) && seen.insert(id.clone())) {
                tracing::warn!(
                    "state: rules.{list}[{i}] has a bad or repeated id; it gets a new one"
                );
                id.clear();
            }
        }
        let mut selectors = BTreeSet::new();
        self.workspace
            .retain(|r| selectors.insert(r.workspace.clone()));
        self.window.truncate(MAX_WINDOW_RULES);
        self.layer.truncate(MAX_LAYER_RULES);
        self.workspace.truncate(MAX_WORKSPACE_RULES);
        seen = self.ids().into_iter().map(String::from).collect();
        self.fill_ids(&mut seen);
        self
    }
}

fn keep<T>(
    items: Vec<T>,
    name: &str,
    check: impl Fn(usize, &mut T) -> Result<(), SettingError>,
) -> Vec<T> {
    items
        .into_iter()
        .enumerate()
        .filter_map(|(i, mut r)| match check(i, &mut r) {
            Ok(()) => Some(r),
            Err(e) => {
                tracing::warn!("state: dropping rules.{name}[{i}]: {e}");
                None
            }
        })
        .collect()
}

/// One list of a section: a list within `max`, whose entries have only known keys and fit the
/// entry type.
fn list<T: DeserializeOwned>(
    obj: &Map<String, Value>,
    name: &str,
    max: usize,
    too_many: &str,
) -> Result<Vec<T>, SettingError> {
    let key = format!("rules.{name}");
    let items = match obj.get(name) {
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
        .map(|(i, v)| {
            let k = format!("{key}[{i}]");
            check::known_keys(name, &k, v)?;
            serde_json::from_value(v.clone())
                .map_err(|_| invalid(&k, "a value is missing or of the wrong kind"))
        })
        .collect()
}

#[cfg(test)]
mod tests;
