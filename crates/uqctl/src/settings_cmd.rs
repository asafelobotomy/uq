//! `uqctl` settings commands: pin, weather, idle, notif, privacy and launches. The mapping from
//! a command to the `SetSetting` call it makes is pure and tested here; `main.rs` does the D-Bus.

use crate::bridge::net::GeoHit;
use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};

#[derive(Subcommand, Debug, PartialEq)]
pub enum PinCmd {
    /// Pin a desktop entry id (e.g. `org.mozilla.firefox`).
    Add { id: String },
    /// Unpin a desktop entry id.
    Rm { id: String },
    /// Print the pinned ids, one per line.
    List,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum WeatherCmd {
    /// Look a city up, print the top 5 matches and store the first (or --pick N, 1-based).
    Set {
        city: String,
        #[arg(long, default_value_t = 1)]
        pick: usize,
    },
    /// Stop showing weather.
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum IdleStage {
    Dim,
    Lock,
    ScreenOff,
    Suspend,
    LockedScreenOff,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum IdleCmd {
    /// e.g. `uqctl idle set lock 300` or `uqctl idle set screen-off off`.
    Set {
        stage: IdleStage,
        /// Seconds, or `off`.
        value: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum NotifCmd {
    /// Choose who owns org.freedesktop.Notifications (e.g. `shell`, `none`).
    Backend { name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PrivacyKey {
    PublicIp,
    Launches,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum LaunchesCmd {
    /// Forget every recorded launch count.
    Clear,
}

/// `SetSetting(key, json)`.
pub type Setting = (&'static str, Value);

pub fn idle_setting(stage: IdleStage, value: &str) -> Result<Setting, String> {
    let key = match stage {
        IdleStage::Dim => "idle.dim",
        IdleStage::Lock => "idle.lock",
        IdleStage::ScreenOff => "idle.screen_off",
        IdleStage::Suspend => "idle.suspend_battery",
        IdleStage::LockedScreenOff => "idle.locked_screen_off",
    };
    if value == "off" {
        return Ok((key, Value::Null));
    }
    // uqd checks the 30-86400 range and the stage ordering; this only needs a whole number.
    let secs: u32 = value
        .parse()
        .map_err(|_| format!("{value:?} is not a number of seconds (or `off`)"))?;
    Ok((key, json!(secs)))
}

pub fn privacy_setting(key: PrivacyKey, state: OnOff) -> Setting {
    let key = match key {
        PrivacyKey::PublicIp => "privacy.public_ip_lookup",
        PrivacyKey::Launches => "privacy.record_launches",
    };
    (key, json!(state == OnOff::On))
}

pub fn notif_backend_setting(name: &str) -> Setting {
    ("notif.backend", json!(name))
}

/// The new `pinned` list after adding, or `None` when `id` is already there.
pub fn pinned_with(current: &[String], id: &str) -> Option<Vec<String>> {
    if current.iter().any(|p| p == id) {
        return None;
    }
    let mut next = current.to_vec();
    next.push(id.to_string());
    Some(next)
}

/// The new `pinned` list after removing, or `None` when `id` is not pinned.
pub fn pinned_without(current: &[String], id: &str) -> Option<Vec<String>> {
    current
        .iter()
        .any(|p| p == id)
        .then(|| current.iter().filter(|p| *p != id).cloned().collect())
}

/// The pinned ids in a `GetSettings` JSON object.
pub fn pinned_of(settings: &Value) -> Vec<String> {
    settings["pinned"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn hit_label(h: &GeoHit) -> String {
    if h.country.is_empty() {
        h.name.clone()
    } else {
        format!("{}, {}", h.name, h.country)
    }
}

/// What `weather set` prints and stores: a line per match, and the chosen `weather_location`.
/// `pick` is 1-based.
pub fn weather_choice(hits: &[GeoHit], pick: usize) -> Result<(Vec<String>, Setting), String> {
    if hits.is_empty() {
        return Err("no match for that city".into());
    }
    let chosen = pick
        .checked_sub(1)
        .and_then(|i| hits.get(i))
        .ok_or_else(|| format!("--pick must be between 1 and {}", hits.len()))?;
    let lines = hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mark = if i + 1 == pick { "*" } else { " " };
            format!(
                "{mark}{}. {} ({:.2}, {:.2})",
                i + 1,
                hit_label(h),
                h.lat,
                h.lon
            )
        })
        .collect();
    let value = json!({"name": hit_label(chosen), "lat": chosen.lat, "lon": chosen.lon});
    Ok((lines, ("weather_location", value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Cmd};
    use clap::Parser;

    fn parse(args: &[&str]) -> Cmd {
        let mut v = vec!["uqctl"];
        v.extend_from_slice(args);
        Cli::try_parse_from(v).unwrap().cmd
    }

    fn hit(name: &str, country: &str, lat: f64, lon: f64) -> GeoHit {
        GeoHit {
            name: name.into(),
            country: country.into(),
            lat,
            lon,
        }
    }

    #[test]
    fn parses_every_new_subcommand() {
        assert!(matches!(
            parse(&["pin", "add", "org.a"]),
            Cmd::Pin { cmd: PinCmd::Add { ref id } } if id == "org.a"
        ));
        assert!(matches!(
            parse(&["pin", "rm", "org.a"]),
            Cmd::Pin {
                cmd: PinCmd::Rm { .. }
            }
        ));
        assert!(matches!(
            parse(&["pin", "list"]),
            Cmd::Pin { cmd: PinCmd::List }
        ));
        assert!(matches!(
            parse(&["weather", "set", "Paris", "--pick", "2"]),
            Cmd::Weather { cmd: WeatherCmd::Set { ref city, pick: 2 } } if city == "Paris"
        ));
        assert!(matches!(
            parse(&["weather", "set", "Paris"]),
            Cmd::Weather {
                cmd: WeatherCmd::Set { pick: 1, .. }
            }
        ));
        assert!(matches!(
            parse(&["weather", "off"]),
            Cmd::Weather {
                cmd: WeatherCmd::Off
            }
        ));
        assert!(matches!(
            parse(&["idle", "set", "locked-screen-off", "off"]),
            Cmd::Idle { cmd: IdleCmd::Set { stage: IdleStage::LockedScreenOff, ref value } } if value == "off"
        ));
        assert!(matches!(
            parse(&["notif", "backend", "shell"]),
            Cmd::Notif { cmd: NotifCmd::Backend { ref name } } if name == "shell"
        ));
        assert!(matches!(
            parse(&["privacy", "public-ip", "on"]),
            Cmd::Privacy {
                key: PrivacyKey::PublicIp,
                state: OnOff::On
            }
        ));
        assert!(matches!(
            parse(&["privacy", "launches", "off"]),
            Cmd::Privacy {
                key: PrivacyKey::Launches,
                state: OnOff::Off
            }
        ));
        assert!(matches!(
            parse(&["launches", "clear"]),
            Cmd::Launches {
                cmd: LaunchesCmd::Clear
            }
        ));
    }

    #[test]
    fn rejects_bad_arguments() {
        for bad in [
            &["idle", "set", "nap", "5"][..],
            &["privacy", "public-ip", "maybe"],
            &["pin", "add"],
            &["weather", "set"],
        ] {
            let mut v = vec!["uqctl"];
            v.extend_from_slice(bad);
            assert!(Cli::try_parse_from(v).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn idle_maps_to_setting_keys() {
        assert_eq!(
            idle_setting(IdleStage::Lock, "off").unwrap(),
            ("idle.lock", Value::Null)
        );
        assert_eq!(
            idle_setting(IdleStage::Dim, "240").unwrap(),
            ("idle.dim", json!(240))
        );
        assert_eq!(
            idle_setting(IdleStage::ScreenOff, "360").unwrap().0,
            "idle.screen_off"
        );
        assert_eq!(
            idle_setting(IdleStage::LockedScreenOff, "30").unwrap().0,
            "idle.locked_screen_off"
        );
        assert_eq!(
            idle_setting(IdleStage::Suspend, "1200").unwrap().0,
            "idle.suspend_battery"
        );
    }

    #[test]
    fn idle_rejects_non_numbers() {
        for bad in ["soon", "-5", "1.5", ""] {
            let err = idle_setting(IdleStage::Lock, bad).unwrap_err();
            assert!(err.contains("seconds"), "{err}");
        }
    }

    #[test]
    fn privacy_and_notif_settings() {
        assert_eq!(
            privacy_setting(PrivacyKey::PublicIp, OnOff::On),
            ("privacy.public_ip_lookup", json!(true))
        );
        assert_eq!(
            privacy_setting(PrivacyKey::Launches, OnOff::Off),
            ("privacy.record_launches", json!(false))
        );
        assert_eq!(
            notif_backend_setting("shell"),
            ("notif.backend", json!("shell"))
        );
    }

    #[test]
    fn pin_list_edits() {
        let cur = vec!["a".to_string(), "b".to_string()];
        assert_eq!(pinned_with(&cur, "c").unwrap(), ["a", "b", "c"]);
        assert_eq!(pinned_with(&cur, "a"), None);
        assert_eq!(pinned_without(&cur, "a").unwrap(), ["b"]);
        assert_eq!(pinned_without(&cur, "z"), None);
        assert_eq!(
            pinned_of(&json!({"pinned": ["x", "y"], "idle": {}})),
            ["x", "y"]
        );
        assert!(pinned_of(&json!({})).is_empty());
    }

    #[test]
    fn weather_choice_picks_one_based() {
        let hits = [
            hit("Paris", "France", 48.85, 2.35),
            hit("Paris", "United States", 33.66, -95.55),
        ];
        let (lines, (key, value)) = weather_choice(&hits, 1).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(key, "weather_location");
        assert_eq!(
            value,
            json!({"name": "Paris, France", "lat": 48.85, "lon": 2.35})
        );
        let (_, (_, value)) = weather_choice(&hits, 2).unwrap();
        assert_eq!(value["lat"], 33.66);
        assert!(weather_choice(&hits, 0).is_err());
        assert!(weather_choice(&hits, 3).is_err());
        assert!(weather_choice(&[], 1).unwrap_err().contains("no match"));
    }
}
