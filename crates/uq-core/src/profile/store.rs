//! The profile directory: one `<id>.toml` per profile.

use super::{Profile, ProfileError, read_file, valid_id};
use crate::atomic::atomic_write;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProfileStore {
    dir: PathBuf,
}

fn bad_id() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, ProfileError::BadId)
}

fn bad_file(e: ProfileError) -> io::Error {
    match e {
        ProfileError::Unreadable(m) => io::Error::other(m),
        e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}

impl ProfileStore {
    pub fn open(dir: PathBuf) -> ProfileStore {
        ProfileStore { dir }
    }

    fn file(&self, id: &str) -> io::Result<PathBuf> {
        if valid_id(id) {
            Ok(self.dir.join(format!("{id}.toml")))
        } else {
            Err(bad_id())
        }
    }

    /// Every good profile by name (case-insensitive), then id. A file that doesn't parse, or
    /// whose id isn't its file name, is skipped.
    pub fn list(&self) -> Vec<Profile> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match read_file(&path).and_then(|t| Profile::parse(&t)) {
                Ok(p) if p.id == stem => out.push(p),
                Ok(p) => {
                    tracing::warn!(?path, id = %p.id, "profile id isn't its file name; skipped")
                }
                Err(e) => tracing::warn!(?path, error = %e, "profile file skipped"),
            }
        }
        out.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
        out
    }

    /// Profile `id`; a file whose id isn't its name is `NotFound`, as `list` skips it.
    pub fn load(&self, id: &str) -> io::Result<Profile> {
        let path = self.file(id)?;
        let text = read_file(&path).map_err(|e| match e {
            ProfileError::Unreadable(_) => {
                // Keep the real kind (NotFound) rather than a message.
                std::fs::metadata(&path)
                    .err()
                    .unwrap_or_else(|| bad_file(e))
            }
            e => bad_file(e),
        })?;
        let p = Profile::parse(&text).map_err(bad_file)?;
        if p.id != id {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "id isn't the file name",
            ));
        }
        Ok(p)
    }

    pub fn save(&self, p: &Profile) -> io::Result<()> {
        let path = self.file(&p.id)?;
        p.validate().map_err(bad_file)?;
        atomic_write(&path, &p.to_toml())
    }

    pub fn delete(&self, id: &str) -> io::Result<()> {
        std::fs::remove_file(self.file(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn profile(id: &str, name: &str) -> Profile {
        Profile {
            id: id.into(),
            name: name.into(),
            icon: None,
            values: BTreeMap::from([("look".to_string(), json!({"gaps_in": 3}))]),
        }
    }

    #[test]
    fn store_round_trip_sorted_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::open(dir.path().join("profiles"));
        assert!(store.list().is_empty());
        let (a, b, c) = (
            profile("b", "work"),
            profile("a", "Gaming"),
            profile("c", "Work"),
        );
        for p in [&a, &b, &c] {
            store.save(p).unwrap();
        }
        let ids: Vec<_> = store.list().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(store.load("a").unwrap(), b);
        store.delete("a").unwrap();
        assert_eq!(store.load("a").unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn store_skips_bad_and_mismatched_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::open(dir.path().to_path_buf());
        store.save(&profile("good", "Good")).unwrap();
        std::fs::write(dir.path().join("junk.toml"), "not [toml").unwrap();
        std::fs::write(dir.path().join("note.txt"), "hello").unwrap();
        std::fs::write(
            dir.path().join("other.toml"),
            profile("good2", "Elsewhere").to_toml(),
        )
        .unwrap();
        let ids: Vec<_> = store.list().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["good"]);
        assert_eq!(
            store.load("junk").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        // As `list`, a file whose id isn't its name isn't that profile.
        assert_eq!(
            store.load("other").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn store_refuses_bad_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::open(dir.path().to_path_buf());
        for id in ["", "../x", "A"] {
            assert_eq!(
                store.load(id).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{id}"
            );
            assert_eq!(
                store.delete(id).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{id}"
            );
            assert_eq!(
                store.save(&profile(id, "X")).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{id}"
            );
        }
    }
}
