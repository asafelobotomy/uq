//! `uqctl` settings commands: pin, weather, idle, notif, privacy, mouse, launches, history and
//! restore points. The mapping from a command to the call it makes, and the printing of what uqd
//! answers, are pure and tested here; `main.rs` does the D-Bus.

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

#[derive(Subcommand, Debug, PartialEq)]
pub enum RestoreCmd {
    /// Print the restore points: id, time, kind and name.
    List,
    /// Save the current settings as a restore point named NAME.
    Save { name: String },
    /// Delete a restore point.
    Delete { id: String },
    /// `uqctl restore <id>`: bring back every setting as it was in that point.
    #[command(external_subcommand)]
    Restore(Vec<String>),
}

/// Unix seconds as local `YYYY-MM-DD HH:MM:SS`.
fn local_time(secs: i64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(secs, 0).earliest().map_or_else(
        || secs.to_string(),
        |t| t.format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

fn parse_list(json: &str, what: &str) -> Result<Vec<Value>, String> {
    serde_json::from_str(json).map_err(|e| format!("bad {what} from uqd: {e}"))
}

/// `ListHistory`'s JSON as `id  time  origin  summary` lines, newest first.
pub fn history_lines(json: &str) -> Result<Vec<String>, String> {
    Ok(parse_list(json, "history")?
        .iter()
        .map(|e| {
            format!(
                "{}  {}  {}  {}",
                e["id"].as_str().unwrap_or("?"),
                local_time(e["time"].as_i64().unwrap_or(0)),
                e["origin"].as_str().unwrap_or("unknown"),
                e["summary"].as_str().unwrap_or(""),
            )
        })
        .collect())
}

/// The newest entry's id in `ListHistory`'s JSON, if any.
pub fn newest_entry_id(json: &str) -> Result<Option<String>, String> {
    Ok(parse_list(json, "history")?
        .first()
        .and_then(|e| e["id"].as_str())
        .map(str::to_owned))
}

/// What `uqctl undo` prints: the id of the entry the undo recorded, or "nothing changed".
pub fn undo_line(new_entry_id: &str) -> String {
    if new_entry_id.is_empty() {
        "nothing changed".into()
    } else {
        new_entry_id.to_string()
    }
}

/// `ListRestorePoints`' JSON as `id  time  kind  name` lines; kind is `manual` or `auto:<why>`.
pub fn restore_lines(json: &str) -> Result<Vec<String>, String> {
    Ok(parse_list(json, "restore points")?
        .iter()
        .map(|p| {
            let kind = match &p["kind"] {
                Value::String(k) => k.clone(),
                Value::Object(o) => o
                    .iter()
                    .map(|(k, v)| format!("{k}:{}", v.as_str().unwrap_or("?")))
                    .collect(),
                _ => "?".into(),
            };
            format!(
                "{}  {}  {}  {}",
                p["id"].as_str().unwrap_or("?"),
                local_time(p["time"].as_i64().unwrap_or(0)),
                kind,
                p["name"].as_str().unwrap_or(""),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests;
