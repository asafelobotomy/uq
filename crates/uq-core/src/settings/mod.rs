//! Validated user settings (spec §2.4): the model, its defaults, and per-key validation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) use load::{check_kept, lenient};

mod appearance;
mod input;
mod layouts;
mod load;
mod look;
mod monitors;
pub mod provenance;
mod shell;
mod wallpaper;
pub use appearance::{Appearance, valid_theme_name};
pub use input::{
    DeviceOverride, InputSettings, KbLayout, KeyboardSettings, TouchpadSettings, input_summary,
    valid_device_name, valid_layout, valid_switch_combo, valid_variant,
};
pub use layouts::{
    DwindleOptions, ForceSplit, LayoutOptions, MasterOptions, NewStatus, Orientation,
    ScrollingOptions, repair_rules, rule_selector, set_rule,
};
pub use look::Look;
pub use monitors::plan;
pub(crate) use monitors::summary::summary as monitors_summary;
pub use monitors::{
    BarChoice, BarShow, LiveOutput, ManualSlot, MonitorId, MonitorProfile, Monitors, OutputConfig,
    Position, SavedOutput, Strategy, WorkspacePlan, clean_name, clean_text, clean_text_spaced,
    counts_in_sets, parse_live, stays_visible, valid_mode, valid_output_name, valid_profile_id,
};
pub use shell::{
    BUILTIN_PROVIDERS, EdgeStyle, Islands, ProviderPref, ShellSettings, provider_enabled,
    provider_order, valid_terminal,
};
pub use wallpaper::{Fit, Wallpaper, WallpaperEntry};

/// Every key `Settings::apply` accepts; anything else is rejected.
pub const SETTING_KEYS: [&str; 18] = [
    "pinned",
    "weather_location",
    "idle.dim",
    "idle.lock",
    "idle.screen_off",
    "idle.suspend_battery",
    "idle.locked_screen_off",
    "idle.media_blocks_lock",
    "notif.backend",
    "notif.show_summary",
    "privacy.public_ip_lookup",
    "privacy.record_launches",
    "mouse.sensitivity",
    "mouse.accel_profile",
    "mouse.natural_scroll",
    "mouse.left_handed",
    "mouse.scroll_factor",
    "ui.reduce_motion",
];

/// Keys that change Hyprland's config: uqd renders, verifies and reloads for these instead of
/// taking the light commit path. `ui.reduce_motion` turns Hyprland's animations off.
pub fn renders_config(key: &str) -> bool {
    key.starts_with("mouse.") || key == "ui.reduce_motion"
}

/// Whole-section domains handled by [`Settings::section`] / [`Settings::set_section`].
pub const SECTION_DOMAINS: [&str; 13] = [
    "appearance",
    "wallpaper",
    "look",
    "layouts",
    "shell",
    "monitors",
    "input",
    "rules",
    "env",
    "compat",
    "autostart",
    "keybinds",
    "integration_rules",
];

/// Sections that change rendered config (theme env, Kvantum, cursor, Hyprland look and
/// layouts, input): uqd re-renders for these. `monitors` is not one: uqd applies it through
/// `ApplyMonitors`, with a revert window, and refuses `SetSection("monitors")`. Nor is
/// `autostart`: uqd's file sync and runner read it, and nothing is rendered.
pub fn renders_section(domain: &str) -> bool {
    matches!(
        domain,
        "appearance"
            | "look"
            | "layouts"
            | "input"
            | "rules"
            | "env"
            | "compat"
            | "keybinds"
            | "integration_rules"
    )
}

const MAX_PINNED: usize = 32;
const MAX_NAME_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SettingError {
    #[error("unknown setting key {0:?}")]
    UnknownKey(String),
    #[error("invalid value for {key}: {reason}")]
    Invalid { key: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdleTimings {
    pub dim: Option<u32>,
    pub lock: Option<u32>,
    pub screen_off: Option<u32>,
    pub suspend_battery: Option<u32>,
    pub locked_screen_off: u32,
    pub media_blocks_lock: bool,
}

impl Default for IdleTimings {
    fn default() -> Self {
        Self {
            dim: Some(240),
            lock: Some(300),
            screen_off: Some(360),
            suspend_battery: Some(1200),
            locked_screen_off: 30,
            media_blocks_lock: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifBackend {
    #[default]
    Auto,
    Notifi,
    Builtin,
    Swaync,
    Mako,
    Dunst,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifSettings {
    pub backend: NotifBackend,
    pub show_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    pub public_ip_lookup: bool,
    pub record_launches: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            public_ip_lookup: true,
            record_launches: true,
        }
    }
}

/// libinput's pointer acceleration profile; `Default` leaves the device's own choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccelProfile {
    #[default]
    Default,
    Adaptive,
    Flat,
}

/// Pointer settings for mice, rendered into the `input` block of `generated.lua`.
/// Person-wide interface preferences (`[ui]`), not tied to a theme.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Reduce motion: the shell skips its fades and crossfades and Hyprland's animations are
    /// off (generated.lua). Default false.
    pub reduce_motion: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseSettings {
    /// -1.0 (slowest) to 1.0 (fastest); 0 is unchanged.
    pub sensitivity: f64,
    pub accel_profile: AccelProfile,
    pub natural_scroll: bool,
    pub left_handed: bool,
    /// Wheel scroll multiplier, 0.1 to 10.
    pub scroll_factor: f64,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.0,
            accel_profile: AccelProfile::Default,
            natural_scroll: false,
            left_handed: false,
            scroll_factor: 1.0,
        }
    }
}

/// Every domain loads leniently (a wrong-typed or newer value costs only that domain, or that
/// field where the domain is lenient per field), so a bad value never fails the whole file.
/// [`Settings::validated`] then repairs values that parse but break a rule, per domain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(deserialize_with = "load::lenient_pins")]
    pub pinned: Vec<String>,
    #[serde(deserialize_with = "lenient")]
    pub weather_location: Option<Location>,
    #[serde(deserialize_with = "lenient")]
    pub idle: IdleTimings,
    #[serde(deserialize_with = "lenient")]
    pub notif: NotifSettings,
    #[serde(deserialize_with = "lenient")]
    pub privacy: PrivacySettings,
    #[serde(deserialize_with = "lenient")]
    pub mouse: MouseSettings,
    #[serde(deserialize_with = "lenient")]
    pub appearance: Appearance,
    #[serde(deserialize_with = "lenient")]
    pub wallpaper: Wallpaper,
    #[serde(deserialize_with = "lenient")]
    pub look: Look,
    /// Default layout and per-layout options (`[layout_options]`; `[layouts]` holds the
    /// workspace rules in `State`).
    #[serde(deserialize_with = "lenient")]
    pub layout_options: LayoutOptions,
    #[serde(deserialize_with = "lenient")]
    pub shell: ShellSettings,
    #[serde(deserialize_with = "lenient")]
    pub monitors: Monitors,
    /// Keyboard, touchpad and per-device options (`[input]`). The section's `mouse` part is
    /// `mouse` above, not stored here.
    #[serde(deserialize_with = "lenient")]
    pub input: InputSettings,
    #[serde(deserialize_with = "lenient")]
    pub ui: UiSettings,
}

pub(crate) fn invalid(key: &str, reason: impl Into<String>) -> SettingError {
    SettingError::Invalid {
        key: key.to_string(),
        reason: reason.into(),
    }
}

/// Reject keys of the JSON object `v` that are not in `known` (non-objects are left to serde).
pub(crate) fn check_known(key: &str, v: &Value, known: &[&str]) -> Result<(), SettingError> {
    if let Some(obj) = v.as_object()
        && let Some(k) = obj.keys().find(|k| !known.contains(&k.as_str()))
    {
        return Err(invalid(key, format!("unknown field {k:?}")));
    }
    Ok(())
}

fn as_bool(key: &str, v: &Value) -> Result<bool, SettingError> {
    v.as_bool()
        .ok_or_else(|| invalid(key, "expected a boolean"))
}

fn as_secs(key: &str, v: &Value, lo: u32, hi: u32) -> Result<u32, SettingError> {
    let n = v
        .as_u64()
        .ok_or_else(|| invalid(key, "expected a whole number of seconds"))?;
    if n < u64::from(lo) || n > u64::from(hi) {
        return Err(invalid(key, format!("must be between {lo} and {hi}")));
    }
    Ok(n as u32)
}

fn as_opt_secs(key: &str, v: &Value) -> Result<Option<u32>, SettingError> {
    if v.is_null() {
        Ok(None)
    } else {
        as_secs(key, v, 30, 86400).map(Some)
    }
}

/// `^[A-Za-z0-9._-]{1,128}(\.desktop)?$`
pub fn valid_pinned_id(s: &str) -> bool {
    fn base(s: &str) -> bool {
        (1..=128).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    }
    base(s) || s.strip_suffix(".desktop").is_some_and(base)
}

fn as_ranged_f64(key: &str, v: &Value, lo: f64, hi: f64) -> Result<f64, SettingError> {
    let n = v
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| invalid(key, "expected a number"))?;
    if !(lo..=hi).contains(&n) {
        return Err(invalid(key, format!("must be between {lo} and {hi}")));
    }
    Ok(round2(n))
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

impl Settings {
    /// One whole section as JSON. `layouts` also needs the workspace rules, so it goes through
    /// `State::section`.
    pub fn section(&self, domain: &str) -> Result<Value, SettingError> {
        let v = match domain {
            "appearance" => serde_json::to_value(&self.appearance),
            "wallpaper" => serde_json::to_value(&self.wallpaper),
            "look" => serde_json::to_value(&self.look),
            "shell" => serde_json::to_value(&self.shell),
            "monitors" => serde_json::to_value(&self.monitors),
            "input" => Ok(self.input_section()),
            _ => return Err(SettingError::UnknownKey(domain.to_string())),
        };
        Ok(v.expect("settings serialise"))
    }

    /// Replace a whole section. Unknown fields are rejected; on error `self` is unchanged.
    pub fn set_section(&mut self, domain: &str, value: Value) -> Result<(), SettingError> {
        match domain {
            "appearance" => {
                check_known(domain, &value, &Appearance::FIELDS)?;
                let a: Appearance =
                    serde_json::from_value(value).map_err(|e| invalid(domain, e.to_string()))?;
                a.validate()?;
                self.appearance = a;
            }
            "wallpaper" => {
                Wallpaper::check_keys(&value)?;
                let w: Wallpaper =
                    serde_json::from_value(value).map_err(|e| invalid(domain, e.to_string()))?;
                w.validate()?;
                self.wallpaper = w;
            }
            "look" => {
                check_known(domain, &value, &Look::FIELDS)?;
                let l: Look = serde_json::from_value(value.clone())
                    .map_err(|e| invalid(domain, e.to_string()))?;
                let parsed = serde_json::to_value(&l).expect("settings serialise");
                check_kept(domain, &value, Some(&parsed))?;
                self.look = l.normalised()?;
            }
            "shell" => self.shell = ShellSettings::from_section(&value)?,
            "monitors" => self.monitors = Monitors::from_section(&value)?,
            "input" => self.set_input(&value)?,
            _ => return Err(SettingError::UnknownKey(domain.to_string())),
        }
        Ok(())
    }

    /// Validate and apply one setting. On error `self` is left unchanged.
    pub fn apply(&mut self, key: &str, value: Value) -> Result<(), SettingError> {
        let mut next = self.clone();
        match key {
            "pinned" => {
                let arr = value
                    .as_array()
                    .ok_or_else(|| invalid(key, "expected an array"))?;
                if arr.len() > MAX_PINNED {
                    return Err(invalid(key, format!("at most {MAX_PINNED} entries")));
                }
                let mut out: Vec<String> = Vec::with_capacity(arr.len());
                for item in arr {
                    let id = item
                        .as_str()
                        .ok_or_else(|| invalid(key, "entries must be strings"))?;
                    if !valid_pinned_id(id) {
                        return Err(invalid(key, format!("invalid desktop id {id:?}")));
                    }
                    if !out.iter().any(|e| e == id) {
                        out.push(id.to_string());
                    }
                }
                next.pinned = out;
            }
            "weather_location" => {
                next.weather_location = if value.is_null() {
                    None
                } else {
                    let loc: Location =
                        serde_json::from_value(value).map_err(|e| invalid(key, e.to_string()))?;
                    if loc.name.chars().count() > MAX_NAME_CHARS {
                        return Err(invalid(
                            key,
                            format!("name over {MAX_NAME_CHARS} characters"),
                        ));
                    }
                    if !loc.lat.is_finite() || !(-90.0..=90.0).contains(&loc.lat) {
                        return Err(invalid(key, "lat must be within [-90, 90]"));
                    }
                    if !loc.lon.is_finite() || !(-180.0..=180.0).contains(&loc.lon) {
                        return Err(invalid(key, "lon must be within [-180, 180]"));
                    }
                    Some(Location {
                        name: loc.name,
                        lat: round2(loc.lat),
                        lon: round2(loc.lon),
                    })
                };
            }
            "idle.dim" => next.idle.dim = as_opt_secs(key, &value)?,
            "idle.lock" => next.idle.lock = as_opt_secs(key, &value)?,
            "idle.screen_off" => next.idle.screen_off = as_opt_secs(key, &value)?,
            "idle.suspend_battery" => next.idle.suspend_battery = as_opt_secs(key, &value)?,
            "idle.locked_screen_off" => {
                next.idle.locked_screen_off = as_secs(key, &value, 10, 600)?;
            }
            "idle.media_blocks_lock" => next.idle.media_blocks_lock = as_bool(key, &value)?,
            "notif.backend" => {
                next.notif.backend = serde_json::from_value(value).map_err(|_| {
                    invalid(key, "expected auto, notifi, builtin, swaync, mako or dunst")
                })?;
            }
            "notif.show_summary" => next.notif.show_summary = as_bool(key, &value)?,
            "privacy.public_ip_lookup" => next.privacy.public_ip_lookup = as_bool(key, &value)?,
            "privacy.record_launches" => next.privacy.record_launches = as_bool(key, &value)?,
            "mouse.sensitivity" => {
                next.mouse.sensitivity = as_ranged_f64(key, &value, -1.0, 1.0)?;
            }
            "mouse.accel_profile" => {
                next.mouse.accel_profile = serde_json::from_value(value)
                    .map_err(|_| invalid(key, "expected default, adaptive or flat"))?;
            }
            "mouse.natural_scroll" => next.mouse.natural_scroll = as_bool(key, &value)?,
            "mouse.left_handed" => next.mouse.left_handed = as_bool(key, &value)?,
            "mouse.scroll_factor" => {
                next.mouse.scroll_factor = as_ranged_f64(key, &value, 0.1, 10.0)?;
            }
            "ui.reduce_motion" => next.ui.reduce_motion = as_bool(key, &value)?,
            _ => return Err(SettingError::UnknownKey(key.to_string())),
        }
        if key.starts_with("idle.") {
            let i = &next.idle;
            let enabled: Vec<u32> = [i.dim, i.lock, i.screen_off, i.suspend_battery]
                .into_iter()
                .flatten()
                .collect();
            if !enabled.windows(2).all(|w| w[0] < w[1]) {
                return Err(invalid(
                    key,
                    "enabled stages must be strictly increasing (dim < lock < screen_off < suspend_battery)",
                ));
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
