//! The `monitors` section's keys beside the layout: the main display, the bar choice, the
//! workspace planner and saved display arrangements. Loading is lenient per key and per
//! entry, writes are strict; none of it ever costs `outputs`.

use super::{
    MonitorId, OutputConfig, SettingError, clean_text_spaced, invalid, lenient, valid_output_name,
};
use crate::rules::fresh_prefixed;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MAX_PROFILES: usize = 32;
pub const MAX_PROFILE_SCREENS: usize = 16;
pub const MAX_BAR_SCREENS: usize = 16;
pub const MAX_NAME_CHARS: usize = 60;
pub const MAX_WORKSPACES: u8 = 20;

/// Whether `v` is the type's default; a default key is left out of `state.toml`.
pub(super) fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

pub(super) fn is_true(b: &bool) -> bool {
    *b
}

/// A bool that reads as `true` when wrong or missing.
pub(super) fn lenient_true<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(d)?.as_bool().unwrap_or(true))
}

/// A list that loads entry by entry: a bad entry is dropped with a log line.
pub(super) fn lenient_list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| match serde_json::from_value::<T>(v.clone()) {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!("state: dropping monitors entry {v}: {e}");
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarShow {
    #[default]
    All,
    Main,
    Chosen,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BarChoice {
    pub show: BarShow,
    #[serde(deserialize_with = "lenient_list")]
    pub on: Vec<MonitorId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    #[default]
    Off,
    Sequential,
    Interleave,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualSlot {
    pub workspace: u8,
    pub monitor: MonitorId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspacePlan {
    pub strategy: Strategy,
    pub count: u8,
    #[serde(deserialize_with = "lenient_list")]
    pub manual: Vec<ManualSlot>,
    pub move_back: bool,
}

impl Default for WorkspacePlan {
    fn default() -> Self {
        Self {
            strategy: Strategy::Off,
            count: 10,
            manual: Vec::new(),
            move_back: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedOutput {
    pub monitor: MonitorId,
    /// The connector the screen was on when saved; a hint for twins, never rendered.
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub config: OutputConfig,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorProfile {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "lenient_list")]
    pub outputs: Vec<SavedOutput>,
    #[serde(deserialize_with = "lenient")]
    pub workspaces: Option<WorkspacePlan>,
}

/// `^m-[0-9a-f]{4,8}$`
pub fn valid_profile_id(id: &str) -> bool {
    id.strip_prefix("m-").is_some_and(|h| {
        (4..=8).contains(&h.len()) && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

impl MonitorProfile {
    pub const FIELDS: [&str; 4] = ["id", "name", "outputs", "workspaces"];

    /// The screens it is for, as a sorted multiset.
    pub fn set(&self) -> Vec<MonitorId> {
        let mut set: Vec<MonitorId> = self.outputs.iter().map(|o| o.monitor.clone()).collect();
        set.sort();
        set
    }

    /// Every rule for one arrangement.
    pub(super) fn validate(&self, key: &str) -> Result<(), SettingError> {
        if !valid_profile_id(&self.id) {
            return Err(invalid(
                &format!("{key}.id"),
                "an arrangement's id is m- and 4 to 8 hex digits",
            ));
        }
        let name = &self.name;
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
            return Err(invalid(
                &format!("{key}.name"),
                format!("a name is 1 to {MAX_NAME_CHARS} characters"),
            ));
        }
        if name.chars().any(char::is_control) {
            return Err(invalid(
                &format!("{key}.name"),
                "a name can't have control characters",
            ));
        }
        let okey = format!("{key}.outputs");
        if self.outputs.is_empty() || self.outputs.len() > MAX_PROFILE_SCREENS {
            return Err(invalid(
                &okey,
                format!("an arrangement has 1 to {MAX_PROFILE_SCREENS} screens"),
            ));
        }
        for (i, o) in self.outputs.iter().enumerate() {
            let k = format!("{okey}[{i}]");
            o.monitor.check(&format!("{k}.monitor"))?;
            if !o.hint.is_empty() && !valid_output_name(&o.hint) {
                return Err(invalid(&format!("{k}.hint"), "not a valid output name"));
            }
            o.config.validate(&k)?;
        }
        if let Some(w) = &self.workspaces {
            w.validate(&format!("{key}.workspaces"))?;
        }
        Ok(())
    }

    pub(super) fn normalised(mut self) -> Self {
        self.name = self.name.trim().to_string();
        for o in &mut self.outputs {
            o.config = std::mem::take(&mut o.config).normalised();
        }
        self
    }
}

impl BarChoice {
    pub(super) fn validate(&self, key: &str) -> Result<(), SettingError> {
        if self.on.len() > MAX_BAR_SCREENS {
            return Err(invalid(
                key,
                format!("at most {MAX_BAR_SCREENS} screens can be chosen"),
            ));
        }
        self.on
            .iter()
            .try_for_each(|m| m.check(&format!("{key}.on")))
    }
}

impl WorkspacePlan {
    pub(super) fn validate(&self, key: &str) -> Result<(), SettingError> {
        if !(1..=MAX_WORKSPACES).contains(&self.count) {
            return Err(invalid(
                key,
                format!("the count is 1 to {MAX_WORKSPACES} workspaces"),
            ));
        }
        let mut seen = BTreeSet::new();
        for s in &self.manual {
            if !(1..=self.count).contains(&s.workspace) {
                return Err(invalid(
                    key,
                    format!("workspace {} is outside 1 to {}", s.workspace, self.count),
                ));
            }
            if !seen.insert(s.workspace) {
                return Err(invalid(
                    key,
                    format!("workspace {} is listed twice", s.workspace),
                ));
            }
            s.monitor.check(key)?;
        }
        Ok(())
    }

    /// Drop the slots a write would refuse (loading is lenient).
    fn repaired(mut self, key: &str, errors: &mut Vec<SettingError>) -> Self {
        if !(1..=MAX_WORKSPACES).contains(&self.count) {
            errors.push(invalid(key, "reset: the count is not 1 to 20"));
            return Self::default();
        }
        let mut seen = BTreeSet::new();
        let count = self.count;
        self.manual.retain(|s| {
            let ok = (1..=count).contains(&s.workspace)
                && s.monitor.check(key).is_ok()
                && seen.insert(s.workspace);
            if !ok {
                errors.push(invalid(
                    key,
                    format!("dropped the slot for workspace {}", s.workspace),
                ));
            }
            ok
        });
        self
    }
}

// ---- load-time repair ----

/// Repair the profiles: bad entries and a second one for the same screens are dropped, a
/// repeated id gets a new one, and everything past 32 is cut.
pub(super) fn repaired_profiles(
    profiles: Vec<MonitorProfile>,
    errors: &mut Vec<SettingError>,
) -> Vec<MonitorProfile> {
    let mut out: Vec<MonitorProfile> = Vec::new();
    let mut sets = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (i, p) in profiles.into_iter().enumerate() {
        let key = format!("monitors.profiles[{i}]");
        let mut p = p.normalised();
        if let Some(w) = p.workspaces.take() {
            p.workspaces = Some(w.repaired(&format!("{key}.workspaces"), errors));
        }
        if let Err(e) = p.validate(&key) {
            errors.push(e);
            continue;
        }
        if out.len() >= MAX_PROFILES {
            errors.push(invalid(&key, "dropped: more than 32 arrangements"));
            continue;
        }
        if !sets.insert(p.set()) {
            errors.push(invalid(
                &key,
                "dropped: another arrangement has these screens",
            ));
            continue;
        }
        if ids.contains(&p.id) {
            errors.push(invalid(
                &format!("{key}.id"),
                "repeated id: it gets a new one",
            ));
            let taken: BTreeSet<String> = ids
                .iter()
                .cloned()
                .chain(std::iter::once(p.id.clone()))
                .collect();
            p.id = fresh_prefixed("m-", &taken);
        }
        ids.insert(p.id.clone());
        out.push(p);
    }
    out
}

pub(super) fn repaired_bar(mut bar: BarChoice, errors: &mut Vec<SettingError>) -> BarChoice {
    let before = bar.on.len();
    bar.on.retain(|m| m.check("monitors.bar.on").is_ok());
    bar.on.truncate(MAX_BAR_SCREENS);
    if bar.on.len() != before {
        errors.push(invalid("monitors.bar.on", "dropped bad or surplus screens"));
    }
    bar
}

pub(super) fn repaired_workspaces(
    w: WorkspacePlan,
    errors: &mut Vec<SettingError>,
) -> WorkspacePlan {
    w.repaired("monitors.workspaces", errors)
}

pub(super) fn repaired_primary(
    p: Option<MonitorId>,
    errors: &mut Vec<SettingError>,
) -> Option<MonitorId> {
    p.filter(|m| match m.check("monitors.primary") {
        Ok(()) => true,
        Err(e) => {
            errors.push(e);
            false
        }
    })
}

/// A name, trimmed and cleaned as a person typed it (control characters become spaces).
pub fn clean_name(name: &str) -> String {
    clean_text_spaced(name, MAX_NAME_CHARS).trim().to_string()
}
