//! The `input` section: keyboard layouts, repeat, touchpad options and per-device overrides.
//! Hyprland renders these into the `input` block and `hl.device`.
//!
//! The section's JSON is `{keyboard, mouse, touchpad, devices, gestures, swipe}`, but `mouse` is not stored
//! here: it is [`Settings::mouse`], kept in the Phase 2 `[mouse]` table so `mouse.*` keys keep
//! working unchanged. [`InputSettings`] holds the other parts (TOML `[input]`).
//!
//! Loading is lenient per field and per entry (a bad value costs only itself, an unknown key
//! inside a nested table is ignored, never fatal to its entry); the API is strict.

use super::{
    AccelProfile, SettingError, Settings, check_kept, check_known, invalid, lenient, round2,
};
use crate::gestures::{self, Gesture, MAX_GESTURES, SwipeOptions};
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

/// Gestures load entry by entry: a bad entry is dropped with a log line. Absent, `null` or
/// not a list is `None`, which means uQ's own two gestures.
fn lenient_gestures<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<Gesture>>, D::Error> {
    let Value::Array(items) = Value::deserialize(d)? else {
        return Ok(None);
    };
    Ok(Some(
        items
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let g = serde_json::from_value::<Gesture>(v.clone())
                    .map_err(|e| e.to_string())
                    .and_then(|g| {
                        g.validate(&format!("input.gestures[{i}]"))
                            .map(|()| g)
                            .map_err(|e| e.to_string())
                    });
                if let Err(e) = &g {
                    tracing::warn!("state: dropping gesture {v}: {e}");
                }
                g.ok()
            })
            .collect(),
    ))
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
    /// `None` is uQ's defaults ([`gestures::defaults`]); a list, even empty, is exactly that.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_gestures"
    )]
    pub gestures: Option<Vec<Gesture>>,
    #[serde(
        skip_serializing_if = "SwipeOptions::is_default",
        deserialize_with = "lenient"
    )]
    pub swipe: SwipeOptions,
}

impl InputSettings {
    /// The parts of the section's JSON (`mouse` is the fourth, read from `Settings::mouse`).
    pub const FIELDS: [&str; 6] = [
        "keyboard", "mouse", "touchpad", "devices", "gestures", "swipe",
    ];

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
        let gestures = match part("gestures") {
            None => None,
            Some(Value::Array(items)) => {
                let list = items
                    .iter()
                    .enumerate()
                    .map(|(i, g)| Gesture::from_value(&format!("input.gestures[{i}]"), g))
                    .collect::<Result<Vec<_>, _>>()?;
                gestures::validate(&list)?;
                Some(list)
            }
            Some(_) => return Err(invalid("input.gestures", "expected a list")),
        };
        let swipe = match part("swipe") {
            Some(v) => {
                let s: SwipeOptions = parse_strict("input.swipe", v, &SwipeOptions::FIELDS)?;
                s.validate()?;
                s.normalised()
            }
            None => SwipeOptions::default(),
        };
        Ok(Self {
            keyboard,
            touchpad,
            devices,
            gestures,
            swipe,
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
        let gestures = self
            .gestures
            .map(|list| repaired_gestures(list, &mut errors));
        let mut swipe = self.swipe;
        if swipe.distance.is_some_and(|d| d > 2000) {
            errors.push(invalid("input.swipe.distance", "cleared: out of range"));
            swipe.distance = None;
        }
        if swipe
            .cancel_ratio
            .is_some_and(|r| !r.is_finite() || !(0.0..=1.0).contains(&r))
        {
            errors.push(invalid("input.swipe.cancel_ratio", "cleared: out of range"));
            swipe.cancel_ratio = None;
        }
        (
            Self {
                keyboard,
                touchpad: touchpad.normalised(),
                devices,
                gestures,
                swipe: swipe.normalised(),
            },
            errors,
        )
    }
}

/// Drop the gestures a write would refuse: bad ones, the later of two that clash, and any past
/// the limit.
fn repaired_gestures(list: Vec<Gesture>, errors: &mut Vec<SettingError>) -> Vec<Gesture> {
    let mut kept: Vec<Gesture> = Vec::new();
    for (i, g) in list.into_iter().enumerate() {
        let key = format!("input.gestures[{i}]");
        if let Err(e) = g.validate(&key) {
            errors.push(e);
            continue;
        }
        if kept.len() >= MAX_GESTURES {
            errors.push(invalid(&key, "dropped: more than 24 gestures"));
            continue;
        }
        kept.push(g);
        if let Some(c) = gestures::clashes_of(&kept).into_iter().next() {
            tracing::warn!(
                "state: dropping a gesture: {}",
                crate::keybinds::conflict::message(&c)
            );
            errors.push(invalid(
                &format!("{key}.direction"),
                "dropped: it clashes with an earlier gesture",
            ));
            kept.pop();
        }
    }
    kept
}

/// One line for a change to the `input` section: gestures in words, every other leaf as
/// "label: old → new". Never JSON.
pub fn input_summary(before: &Value, after: &Value, labels: crate::history::Labels) -> String {
    const MAX_NAMED: usize = 3;
    let mut lines = gestures::summary(before, after);
    let rest = |v: &Value| {
        let mut v = v.clone();
        if let Some(o) = v.as_object_mut() {
            o.remove("gestures");
        }
        v
    };
    let (b, a) = (rest(before), rest(after));
    if b != a {
        lines.push(crate::history::summarize_leaves("input", &b, &a, labels));
    }
    let mut line = lines
        .iter()
        .take(MAX_NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    if line.is_empty() {
        line = "Input".into();
    }
    line
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
