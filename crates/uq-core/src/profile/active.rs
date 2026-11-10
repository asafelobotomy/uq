//! The active profile's memory, kept in `profile-active.json` so it survives a uqd restart.

use super::ActiveProfile;
use crate::atomic::atomic_write;
use std::io;
use std::path::Path;

/// A missing file is `None`; an unreadable or damaged one is `None` with a warning.
pub fn load_active(path: &Path) -> Option<ActiveProfile> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!("can't read {}: {e}", path.display());
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!("ignoring damaged {}: {e}", path.display());
            None
        }
    }
}

pub fn save_active(path: &Path, a: &ActiveProfile) -> io::Result<()> {
    let text = serde_json::to_string_pretty(a).map_err(io::Error::other)?;
    atomic_write(path, &text)
}

/// Remove the file; one that isn't there is fine.
pub fn clear_active(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{KeyMemo, PowerMemo};
    use serde_json::json;

    fn sample() -> ActiveProfile {
        ActiveProfile {
            id: "gaming".into(),
            name: "Gaming".into(),
            since: 42,
            keys: vec![
                KeyMemo {
                    domain: "look".into(),
                    path: vec!["gaps_in".into()],
                    before: Some(json!(5)),
                    set: json!(0),
                },
                KeyMemo {
                    domain: "look".into(),
                    path: vec!["blur".into()],
                    before: None,
                    set: json!(false),
                },
            ],
            power: Some(PowerMemo {
                before: "balanced".into(),
                set: "performance".into(),
            }),
        }
    }

    #[test]
    fn active_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/profile-active.json");
        assert_eq!(load_active(&path), None);
        save_active(&path, &sample()).unwrap();
        assert_eq!(load_active(&path), Some(sample()));
        clear_active(&path).unwrap();
        assert_eq!(load_active(&path), None);
        clear_active(&path).unwrap();
    }

    #[test]
    fn damaged_active_file_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile-active.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_active(&path), None);
        std::fs::write(&path, r#"{"id":"x"}"#).unwrap();
        assert_eq!(load_active(&path), None);
    }
}
