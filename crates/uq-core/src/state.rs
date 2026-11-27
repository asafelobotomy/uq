//! uqd's persisted desktop state (`~/.local/state/uq/state.toml`).

use crate::atomic::atomic_write;
use crate::autostart::Autostart;
use crate::compat::Compat;
use crate::env::Env;
use crate::integration::rules::IntegrationRules;
use crate::keybinds::Keybinds;
use crate::lua::Layout;
use crate::rules::Rules;
use crate::schedule::Schedule;
use crate::settings::{LayoutOptions, SettingError, Settings};
use crate::theme::Variant;
use crate::welcome::Welcome;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_THEME: &str = "graphite";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub theme: String,
    pub variant: Variant,
    pub schedule: Schedule,
    #[serde(deserialize_with = "lenient_rules")]
    pub layouts: BTreeMap<String, Layout>,
    /// Window, panel and workspace rules; each entry loads on its own.
    #[serde(default)]
    pub rules: Rules,
    /// Keyboard shortcuts: changes to uQ's own, and your own; each entry loads on its own.
    #[serde(default)]
    pub keybinds: Keybinds,
    /// Environment variables for the session; each entry loads on its own.
    #[serde(default)]
    pub env: Env,
    /// Apps and scaling: the switches and the per-app fixes; each fix loads on its own.
    #[serde(default)]
    pub compat: Compat,
    /// Startup apps: choices about the XDG entries, and the user's own; each loads on its own.
    #[serde(default)]
    pub autostart: Autostart,
    /// Window rules from apps (packaged `rules.toml`) the person turned on, by id; each loads on
    /// its own. Empty: every packaged rule is opt-in.
    #[serde(default, skip_serializing_if = "is_empty_rules")]
    pub integration_rules: IntegrationRules,
    /// The scheduled transition uqd last applied; a manual change holds until a newer one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_mark: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether Welcome is still due (spec §5.1). Bookkeeping, not a setting: never journalled.
    #[serde(deserialize_with = "crate::welcome::lenient")]
    pub welcome: Welcome,
    #[serde(flatten)]
    pub settings: Settings,
    pub launch_counts: BTreeMap<String, u32>,
}

/// Workspace rules load one by one: a rule whose layout this version doesn't know (a newer
/// uQ's) is dropped with a log line, and the others are kept.
fn lenient_rules<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, Layout>, D::Error> {
    let serde_json::Value::Object(obj) = serde_json::Value::deserialize(d)? else {
        tracing::warn!("[STATE-002] state: [layouts] is not a table; ignoring it");
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(
            |(ws, v)| match serde_json::from_value::<Layout>(v.clone()) {
                Ok(l) => Some((ws, l)),
                Err(_) => {
                    tracing::warn!("[STATE-002] state: dropping workspace rule {ws:?} = {v}");
                    None
                }
            },
        )
        .collect())
}

fn is_empty_rules(r: &IntegrationRules) -> bool {
    r.enabled.is_empty()
}

/// TOML has no null, so a disabled idle stage (`None`) is stored as 0 in `state.toml`.
const IDLE_STAGES: [&str; 4] = ["dim", "lock", "screen_off", "suspend_battery"];

impl Default for State {
    fn default() -> Self {
        State {
            theme: DEFAULT_THEME.into(),
            variant: Variant::Dark,
            schedule: Schedule::Off,
            layouts: BTreeMap::new(),
            rules: Rules::default(),
            keybinds: Keybinds::default(),
            env: Env::default(),
            compat: Compat::default(),
            autostart: Autostart::default(),
            integration_rules: IntegrationRules::default(),
            schedule_mark: None,
            welcome: Welcome::default(),
            settings: Settings::default(),
            launch_counts: BTreeMap::new(),
        }
    }
}

impl State {
    /// Load state. A missing file yields the defaults. A file that won't parse is renamed to
    /// `state.toml.bad-<unix_secs>` first so pins and counts are never silently lost. A value
    /// that is wrong-typed, or parses but breaks a rule, costs only its own field or domain
    /// (see [`Settings::validated`]); the file is kept.
    pub fn load(path: &Path) -> State {
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return State::default(),
            Err(e) => {
                tracing::warn!("[STATE-001] {}: {e}; using defaults", path.display());
                return State::default();
            }
        };
        match Self::parse_strict(&src) {
            Ok(state) => state,
            Err(e) => {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let mut bad = path.as_os_str().to_owned();
                bad.push(format!(".bad-{secs}"));
                match std::fs::rename(path, &bad) {
                    Ok(()) => tracing::warn!(
                        "[STATE-001] {}: unreadable state ({e}); kept as {:?}, using defaults",
                        path.display(),
                        bad
                    ),
                    Err(re) => tracing::warn!(
                        "[STATE-001] {}: unreadable state ({e}), could not move it aside ({re}); using defaults",
                        path.display()
                    ),
                }
                State::default()
            }
        }
    }

    /// Parse `state.toml` text without touching any file: a document that isn't valid TOML or
    /// doesn't fit `State` (wrong type, unknown variant) is an error. Values that parse but break
    /// a rule are repaired as [`State::load`] does, costing only their own field or domain.
    pub fn parse_strict(src: &str) -> Result<State, toml::de::Error> {
        let mut state = Self::parse(src)?;
        let (valid, repaired) = state.settings.validated();
        for e in repaired {
            tracing::warn!("[STATE-002] state: {e}; that setting uses its default");
        }
        state.settings = valid;
        state.layouts = crate::settings::repair_rules(std::mem::take(&mut state.layouts));
        state.rules = std::mem::take(&mut state.rules).repaired();
        state.keybinds = std::mem::take(&mut state.keybinds).repaired();
        state.env = std::mem::take(&mut state.env).repaired();
        state.compat = std::mem::take(&mut state.compat).repaired();
        state.autostart = std::mem::take(&mut state.autostart).repaired();
        state.integration_rules = std::mem::take(&mut state.integration_rules).repaired();
        Ok(state)
    }

    fn parse(src: &str) -> Result<State, toml::de::Error> {
        let table: toml::Table = toml::from_str(src)?;
        let mut state: State = table.clone().try_into()?;
        if let Some(idle) = table.get("idle").and_then(toml::Value::as_table) {
            for stage in IDLE_STAGES {
                if idle.get(stage).and_then(toml::Value::as_integer) == Some(0) {
                    *Self::stage_mut(&mut state.settings, stage) = None;
                }
            }
        }
        Ok(state)
    }

    fn stage_mut<'a>(s: &'a mut Settings, stage: &str) -> &'a mut Option<u32> {
        match stage {
            "dim" => &mut s.idle.dim,
            "lock" => &mut s.idle.lock,
            "screen_off" => &mut s.idle.screen_off,
            _ => &mut s.idle.suspend_battery,
        }
    }

    /// One settings section as JSON. `layouts` joins the workspace rules in `self.layouts` with
    /// `settings.layout_options`; every other domain is `Settings`'.
    pub fn section(&self, domain: &str) -> Result<serde_json::Value, SettingError> {
        if domain == "rules" {
            return Ok(serde_json::to_value(&self.rules).expect("rules serialise"));
        }
        if domain == "keybinds" {
            return Ok(serde_json::to_value(&self.keybinds).expect("keybinds serialise"));
        }
        if domain == "env" {
            return Ok(self.env.section());
        }
        if domain == "compat" {
            return Ok(self.compat.section());
        }
        if domain == "autostart" {
            return Ok(self.autostart.section());
        }
        if domain == crate::integration::rules::DOMAIN {
            return Ok(self.integration_rules.section());
        }
        if domain != "layouts" {
            return self.settings.section(domain);
        }
        let mut v =
            serde_json::to_value(&self.settings.layout_options).expect("settings serialise");
        v["workspaces"] = serde_json::to_value(&self.layouts).expect("layouts serialise");
        Ok(v)
    }

    /// Replace a whole section; on error `self` is unchanged. A `layouts` write replaces both
    /// the options and every workspace rule (selectors normalised); a missing `workspaces`
    /// clears the rules.
    pub fn set_section(
        &mut self,
        domain: &str,
        value: serde_json::Value,
    ) -> Result<(), SettingError> {
        if domain == "rules" {
            self.rules = Rules::from_section(&value, &self.rules)?;
            return Ok(());
        }
        if domain == "keybinds" {
            self.keybinds = Keybinds::from_section(&value, &self.keybinds)?;
            return Ok(());
        }
        if domain == "env" {
            self.env = Env::from_section(&value)?;
            return Ok(());
        }
        if domain == "compat" {
            self.compat = Compat::from_section(&value)?;
            return Ok(());
        }
        if domain == "autostart" {
            self.autostart = Autostart::from_section(&value, &self.autostart)?;
            return Ok(());
        }
        if domain == crate::integration::rules::DOMAIN {
            self.integration_rules = IntegrationRules::from_section(&value)?;
            return Ok(());
        }
        if domain != "layouts" {
            return self.settings.set_section(domain, value);
        }
        let (options, rules) = LayoutOptions::from_section(&value)?;
        self.settings.layout_options = options;
        self.layouts = rules;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        atomic_write(path, &self.to_toml()?)
    }

    /// The file `save` writes, as text (a settings backup is this).
    pub fn to_toml(&self) -> std::io::Result<String> {
        let mut table = toml::Table::try_from(self).map_err(std::io::Error::other)?;
        let mut settings = self.settings.clone();
        if let Some(idle) = table.get_mut("idle").and_then(toml::Value::as_table_mut) {
            for stage in IDLE_STAGES {
                if Self::stage_mut(&mut settings, stage).is_none() {
                    idle.insert(stage.into(), toml::Value::Integer(0));
                }
            }
        }
        toml::to_string(&table).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod load_tests;

#[cfg(test)]
mod tests;
