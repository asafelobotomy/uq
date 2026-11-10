//! Restore points: full copies of `state.toml` with a name, a time and a kind.

use crate::atomic::atomic_write;
use crate::state::State;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};

/// Automatic points kept; older ones are pruned. Manual points are never pruned.
pub const MAX_AUTO: usize = 20;
const MAX_NAME_CHARS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoReason {
    Daily,
    BeforeRestore,
    BeforeProfile,
    BeforeImport,
    NewVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointKind {
    Auto(AutoReason),
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePoint {
    pub id: String,
    pub name: String,
    pub time: i64,
    pub kind: PointKind,
}

/// `[0-9]{10}-[0-9a-f]{6}`: the only shape of id that ever reaches a path.
fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 17
        && b[..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-'
        && b[11..]
            .iter()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect()
}

fn random_suffix() -> u32 {
    RandomState::new().build_hasher().finish() as u32 & 0x00ff_ffff
}

pub struct RestoreStore {
    dir: PathBuf,
}

impl RestoreStore {
    pub fn open(dir: impl Into<PathBuf>) -> RestoreStore {
        RestoreStore { dir: dir.into() }
    }

    fn state_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.toml"))
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.meta.json"))
    }

    fn checked(id: &str) -> io::Result<&str> {
        if valid_id(id) {
            Ok(id)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid restore point id {id:?}"),
            ))
        }
    }

    /// Store a copy of `state`. The state file is written first, so a point with metadata
    /// always has its state.
    pub fn save(
        &self,
        state: &State,
        name: &str,
        kind: PointKind,
        now: i64,
    ) -> io::Result<RestorePoint> {
        let id = loop {
            let id = format!("{:010}-{:06x}", now.max(0), random_suffix());
            if !self.state_path(&id).exists() && !self.meta_path(&id).exists() {
                break id;
            }
        };
        let point = RestorePoint {
            id,
            name: clean_name(name),
            time: now,
            kind,
        };
        state.save(&self.state_path(&point.id))?;
        let meta = serde_json::to_string(&point).map_err(io::Error::other)?;
        atomic_write(&self.meta_path(&point.id), &meta)?;
        Ok(point)
    }

    /// All points, newest first. Unreadable metadata is skipped.
    pub fn list(&self) -> Vec<RestorePoint> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut points: Vec<RestorePoint> = rd
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".meta.json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|s| serde_json::from_str::<RestorePoint>(&s).ok())
            .filter(|p| valid_id(&p.id))
            .collect();
        points.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.id.cmp(&a.id)));
        points
    }

    /// The point's state. An empty file, or one that doesn't parse as a `State`, is
    /// `InvalidData`. The file is never renamed or rewritten (unlike [`State::load`]).
    pub fn load(&self, id: &str) -> io::Result<State> {
        let path = self.state_path(Self::checked(id)?);
        let src = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no restore point {id}"),
                ));
            }
            other => other?,
        };
        let damaged = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("restore point {id} is damaged"),
            )
        };
        if src.trim().is_empty() {
            return Err(damaged());
        }
        State::parse_strict(&src).map_err(|_| damaged())
    }

    pub fn delete(&self, id: &str) -> io::Result<()> {
        Self::checked(id)?;
        let remove = |p: &Path| match std::fs::remove_file(p) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
        // Metadata first: a point without it is no longer listed.
        remove(&self.meta_path(id))?;
        remove(&self.state_path(id))
    }

    /// The newest automatic point with this reason.
    pub fn last_auto(&self, reason: AutoReason) -> Option<RestorePoint> {
        self.list()
            .into_iter()
            .find(|p| p.kind == PointKind::Auto(reason))
    }

    /// Keep the newest [`MAX_AUTO`] automatic points; manual points are never removed.
    pub fn prune(&self) {
        let autos = self
            .list()
            .into_iter()
            .filter(|p| matches!(p.kind, PointKind::Auto(_)));
        for p in autos.skip(MAX_AUTO) {
            if let Err(e) = self.delete(&p.id) {
                tracing::warn!("restore point {}: prune failed: {e}", p.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, RestoreStore) {
        let dir = tempfile::tempdir().unwrap();
        let s = RestoreStore::open(dir.path().join("restore"));
        (dir, s)
    }

    #[test]
    fn save_load_round_trip() {
        let (_d, s) = store();
        let mut state = State {
            theme: "nord".into(),
            ..State::default()
        };
        state.launch_counts.insert("kitty".into(), 3);
        let p = s
            .save(&state, "before", PointKind::Manual, 1_700_000_000)
            .unwrap();
        assert!(valid_id(&p.id), "{}", p.id);
        assert_eq!((p.name.as_str(), p.time), ("before", 1_700_000_000));
        assert_eq!(s.load(&p.id).unwrap(), state);
        assert_eq!(s.list(), vec![p]);
    }

    #[test]
    fn prune_keeps_20_autos_and_all_manual() {
        let (_d, s) = store();
        let st = State::default();
        for i in 0..25 {
            s.save(
                &st,
                "a",
                PointKind::Auto(AutoReason::Daily),
                1_700_000_000 + i,
            )
            .unwrap();
        }
        for i in 0..2 {
            s.save(&st, "m", PointKind::Manual, 1_600_000_000 + i)
                .unwrap();
        }
        s.prune();
        let list = s.list();
        assert_eq!(list.len(), 22);
        assert_eq!(
            list.iter().filter(|p| p.kind == PointKind::Manual).count(),
            2
        );
        let oldest_auto = list
            .iter()
            .filter(|p| p.kind != PointKind::Manual)
            .map(|p| p.time)
            .min();
        assert_eq!(oldest_auto, Some(1_700_000_005));
    }

    fn fs_read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn damaged_point_is_invalid_data_and_kept() {
        let (_d, s) = store();
        let p = s
            .save(&State::default(), "x", PointKind::Manual, 1)
            .unwrap();
        for damage in [
            "",
            "  \n",
            "theme = \"nord\"\n[look\n",
            "variant = \"purple\"\n",
            "theme = 5\n",
        ] {
            std::fs::write(s.state_path(&p.id), damage).unwrap();
            let err = s.load(&p.id).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "{damage:?}");
            assert_eq!(fs_read(&s.state_path(&p.id)), damage);
            assert_eq!(s.list().len(), 1);
        }
        // A value the loader repairs on its own costs only that value, as in `State::load`.
        std::fs::write(s.state_path(&p.id), "layouts = \"x\"\ntheme = \"nord\"\n").unwrap();
        let repaired = s.load(&p.id).unwrap();
        assert!(repaired.layouts.is_empty());
        assert_eq!(repaired.theme, "nord");
        let missing = s.load("1700000000-abcdef").unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn bad_id_is_refused() {
        let (_d, s) = store();
        for id in ["../x", "x", "", "1700000000-ABCDEF", "1700000000-abcdef/.."] {
            assert!(s.load(id).is_err(), "{id}");
            assert!(s.delete(id).is_err(), "{id}");
        }
    }

    #[test]
    fn name_is_capped_and_stripped() {
        let (_d, s) = store();
        let long = format!("a\u{0}b\nc\u{7f}{}", "x".repeat(200));
        let p = s
            .save(&State::default(), &long, PointKind::Manual, 1)
            .unwrap();
        assert_eq!(p.name.chars().count(), 100);
        assert!(p.name.starts_with("abcx"));
        assert!(!p.name.chars().any(char::is_control));
    }

    #[test]
    fn last_auto_by_reason() {
        let (_d, s) = store();
        let st = State::default();
        s.save(&st, "", PointKind::Auto(AutoReason::Daily), 10)
            .unwrap();
        let newer = s
            .save(&st, "", PointKind::Auto(AutoReason::Daily), 20)
            .unwrap();
        s.save(&st, "", PointKind::Manual, 30).unwrap();
        assert_eq!(s.last_auto(AutoReason::Daily), Some(newer));
        assert_eq!(s.last_auto(AutoReason::BeforeImport), None);
    }
}
