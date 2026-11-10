//! The `input` section: keyboard layouts, repeat, touchpad options and per-device overrides.
//! Hyprland renders these into the `input` block and `hl.device`.
//!
//! The section's JSON is `{keyboard, mouse, touchpad, devices}`, but `mouse` is not stored
//! here: it is [`Settings::mouse`], kept in the Phase 2 `[mouse]` table so `mouse.*` keys keep
//! working unchanged. [`InputSettings`] holds the other three parts (TOML `[input]`).
//!
//! Loading is lenient per field and per entry (a bad value costs only itself, an unknown key
//! inside a nested table is ignored, never fatal to its entry); the API is strict.

use super::{
    AccelProfile, SettingError, Settings, check_kept, check_known, invalid, lenient, round2,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

mod keyboard;
pub use keyboard::{KbLayout, KeyboardSettings, valid_layout, valid_switch_combo, valid_variant};
use std::collections::BTreeMap;

const MAX_DEVICES: usize = 32;
const MAX_NAME_CHARS: usize = 128;
const MOUSE_FIELDS: [&str; 5] = [
    "sensitivity",
    "accel_profile",
    "natural_scroll",
    "left_handed",
    "scroll_factor",
];

/// 1 to 128 characters, no control characters.
pub fn valid_device_name(s: &str) -> bool {
    let n = s.chars().count();
    (1..=MAX_NAME_CHARS).contains(&n) && !s.chars().any(char::is_control)
}

/// Parse one object strictly: unknown keys and values the lenient fields would drop are errors.
pub(super) fn parse_strict<T: DeserializeOwned + Serialize>(
    key: &str,
    v: &Value,
    fields: &[&str],
) -> Result<T, SettingError> {
    if !v.is_object() {
        return Err(invalid(key, "expected an object"));
    }
    check_known(key, v, fields)?;
    let t: T = serde_json::from_value(v.clone()).map_err(|e| invalid(key, e.to_string()))?;
    let back = serde_json::to_value(&t).expect("settings serialise");
    check_kept(key, v, Some(&back))?;
    Ok(t)
}

/// The loader's version of [`parse_strict`]: unknown keys are dropped first (a newer uQ's).
pub(super) fn parse_lenient<T: DeserializeOwned + Serialize>(
    key: &str,
    v: &Value,
    fields: &[&str],
) -> Option<T> {
    let known: Map<String, Value> = v
        .as_object()?
        .iter()
        .filter(|(k, _)| fields.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    parse_strict(key, &Value::Object(known), fields).ok()
}

fn scroll_ok(f: Option<f64>) -> bool {
    f.is_none_or(|f| f.is_finite() && (0.1..=10.0).contains(&f))
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TouchpadSettings {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub natural_scroll: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub tap_to_click: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub disable_while_typing: Option<bool>,
    /// 0.1 to 10, rounded to 2 places.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scroll_factor: Option<f64>,
}

impl TouchpadSettings {
    pub const FIELDS: [&str; 4] = [
        "natural_scroll",
        "tap_to_click",
        "disable_while_typing",
        "scroll_factor",
    ];

    pub fn validate(&self) -> Result<(), SettingError> {
        if !scroll_ok(self.scroll_factor) {
            return Err(invalid(
                "input.touchpad",
                "scroll_factor must be between 0.1 and 10",
            ));
        }
        Ok(())
    }

    fn normalised(mut self) -> Self {
        self.scroll_factor = self.scroll_factor.map(round2);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceOverride {
    /// -1 to 1, rounded to 2 places.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub sensitivity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub accel_profile: Option<AccelProfile>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub natural_scroll: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub left_handed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scroll_factor: Option<f64>,
}

impl DeviceOverride {
    pub const FIELDS: [&str; 5] = [
        "sensitivity",
        "accel_profile",
        "natural_scroll",
        "left_handed",
        "scroll_factor",
    ];

    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn sens_ok(&self) -> bool {
        self.sensitivity
            .is_none_or(|s| s.is_finite() && (-1.0..=1.0).contains(&s))
    }

    fn validate(&self, key: &str) -> Result<(), SettingError> {
        if !self.sens_ok() {
            return Err(invalid(key, "sensitivity must be between -1 and 1"));
        }
        if !scroll_ok(self.scroll_factor) {
            return Err(invalid(key, "scroll_factor must be between 0.1 and 10"));
        }
        Ok(())
    }

    fn normalised(mut self) -> Self {
        self.sensitivity = self.sensitivity.map(round2);
        self.scroll_factor = self.scroll_factor.map(round2);
        self
    }
}

/// Entries load one by one: a bad one is dropped, the others are kept.
fn lenient_devices<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, DeviceOverride>, D::Error> {
    let Value::Object(obj) = Value::deserialize(d)? else {
        return Ok(BTreeMap::new());
    };
    Ok(obj
        .into_iter()
        .filter_map(|(name, v)| {
            let key = format!("input.devices.{name}");
            let e = parse_lenient(&key, &v, &DeviceOverride::FIELDS);
            if e.is_none() {
                tracing::warn!("state: dropping input device entry {name:?} = {v}");
            }
            e.map(|e| (name, e))
        })
        .collect())
}

/// Hyprland turns spaces in a device name into `-`; so do we, on write.
fn device_key(name: &str) -> String {
    name.replace(' ', "-")
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InputSettings {
    #[serde(deserialize_with = "lenient")]
    pub keyboard: KeyboardSettings,
    #[serde(deserialize_with = "lenient")]
    pub touchpad: TouchpadSettings,
    #[serde(deserialize_with = "lenient_devices")]
    pub devices: BTreeMap<String, DeviceOverride>,
}

impl InputSettings {
    /// The parts of the section's JSON (`mouse` is the fourth, read from `Settings::mouse`).
    pub const FIELDS: [&str; 4] = ["keyboard", "mouse", "touchpad", "devices"];

    /// Parse the section's JSON strictly, ignoring `mouse` (see [`Settings::set_section`]).
    pub fn from_section(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "input";
        let obj = v
            .as_object()
            .ok_or_else(|| invalid(KEY, "expected an object"))?;
        check_known(KEY, v, &Self::FIELDS)?;
        let part = |name: &str| obj.get(name).filter(|v| !v.is_null());
        let keyboard = part("keyboard")
            .map(KeyboardSettings::from_value)
            .transpose()?
            .unwrap_or_default();
        let touchpad = match part("touchpad") {
            Some(t) => {
                let t: TouchpadSettings =
                    parse_strict("input.touchpad", t, &TouchpadSettings::FIELDS)?;
                t.validate()?;
                t.normalised()
            }
            None => TouchpadSettings::default(),
        };
        let mut devices = BTreeMap::new();
        match part("devices") {
            None => {}
            Some(Value::Object(entries)) => {
                for (name, e) in entries {
                    if !valid_device_name(name) {
                        return Err(invalid(
                            "input.devices",
                            "a device name is 1 to 128 characters with no control characters",
                        ));
                    }
                    let key = format!("input.devices.{name}");
                    let cfg: DeviceOverride = parse_strict(&key, e, &DeviceOverride::FIELDS)?;
                    cfg.validate(&key)?;
                    if cfg.is_empty() {
                        continue;
                    }
                    if devices.insert(device_key(name), cfg.normalised()).is_some() {
                        return Err(invalid(&key, "two devices with the same name"));
                    }
                }
            }
            Some(_) => return Err(invalid("input.devices", "expected an object")),
        }
        if devices.len() > MAX_DEVICES {
            return Err(invalid(
                "input.devices",
                format!("at most {MAX_DEVICES} devices"),
            ));
        }
        Ok(Self {
            keyboard,
            touchpad,
            devices,
        })
    }

    /// Load-time repair: clear out-of-range fields one by one, drop bad, empty, repeated and
    /// surplus entries, round and rename devices. Returns what was repaired.
    pub fn sanitised(self) -> (Self, Vec<SettingError>) {
        let mut errors = Vec::new();
        let keyboard = self.keyboard.sanitised(&mut errors);
        let mut touchpad = self.touchpad;
        if !scroll_ok(touchpad.scroll_factor) {
            errors.push(invalid(
                "input.touchpad",
                "scroll_factor cleared: out of range",
            ));
            touchpad.scroll_factor = None;
        }
        let mut devices: BTreeMap<String, DeviceOverride> = BTreeMap::new();
        for (name, mut cfg) in self.devices {
            let key = format!("input.devices.{name}");
            if !valid_device_name(&name) {
                errors.push(invalid(
                    "input.devices",
                    "dropped a device with an invalid name",
                ));
                continue;
            }
            if !cfg.sens_ok() {
                errors.push(invalid(&key, "sensitivity cleared: out of range"));
                cfg.sensitivity = None;
            }
            if !scroll_ok(cfg.scroll_factor) {
                errors.push(invalid(&key, "scroll_factor cleared: out of range"));
                cfg.scroll_factor = None;
            }
            if cfg.is_empty() {
                continue;
            }
            let k = device_key(&name);
            if devices.contains_key(&k) {
                errors.push(invalid(&key, "dropped: same name as another device"));
            } else if devices.len() >= MAX_DEVICES {
                errors.push(invalid(&key, "dropped: more than 32 devices"));
            } else {
                devices.insert(k, cfg.normalised());
            }
        }
        (
            Self {
                keyboard,
                touchpad: touchpad.normalised(),
                devices,
            },
            errors,
        )
    }
}

impl Settings {
    /// The whole `input` section: the stored parts plus `mouse` from `[mouse]`.
    pub(super) fn input_section(&self) -> Value {
        let mut v = serde_json::to_value(&self.input).expect("settings serialise");
        v["mouse"] = serde_json::to_value(&self.mouse).expect("settings serialise");
        v
    }

    /// Replace the `input` section strictly. `mouse` replays each key through
    /// `apply("mouse.<k>")`; a missing part means its default. On error `self` is unchanged.
    pub(super) fn set_input(&mut self, value: &Value) -> Result<(), SettingError> {
        let input = InputSettings::from_section(value)?;
        let mut next = self.clone();
        next.mouse = Default::default();
        match value.get("mouse") {
            None | Some(Value::Null) => {}
            Some(m @ Value::Object(entries)) => {
                check_known("input.mouse", m, &MOUSE_FIELDS)?;
                for (k, v) in entries {
                    next.apply(&format!("mouse.{k}"), v.clone())?;
                }
            }
            Some(_) => return Err(invalid("input.mouse", "expected an object")),
        }
        next.input = input;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
