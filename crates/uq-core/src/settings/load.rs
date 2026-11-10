//! Load-time leniency (spec ruling: loading is lenient, the API is strict) and the per-domain
//! repair of settings that came from a hand-edited or newer `state.toml`.

use super::{
    IdleTimings, MAX_PINNED, SETTING_KEYS, SettingError, Settings, invalid, valid_pinned_id,
};
use serde::Deserialize;
use serde_json::Value;

/// Load-time leniency: a value that doesn't parse as `T` (wrong type, out of the type's range,
/// or a newer uQ's) loads as `T::default()` instead of failing the whole file.
pub(crate) fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    Ok(T::deserialize(Value::deserialize(d)?).unwrap_or_default())
}

/// Pins load entry by entry: non-strings are dropped here, invalid ids in `validated`.
pub(super) fn lenient_pins<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(a) => a
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    })
}

/// The API side of load leniency: a lenient parse drops a value it can't read, so every
/// non-null value of `input` must survive into `output` (the parsed section re-serialised).
pub(crate) fn check_kept(
    key: &str,
    input: &Value,
    output: Option<&Value>,
) -> Result<(), SettingError> {
    match (input, output) {
        (Value::Null, _) => Ok(()),
        (Value::Object(i), Some(Value::Object(o))) => i
            .iter()
            .try_for_each(|(k, v)| check_kept(&format!("{key}.{k}"), v, o.get(k))),
        (Value::Object(_), _) | (_, None | Some(Value::Object(_))) => {
            Err(invalid(key, "wrong type or out of range"))
        }
        _ => Ok(()),
    }
}

impl Settings {
    /// Re-validate settings that came from outside (a hand-edited file), per domain: each
    /// Phase 2 group is replayed through [`Settings::apply`] from a blank idle table, so the
    /// result is normalised (de-duplicated, rounded) and holds every §2.4 invariant. A domain
    /// that breaks a rule falls back to its default alone; everything else is kept. Invalid,
    /// duplicate and surplus pins are dropped one by one. Returns what was repaired.
    pub fn validated(&self) -> (Settings, Vec<SettingError>) {
        let json = serde_json::to_value(self).expect("settings serialise");
        let mut errors = Vec::new();
        let mut out = Settings {
            idle: IdleTimings {
                dim: None,
                lock: None,
                screen_off: None,
                suspend_battery: None,
                ..IdleTimings::default()
            },
            ..Settings::default()
        };
        let pins: Vec<String> =
            self.pinned
                .iter()
                .filter(|id| valid_pinned_id(id))
                .fold(Vec::new(), |mut acc, id| {
                    if !acc.contains(id) {
                        acc.push(id.clone());
                    }
                    acc
                });
        if pins.len() != self.pinned.len() || pins.len() > MAX_PINNED {
            errors.push(invalid(
                "pinned",
                "dropped invalid, duplicate or surplus pins",
            ));
        }
        out.pinned = pins.into_iter().take(MAX_PINNED).collect();
        for group in ["weather_location", "idle", "notif", "privacy", "mouse"] {
            let mut next = out.clone();
            let replay = SETTING_KEYS
                .iter()
                .filter(|k| k.split('.').next() == Some(group))
                .try_for_each(|key| {
                    let value = key
                        .split('.')
                        .try_fold(&json, |v, part| v.get(part))
                        .cloned()
                        .unwrap_or(Value::Null);
                    next.apply(key, value)
                });
            match replay {
                Ok(()) => out = next,
                Err(e) => {
                    errors.push(e);
                    let d = Settings::default();
                    match group {
                        "weather_location" => out.weather_location = d.weather_location,
                        "idle" => out.idle = d.idle,
                        "notif" => out.notif = d.notif,
                        "privacy" => out.privacy = d.privacy,
                        _ => out.mouse = d.mouse,
                    }
                }
            }
        }
        match self.appearance.validate() {
            Ok(()) => out.appearance = self.appearance.clone(),
            Err(e) => errors.push(e),
        }
        match self.wallpaper.validate() {
            Ok(()) => out.wallpaper = self.wallpaper.clone(),
            Err(e) => errors.push(e),
        }
        match self.look.clone().normalised() {
            Ok(l) => out.look = l,
            Err(e) => errors.push(e),
        }
        match self.layout_options.clone().normalised() {
            Ok(l) => out.layout_options = l,
            Err(e) => errors.push(e),
        }
        out.shell = self.shell.clone().sanitised();
        let (monitors, repaired) = self.monitors.clone().sanitised();
        errors.extend(repaired);
        out.monitors = monitors;
        (out, errors)
    }
}
