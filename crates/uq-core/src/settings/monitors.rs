//! The `monitors` section: per output name, the mode, position, scale, transform, enabled
//! flag, mirror target and VRR. uqd applies it through `ApplyMonitors` (never `SetSection`),
//! so a bad layout can be reverted; the shapes here are shared by that path and the loader.
//!
//! Loading is lenient (a bad output entry costs only itself, never the theme or pins); the
//! API is strict ([`Monitors::from_section`]). Output names come from Hyprland, so they are
//! checked for charset and length before they can reach `generated.lua`.

use super::{SettingError, check_kept, check_known, invalid, lenient};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

mod extras;
mod identity;
mod live;
mod parse;
pub mod plan;
pub(crate) mod summary;
pub use extras::{
    BarChoice, BarShow, ManualSlot, MonitorProfile, SavedOutput, Strategy, WorkspacePlan,
    clean_name, valid_profile_id,
};
pub use identity::{MonitorId, counts_in_sets};
pub use live::{LiveOutput, parse_live, stays_visible};

const MAX_OUTPUTS: usize = 16;
pub(super) const MAX_TEXT_CHARS: usize = 200;

/// `^[A-Za-z0-9._-]{1,32}$`
pub fn valid_output_name(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Strip control characters and cut to `max` characters: text from Hyprland is untrusted.
pub fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// Like [`clean_text`], but each control character becomes a space, so words around it stay
/// apart (a title with an escape sequence shows as plain text).
pub fn clean_text_spaced(s: &str, max: usize) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max)
        .collect()
}

fn digits(s: &str, len: std::ops::RangeInclusive<usize>) -> bool {
    len.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// `preferred`, `highres`, `highrr`, or `WxH` / `WxH@R` (W, H 2-5 digits; R `\d{1,3}(\.\d{1,3})?`).
pub fn valid_mode(s: &str) -> bool {
    if matches!(s, "preferred" | "highres" | "highrr") {
        return true;
    }
    let (res, rate) = match s.split_once('@') {
        Some((r, rate)) => (r, Some(rate)),
        None => (s, None),
    };
    let Some((w, h)) = res.split_once('x') else {
        return false;
    };
    let rate_ok = rate.is_none_or(|r| match r.split_once('.') {
        Some((i, f)) => digits(i, 1..=3) && digits(f, 1..=3),
        None => digits(r, 1..=3),
    });
    digits(w, 2..=5) && digits(h, 2..=5) && rate_ok
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// One output's settings. `None` means "Hyprland's default" (`auto` for position and scale).
/// Unknown keys are ignored on load and rejected by [`Monitors::from_section`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub position: Option<Position>,
    /// 0.25 to 4.0, rounded to 6 places.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scale: Option<f64>,
    /// 0 to 7, as Hyprland's transform.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub transform: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub mirror_of: Option<String>,
    /// 0 off, 1 on, 2 fullscreen only, 3 fullscreen video/game.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub vrr: Option<u8>,
    /// "make model serial" of the output when it was saved; metadata, never rendered.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub description: Option<String>,
}

impl OutputConfig {
    pub const FIELDS: [&str; 8] = [
        "mode",
        "position",
        "scale",
        "transform",
        "enabled",
        "mirror_of",
        "vrr",
        "description",
    ];

    /// Parse one entry, refusing a field that is present but the wrong type or out of the
    /// type's range (the lenient field parse would otherwise silently drop it).
    fn parse(key: &str, v: &Value) -> Result<Self, SettingError> {
        let cfg: Self =
            serde_json::from_value(v.clone()).map_err(|e| invalid(key, e.to_string()))?;
        let back = serde_json::to_value(&cfg).expect("settings serialise");
        check_kept(key, v, Some(&back))?;
        Ok(cfg)
    }

    fn validate(&self, key: &str) -> Result<(), SettingError> {
        if let Some(m) = &self.mode
            && !valid_mode(m)
        {
            return Err(invalid(
                key,
                format!("mode {m:?} must be preferred, highres, highrr, WxH or WxH@R"),
            ));
        }
        if let Some(p) = self.position
            && ![p.x, p.y].iter().all(|n| (-32768..=32767).contains(n))
        {
            return Err(invalid(key, "position must be within -32768 and 32767"));
        }
        if let Some(s) = self.scale
            && (!s.is_finite() || !(0.25..=4.0).contains(&s))
        {
            return Err(invalid(key, "scale must be between 0.25 and 4"));
        }
        if self.transform.is_some_and(|t| t > 7) {
            return Err(invalid(key, "transform must be 0 to 7"));
        }
        if self.vrr.is_some_and(|v| v > 3) {
            return Err(invalid(key, "vrr must be 0 to 3"));
        }
        if let Some(t) = &self.mirror_of
            && !valid_output_name(t)
        {
            return Err(invalid(key, format!("invalid output name {t:?}")));
        }
        Ok(())
    }

    /// Round the scale, and clean the description (an empty one is none).
    fn normalised(mut self) -> Self {
        self.scale = self.scale.map(|s| (s * 1e6).round() / 1e6);
        self.description = self
            .description
            .map(|d| clean_text(&d, MAX_TEXT_CHARS))
            .filter(|d| !d.is_empty());
        self
    }
}

/// An entry of the loader: unknown keys dropped, wrong types refuse the whole entry.
fn lenient_entry(key: &str, v: &Value) -> Option<OutputConfig> {
    let obj = v.as_object()?;
    let known: Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| OutputConfig::FIELDS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    OutputConfig::parse(key, &Value::Object(known)).ok()
}

/// Entries load one by one: a bad one is dropped with a log line, the others are kept.
fn lenient_outputs<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, OutputConfig>, D::Error> {
    let Value::Object(obj) = Value::deserialize(d)? else {
        tracing::warn!("state: [monitors.outputs] is not a table; ignoring it");
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(|(name, v)| match lenient_entry(&name, &v) {
            Some(c) => Some((name, c)),
            None => {
                tracing::warn!("state: dropping monitor entry {name:?} = {v}");
                None
            }
        })
        .collect())
}

/// The layout (`outputs`) and, beside it, the main display, the bar choice, the workspace
/// planner and the saved arrangements. A default key is not written to `state.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Monitors {
    #[serde(deserialize_with = "lenient_outputs")]
    pub outputs: BTreeMap<String, OutputConfig>,
    /// The main display; `None` means "none chosen".
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub primary: Option<MonitorId>,
    /// Switch to a saved arrangement when the connected screens change.
    #[serde(
        skip_serializing_if = "extras::is_true",
        deserialize_with = "extras::lenient_true"
    )]
    pub auto_profiles: bool,
    #[serde(
        skip_serializing_if = "extras::is_default",
        deserialize_with = "lenient"
    )]
    pub bar: BarChoice,
    #[serde(
        skip_serializing_if = "extras::is_default",
        deserialize_with = "lenient"
    )]
    pub workspaces: WorkspacePlan,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "extras::lenient_list"
    )]
    pub profiles: Vec<MonitorProfile>,
}

impl Default for Monitors {
    fn default() -> Self {
        Self {
            outputs: BTreeMap::new(),
            primary: None,
            auto_profiles: true,
            bar: BarChoice::default(),
            workspaces: WorkspacePlan::default(),
            profiles: Vec::new(),
        }
    }
}

impl Monitors {
    pub const FIELDS: [&str; 6] = [
        "outputs",
        "primary",
        "auto_profiles",
        "bar",
        "workspaces",
        "profiles",
    ];

    /// Whether the layout (`outputs`) is the same; the other keys are not compared.
    pub fn layout_eq(&self, other: &Monitors) -> bool {
        self.outputs == other.outputs
    }

    /// These settings with `outputs` replaced.
    pub fn with_layout(&self, outputs: BTreeMap<String, OutputConfig>) -> Monitors {
        Monitors {
            outputs,
            ..self.clone()
        }
    }

    /// Every rule: at most 16 outputs, valid names and values, and mirrors that point at a
    /// different output which doesn't mirror another (an output with no entry is fine).
    pub fn validate(&self) -> Result<(), SettingError> {
        if self.outputs.len() > MAX_OUTPUTS {
            return Err(invalid(
                "monitors.outputs",
                format!("at most {MAX_OUTPUTS} outputs"),
            ));
        }
        for (name, cfg) in &self.outputs {
            let key = format!("monitors.outputs.{name}");
            if !valid_output_name(name) {
                return Err(invalid(
                    "monitors.outputs",
                    format!("invalid output name {name:?}"),
                ));
            }
            cfg.validate(&key)?;
            if let Some(t) = &cfg.mirror_of {
                if t == name {
                    return Err(invalid(&key, "an output can't mirror itself"));
                }
                if self.outputs.get(t).is_some_and(|c| c.mirror_of.is_some()) {
                    return Err(invalid(
                        &key,
                        format!("{t:?} already mirrors another output"),
                    ));
                }
            }
        }
        if let Some(p) = &self.primary {
            p.check("monitors.primary")?;
        }
        self.bar.validate("monitors.bar")?;
        self.workspaces.validate("monitors.workspaces")?;
        parse::validate_profiles(&self.profiles)
    }

    /// Parse a whole `monitors` section strictly: unknown keys, wrong types and rule breaks
    /// are errors. Scales are rounded and descriptions cleaned.
    pub fn from_section(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "monitors";
        let obj = v
            .as_object()
            .ok_or_else(|| invalid(KEY, "expected an object"))?;
        check_known(KEY, v, &Self::FIELDS)?;
        let mut out = Self::default();
        match obj.get("outputs") {
            None | Some(Value::Null) => {}
            Some(Value::Object(entries)) => {
                for (name, e) in entries {
                    let key = format!("monitors.outputs.{name}");
                    if !e.is_object() {
                        return Err(invalid(&key, "expected an object"));
                    }
                    check_known(&key, e, &OutputConfig::FIELDS)?;
                    out.outputs
                        .insert(name.clone(), OutputConfig::parse(&key, e)?.normalised());
                }
            }
            Some(_) => return Err(invalid("monitors.outputs", "expected an object")),
        }
        match obj.get("primary") {
            None | Some(Value::Null) => {}
            Some(p) => out.primary = Some(parse::parse_id("monitors.primary", p)?),
        }
        match obj.get("auto_profiles") {
            None | Some(Value::Null) => {}
            Some(Value::Bool(b)) => out.auto_profiles = *b,
            Some(_) => return Err(invalid("monitors.auto_profiles", "expected true or false")),
        }
        if let Some(b) = obj.get("bar").filter(|b| !b.is_null()) {
            out.bar = parse::parse_bar(b)?;
        }
        if let Some(w) = obj.get("workspaces").filter(|w| !w.is_null()) {
            out.workspaces = parse::parse_workspaces("monitors.workspaces", w)?;
        }
        if let Some(p) = obj.get("profiles").filter(|p| !p.is_null()) {
            out.profiles = parse::parse_profiles(p)?;
        }
        out.validate()?;
        Ok(out)
    }

    /// Load-time repair: drop entries that break a rule (and surplus ones past 16) one by
    /// one, and clear a `mirror_of` that points at itself or at an output that mirrors.
    pub fn sanitised(self) -> (Self, Vec<SettingError>) {
        let mut errors = Vec::new();
        let mut outputs = BTreeMap::new();
        for (name, cfg) in self.outputs {
            let key = format!("monitors.outputs.{name}");
            let ok = if valid_output_name(&name) {
                cfg.validate(&key)
            } else {
                Err(invalid(
                    "monitors.outputs",
                    format!("invalid output name {name:?}"),
                ))
            };
            match ok {
                Err(e) => errors.push(e),
                Ok(()) if outputs.len() >= MAX_OUTPUTS => {
                    errors.push(invalid(&key, "dropped: more than 16 outputs"));
                }
                Ok(()) => {
                    outputs.insert(name, cfg.normalised());
                }
            }
        }
        let mirrors: BTreeMap<String, bool> = outputs
            .iter()
            .map(|(n, c)| (n.clone(), c.mirror_of.is_some()))
            .collect();
        for (name, cfg) in &mut outputs {
            if let Some(t) = &cfg.mirror_of
                && (t == name || mirrors.get(t).copied().unwrap_or(false))
            {
                errors.push(invalid(
                    &format!("monitors.outputs.{name}"),
                    "mirror_of cleared: it points at itself or at an output that mirrors",
                ));
                cfg.mirror_of = None;
            }
        }
        let primary = extras::repaired_primary(self.primary, &mut errors);
        let bar = extras::repaired_bar(self.bar, &mut errors);
        let workspaces = extras::repaired_workspaces(self.workspaces, &mut errors);
        let profiles = extras::repaired_profiles(self.profiles, &mut errors);
        (
            Self {
                outputs,
                primary,
                auto_profiles: self.auto_profiles,
                bar,
                workspaces,
                profiles,
            },
            errors,
        )
    }
}

#[cfg(test)]
mod extras_tests;
#[cfg(test)]
mod tests;
