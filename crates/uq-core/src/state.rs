//! uqd's persisted desktop state (`~/.local/state/uq/state.toml`).

use crate::atomic::atomic_write;
use crate::lua::Layout;
use crate::schedule::Schedule;
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
}

impl Default for State {
    fn default() -> Self {
        State {
            theme: DEFAULT_THEME.into(),
            variant: Variant::Dark,
            schedule: Schedule::Off,
            layouts: BTreeMap::new(),
            schedule_mark: None,
        }
    }
}

impl State {
    /// Load state; a missing or unreadable file yields the defaults (with a warning if it existed).
    pub fn load(path: &Path) -> State {
        match std::fs::read_to_string(path) {
            Ok(src) => toml::from_str(&src).unwrap_or_else(|e| {
                tracing::warn!("{}: unreadable state, using defaults: {e}", path.display());
                State::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => {
                tracing::warn!("{}: {e}; using defaults", path.display());
                State::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = toml::to_string(self).map_err(std::io::Error::other)?;
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
    fn corrupt_state_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.toml");
        std::fs::write(&p, "{{nope").unwrap();
        assert_eq!(State::load(&p), State::default());
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
        };
        s.save(&p).unwrap();
        assert_eq!(State::load(&p), s);
    }
}
