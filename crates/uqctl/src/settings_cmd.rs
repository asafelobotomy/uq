//! `uqctl` settings commands: pin, weather, idle, notif, privacy, mouse and launches. The mapping from
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AccelChoice {
    /// The device's own libinput default.
    Default,
    Adaptive,
    Flat,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum MouseCmd {
    /// Print the current mouse settings.
    Show,
    /// Pointer speed, -1 (slowest) to 1 (fastest); 0 leaves it unchanged.
    #[command(allow_negative_numbers = true)]
    Speed { value: f64 },
    /// Pointer acceleration profile.
    Accel { profile: AccelChoice },
    /// Reverse the wheel direction.
    NaturalScroll { state: OnOff },
    /// Swap the left and right buttons.
    LeftHanded { state: OnOff },
    /// Wheel scroll multiplier, 0.1 to 10.
    ScrollFactor { value: f64 },
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

/// The `SetSetting` a mouse command makes; `None` for `show`. uqd checks the ranges.
pub fn mouse_setting(cmd: &MouseCmd) -> Option<Setting> {
    Some(match *cmd {
        MouseCmd::Show => return None,
        MouseCmd::Speed { value } => ("mouse.sensitivity", json!(value)),
        MouseCmd::Accel { profile } => (
            "mouse.accel_profile",
            json!(match profile {
                AccelChoice::Default => "default",
                AccelChoice::Adaptive => "adaptive",
                AccelChoice::Flat => "flat",
            }),
        ),
        MouseCmd::NaturalScroll { state } => ("mouse.natural_scroll", json!(state == OnOff::On)),
        MouseCmd::LeftHanded { state } => ("mouse.left_handed", json!(state == OnOff::On)),
        MouseCmd::ScrollFactor { value } => ("mouse.scroll_factor", json!(value)),
    })
}

/// What `mouse show` prints, from a `GetSettings` JSON object.
pub fn mouse_lines(settings: &Value) -> Vec<String> {
    let m = &settings["mouse"];
    let on_off = |v: &Value| {
        if v.as_bool() == Some(true) {
            "on"
        } else {
            "off"
        }
    };
    vec![
        format!("speed\t{}", m["sensitivity"].as_f64().unwrap_or(0.0)),
        format!(
            "accel\t{}",
            m["accel_profile"].as_str().unwrap_or("default")
        ),
        format!("natural-scroll\t{}", on_off(&m["natural_scroll"])),
        format!("left-handed\t{}", on_off(&m["left_handed"])),
        format!(
            "scroll-factor\t{}",
            m["scroll_factor"].as_f64().unwrap_or(1.0)
        ),
    ]
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
    fn parses_mouse_commands() {
        assert!(matches!(
            parse(&["mouse", "show"]),
            Cmd::Mouse {
                cmd: MouseCmd::Show
            }
        ));
        assert_eq!(
            parse_mouse(&["speed", "-0.4"]),
            MouseCmd::Speed { value: -0.4 }
        );
        assert_eq!(
            parse_mouse(&["accel", "flat"]),
            MouseCmd::Accel {
                profile: AccelChoice::Flat
            }
        );
        assert_eq!(
            parse_mouse(&["natural-scroll", "on"]),
            MouseCmd::NaturalScroll { state: OnOff::On }
        );
        assert_eq!(
            parse_mouse(&["left-handed", "off"]),
            MouseCmd::LeftHanded { state: OnOff::Off }
        );
        assert_eq!(
            parse_mouse(&["scroll-factor", "2"]),
            MouseCmd::ScrollFactor { value: 2.0 }
        );
        for bad in [
            &["mouse", "speed", "fast"][..],
            &["mouse", "accel", "custom"],
            &["mouse", "left-handed", "yes"],
            &["mouse", "speed"],
        ] {
            let mut v = vec!["uqctl"];
            v.extend_from_slice(bad);
            assert!(Cli::try_parse_from(v).is_err(), "{bad:?}");
        }
    }

    fn parse_mouse(args: &[&str]) -> MouseCmd {
        let mut v = vec!["mouse"];
        v.extend_from_slice(args);
        match parse(&v) {
            Cmd::Mouse { cmd } => cmd,
            _ => panic!("not a mouse command: {args:?}"),
        }
    }

    #[test]
    fn mouse_maps_to_setting_keys() {
        assert_eq!(mouse_setting(&MouseCmd::Show), None);
        assert_eq!(
            mouse_setting(&MouseCmd::Speed { value: -0.4 }),
            Some(("mouse.sensitivity", json!(-0.4)))
        );
        assert_eq!(
            mouse_setting(&MouseCmd::Accel {
                profile: AccelChoice::Default
            }),
            Some(("mouse.accel_profile", json!("default")))
        );
        assert_eq!(
            mouse_setting(&MouseCmd::NaturalScroll { state: OnOff::On }),
            Some(("mouse.natural_scroll", json!(true)))
        );
        assert_eq!(
            mouse_setting(&MouseCmd::LeftHanded { state: OnOff::Off }),
            Some(("mouse.left_handed", json!(false)))
        );
        assert_eq!(
            mouse_setting(&MouseCmd::ScrollFactor { value: 1.5 }),
            Some(("mouse.scroll_factor", json!(1.5)))
        );
    }

    #[test]
    fn mouse_show_lines() {
        let v = json!({"mouse": {"sensitivity": -0.5, "accel_profile": "flat",
            "natural_scroll": true, "left_handed": false, "scroll_factor": 1.0}});
        assert_eq!(
            mouse_lines(&v),
            [
                "speed\t-0.5",
                "accel\tflat",
                "natural-scroll\ton",
                "left-handed\toff",
                "scroll-factor\t1"
            ]
        );
        // An older uqd without mouse settings shows the defaults.
        assert_eq!(mouse_lines(&json!({}))[0], "speed\t0");
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
