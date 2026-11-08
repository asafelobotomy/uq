//! uqd's persisted desktop state (`~/.local/state/uq/state.toml`).

use crate::atomic::atomic_write;
use crate::lua::Layout;
use crate::schedule::Schedule;
use crate::settings::Settings;
use crate::theme::Variant;
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
    pub layouts: BTreeMap<String, Layout>,
    /// The scheduled transition uqd last applied; a manual change holds until a newer one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_mark: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(flatten)]
    pub settings: Settings,
    pub launch_counts: BTreeMap<String, u32>,
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
            schedule_mark: None,
            settings: Settings::default(),
            launch_counts: BTreeMap::new(),
        }
    }
}

impl State {
    /// Load state. A missing file yields the defaults. A file that won't parse is renamed to
    /// `state.toml.bad-<unix_secs>` first so pins and counts are never silently lost. Settings
    /// that parse but break a §2.4 invariant fall back to the defaults (the file is kept).
    pub fn load(path: &Path) -> State {
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return State::default(),
            Err(e) => {
                tracing::warn!("{}: {e}; using defaults", path.display());
                return State::default();
            }
        };
        let mut state = match Self::parse(&src) {
            Ok(state) => state,
            Err(e) => {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let mut bad = path.as_os_str().to_owned();
                bad.push(format!(".bad-{secs}"));
                match std::fs::rename(path, &bad) {
                    Ok(()) => tracing::warn!(
                        "{}: unreadable state ({e}); kept as {:?}, using defaults",
                        path.display(),
                        bad
                    ),
                    Err(re) => tracing::warn!(
                        "{}: unreadable state ({e}), could not move it aside ({re}); using defaults",
                        path.display()
                    ),
                }
                return State::default();
            }
        };
        match state.settings.validated() {
            Ok(valid) => state.settings = valid,
            Err(e) => {
                tracing::warn!(
                    "{}: invalid settings ({e}); using default settings",
                    path.display()
                );
                state.settings = Settings::default();
            }
        }
        state
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

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut table = toml::Table::try_from(self).map_err(std::io::Error::other)?;
        let mut settings = self.settings.clone();
        if let Some(idle) = table.get_mut("idle").and_then(toml::Value::as_table_mut) {
            for stage in IDLE_STAGES {
                if Self::stage_mut(&mut settings, stage).is_none() {
                    idle.insert(stage.into(), toml::Value::Integer(0));
                }
            }
        }
        let body = toml::to_string(&table).map_err(std::io::Error::other)?;
        atomic_write(path, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_graphite_dark() {
        let s = State::default();
        assert_eq!((s.theme.as_str(), s.variant), ("graphite", Variant::Dark));
        assert_eq!(s.schedule, Schedule::Off);
        assert!(s.layouts.is_empty());
    }

    #[test]
    fn missing_state_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(&dir.path().join("nope.toml")), State::default());
    }

    #[test]
    fn corrupt_state_is_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, "{{nope").unwrap();
        assert_eq!(State::load(&p), State::default());
        assert!(!p.exists());
        let bad: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|f| {
                f.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("state.toml.bad-")
            })
            .collect();
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert_eq!(std::fs::read(&bad[0]).unwrap(), b"{{nope");
    }

    #[test]
    fn phase1_state_upgrades() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, include_str!("../tests/fixtures/phase1-state.toml")).unwrap();
        let s = State::load(&p);
        assert_eq!(s.settings, Settings::default());
        assert!(s.launch_counts.is_empty());
        assert_eq!(s.theme, "dracula");
        assert_eq!(s.variant, Variant::Light);
        assert_eq!(s.layouts.get("2"), Some(&Layout::Scrolling));
        assert_eq!(s.layouts.get("special:magic"), Some(&Layout::Master));
        assert!(p.exists(), "a valid Phase 1 file is not renamed");
    }

    #[test]
    fn out_of_order_idle_on_load_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(
            &p,
            "launch_counts = { a = 3 }\n[idle]\ndim = 500\nlock = 300\n",
        )
        .unwrap();
        let s = State::load(&p);
        assert_eq!(s.settings, Settings::default());
        assert_eq!(s.launch_counts.get("a"), Some(&3), "other state is kept");
        assert!(p.exists(), "only unparsable files are renamed");
    }

    #[test]
    fn disabled_idle_stage_and_settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        let mut s = State::default();
        s.settings.idle.dim = None;
        s.settings.weather_location = Some(crate::settings::Location {
            name: "London".into(),
            lat: 51.51,
            lon: -0.13,
        });
        s.settings.pinned = vec!["firefox.desktop".into()];
        s.launch_counts.insert("firefox.desktop".into(), 7);
        s.save(&p).unwrap();
        assert_eq!(State::load(&p), s);
    }

    #[test]
    fn state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("uq/state.toml");
        let s = State {
            theme: "rosewood".into(),
            variant: Variant::Light,
            schedule: Schedule::Sun {
                lat: 51.5,
                lon: -0.13,
            },
            layouts: BTreeMap::from([
                ("2".into(), Layout::Scrolling),
                ("special:magic".into(), Layout::Master),
            ]),
            schedule_mark: None,
            settings: Settings::default(),
            launch_counts: BTreeMap::new(),
        };
        s.save(&p).unwrap();
        assert_eq!(State::load(&p), s);
    }
}
