//! Validated user settings (spec §2.4): the model, its defaults, and per-key validation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every key `Settings::apply` accepts; anything else is rejected.
pub const SETTING_KEYS: [&str; 12] = [
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
];

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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub pinned: Vec<String>,
    pub weather_location: Option<Location>,
    pub idle: IdleTimings,
    pub notif: NotifSettings,
    pub privacy: PrivacySettings,
}

fn invalid(key: &str, reason: impl Into<String>) -> SettingError {
    SettingError::Invalid {
        key: key.to_string(),
        reason: reason.into(),
    }
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

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

impl Settings {
    /// Re-validate settings that came from outside (a hand-edited file): every key is replayed
    /// through [`Settings::apply`] from a blank idle table, so the result is normalised
    /// (de-duplicated, rounded) and holds every §2.4 invariant.
    pub fn validated(&self) -> Result<Settings, SettingError> {
        let json = serde_json::to_value(self).expect("settings serialise");
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
        for key in SETTING_KEYS {
            let value = key
                .split('.')
                .try_fold(&json, |v, part| v.get(part))
                .cloned()
                .unwrap_or(Value::Null);
            out.apply(key, value)?;
        }
        Ok(out)
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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert!(s.pinned.is_empty());
        assert_eq!(s.weather_location, None);
        assert_eq!(
            (
                s.idle.dim,
                s.idle.lock,
                s.idle.screen_off,
                s.idle.suspend_battery
            ),
            (Some(240), Some(300), Some(360), Some(1200))
        );
        assert_eq!(s.idle.locked_screen_off, 30);
        assert!(!s.idle.media_blocks_lock);
        assert_eq!(s.notif.backend, NotifBackend::Auto);
        assert!(!s.notif.show_summary);
        assert!(s.privacy.public_ip_lookup);
        assert!(s.privacy.record_launches);
    }

    #[test]
    fn rejects_unknown_key() {
        let mut s = Settings::default();
        assert_eq!(
            s.apply("theme.x", json!(1)),
            Err(SettingError::UnknownKey("theme.x".into()))
        );
    }

    #[test]
    fn pinned_validation() {
        let mut s = Settings::default();
        let many: Vec<String> = (0..33).map(|i| format!("app{i}.desktop")).collect();
        assert!(s.apply("pinned", json!(many)).is_err());
        assert!(s.apply("pinned", json!(["../evil"])).is_err());
        assert!(s.apply("pinned", json!([1])).is_err());
        s.apply("pinned", json!(["firefox.desktop", "a", "firefox.desktop"]))
            .unwrap();
        assert_eq!(s.pinned, ["firefox.desktop", "a"]);
        let long = "a".repeat(129);
        assert!(s.apply("pinned", json!([long])).is_err());
        let ok = format!("{}.desktop", "a".repeat(128));
        assert!(s.apply("pinned", json!([ok])).is_ok());
    }

    #[test]
    fn idle_order_enforced() {
        let mut s = Settings::default();
        assert!(s.apply("idle.lock", json!(200)).is_err());
        s.apply("idle.dim", Value::Null).unwrap();
        s.apply("idle.lock", json!(200)).unwrap();
        assert_eq!(s.idle.lock, Some(200));
        assert!(s.apply("idle.lock", json!(29)).is_err());
        assert!(s.apply("idle.lock", json!(86401)).is_err());
        assert!(s.apply("idle.lock", json!(60.5)).is_err());
        assert!(s.apply("idle.screen_off", json!(200)).is_err());
    }

    #[test]
    fn locked_screen_off_range() {
        let mut s = Settings::default();
        assert!(s.apply("idle.locked_screen_off", json!(9)).is_err());
        assert!(s.apply("idle.locked_screen_off", json!(601)).is_err());
        s.apply("idle.locked_screen_off", json!(10)).unwrap();
        assert_eq!(s.idle.locked_screen_off, 10);
    }

    #[test]
    fn location_validation() {
        let mut s = Settings::default();
        assert!(
            s.apply("weather_location", json!({"name":"x","lat":91,"lon":0}))
                .is_err()
        );
        assert!(
            s.apply("weather_location", json!({"name":"x","lat":0,"lon":181}))
                .is_err()
        );
        let long = "n".repeat(101);
        assert!(
            s.apply("weather_location", json!({"name":long,"lat":0,"lon":0}))
                .is_err()
        );
        s.apply(
            "weather_location",
            json!({"name":"London","lat":51.50735,"lon":-0.12776}),
        )
        .unwrap();
        let loc = s.weather_location.clone().unwrap();
        assert_eq!((loc.lat, loc.lon), (51.51, -0.13));
        s.apply("weather_location", Value::Null).unwrap();
        assert_eq!(s.weather_location, None);
    }

    #[test]
    fn backend_enum() {
        let mut s = Settings::default();
        s.apply("notif.backend", json!("notifi")).unwrap();
        assert_eq!(s.notif.backend, NotifBackend::Notifi);
        assert_eq!(
            serde_json::to_value(s.notif.backend).unwrap(),
            json!("notifi")
        );
        assert!(s.apply("notif.backend", json!("growl")).is_err());
    }

    #[test]
    fn bools_apply() {
        let mut s = Settings::default();
        s.apply("privacy.public_ip_lookup", json!(false)).unwrap();
        s.apply("notif.show_summary", json!(true)).unwrap();
        assert!(!s.privacy.public_ip_lookup && s.notif.show_summary);
        assert!(s.apply("privacy.record_launches", json!("yes")).is_err());
    }

    #[test]
    fn apply_is_atomic() {
        let mut s = Settings::default();
        s.apply("pinned", json!(["a.desktop"])).unwrap();
        let before = s.clone();
        assert!(s.apply("idle.lock", json!(200)).is_err());
        assert!(s.apply("pinned", json!(["ok", "../bad"])).is_err());
        assert!(s.apply("nope", json!(1)).is_err());
        assert_eq!(s, before);
    }

    #[test]
    fn every_setting_key_is_known() {
        for k in SETTING_KEYS {
            let mut s = Settings::default();
            let r = s.apply(k, json!("zzz"));
            assert!(!matches!(r, Err(SettingError::UnknownKey(_))), "{k}");
        }
    }
}
