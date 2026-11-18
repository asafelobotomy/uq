//! The clipboard history on disk.
//!
//! `index.json` lists the entries, newest first; each entry's bytes are in `blobs/<id>`. Both are
//! private to the user (0700 directory, 0600 files) and written through a temporary file, so a
//! crash never leaves half an index. Pinned entries are never pruned and never cleared by
//! "clear history".

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
    pub mime: String,
    /// Unix seconds of the latest copy of this content.
    pub created: u64,
    pub pinned: bool,
    /// The window's class when it was copied, if known.
    pub app: Option<String>,
    pub bytes: u64,
    /// A single line of text for lists; empty for images.
    pub preview: String,
    pub hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Unpinned entries kept; the oldest go first.
    pub max_items: usize,
    /// Largest single entry stored; anything bigger is skipped.
    pub max_entry_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_items: 200,
            max_entry_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the history can't be read or written: {0}")]
    Io(#[from] io::Error),
    #[error("no entry {0}")]
    NotFound(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Added {
    New(u64),
    /// The same content was already there; it moved to the top.
    Bumped(u64),
    TooBig,
    Empty,
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    next_id: u64,
    entries: Vec<Entry>,
}

pub struct Store {
    dir: PathBuf,
    index: Index,
    limits: Limits,
}

/// FNV-1a: stable across runs and Rust versions, which `DefaultHasher` is not.
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// One line for a list: whitespace collapsed, cut at 200 characters.
pub fn preview_of(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars() {
        if c.is_whitespace() {
            gap = !out.is_empty();
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        out.push(c);
        if out.chars().count() >= 200 {
            break;
        }
    }
    out
}

fn private_dir(p: &Path) -> io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(p)
}

fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(&tmp, path)
}

impl Store {
    /// Open (or create) the history in `dir`. A broken index starts an empty history instead of
    /// failing: the clipboard must keep working, and the blobs are only copies.
    pub fn open(dir: &Path, limits: Limits) -> Result<Store, StoreError> {
        private_dir(&dir.join("blobs"))?;
        let index = fs::read(dir.join("index.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Index>(&b).ok())
            .unwrap_or_default();
        let mut store = Store {
            dir: dir.to_path_buf(),
            index,
            limits,
        };
        store.sweep_orphans();
        Ok(store)
    }

    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    fn blob(&self, id: u64) -> PathBuf {
        self.dir.join("blobs").join(id.to_string())
    }

    fn save(&self) -> Result<(), StoreError> {
        let json = serde_json::to_vec(&self.index).map_err(io::Error::other)?;
        write_private(&self.dir.join("index.json"), &json)?;
        Ok(())
    }

    fn sweep_orphans(&mut self) {
        let Ok(rd) = fs::read_dir(self.dir.join("blobs")) else {
            return;
        };
        for e in rd.flatten() {
            let keep = e
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u64>().ok())
                .is_some_and(|id| self.index.entries.iter().any(|x| x.id == id));
            if !keep {
                let _ = fs::remove_file(e.path());
            }
        }
        // An entry whose bytes are gone is no use either.
        let missing: Vec<u64> = self
            .index
            .entries
            .iter()
            .filter(|e| !self.blob(e.id).exists())
            .map(|e| e.id)
            .collect();
        if !missing.is_empty() {
            self.index.entries.retain(|e| !missing.contains(&e.id));
        }
    }

    pub fn add(
        &mut self,
        kind: Kind,
        mime: &str,
        data: &[u8],
        app: Option<&str>,
        now: u64,
    ) -> Result<Added, StoreError> {
        if data.is_empty() || (kind == Kind::Text && data.iter().all(u8::is_ascii_whitespace)) {
            return Ok(Added::Empty);
        }
        if data.len() as u64 > self.limits.max_entry_bytes {
            return Ok(Added::TooBig);
        }
        let hash = fnv1a(data);
        if let Some(pos) = self
            .index
            .entries
            .iter()
            .position(|e| e.hash == hash && e.kind == kind && e.bytes == data.len() as u64)
        {
            let mut e = self.index.entries.remove(pos);
            e.created = now;
            if app.is_some() {
                e.app = app.map(str::to_owned);
            }
            let id = e.id;
            self.index.entries.insert(0, e);
            self.save()?;
            return Ok(Added::Bumped(id));
        }
        let id = self.index.next_id + 1;
        self.index.next_id = id;
        write_private(&self.blob(id), data)?;
        let preview = match kind {
            Kind::Text => preview_of(&String::from_utf8_lossy(data)),
            Kind::Image => String::new(),
        };
        self.index.entries.insert(
            0,
            Entry {
                id,
                kind,
                mime: mime.to_owned(),
                created: now,
                pinned: false,
                app: app.map(str::to_owned),
                bytes: data.len() as u64,
                preview,
                hash,
            },
        );
        self.prune();
        self.save()?;
        Ok(Added::New(id))
    }

    fn prune(&mut self) {
        let mut seen = 0;
        let mut drop = Vec::new();
        for e in &self.index.entries {
            if e.pinned {
                continue;
            }
            seen += 1;
            if seen > self.limits.max_items {
                drop.push(e.id);
            }
        }
        self.remove_ids(&drop);
    }

    fn remove_ids(&mut self, ids: &[u64]) {
        for id in ids {
            let _ = fs::remove_file(self.blob(*id));
        }
        self.index.entries.retain(|e| !ids.contains(&e.id));
    }

    /// Pinned entries first, then the rest, each newest first.
    pub fn list(&self, query: &str, kind: Option<Kind>, limit: usize) -> Vec<Entry> {
        let q = query.trim().to_lowercase();
        let hit = |e: &&Entry| {
            kind.is_none_or(|k| e.kind == k)
                && (q.is_empty() || e.preview.to_lowercase().contains(&q) || self.text_has(e, &q))
        };
        let pinned = self.index.entries.iter().filter(|e| e.pinned).filter(hit);
        let rest = self.index.entries.iter().filter(|e| !e.pinned).filter(hit);
        pinned.chain(rest).take(limit).cloned().collect()
    }

    /// The preview is cut; a search should still find the rest of a long entry.
    fn text_has(&self, e: &Entry, q: &str) -> bool {
        e.kind == Kind::Text
            && e.bytes > 200
            && fs::read(self.blob(e.id))
                .map(|b| String::from_utf8_lossy(&b).to_lowercase().contains(q))
                .unwrap_or(false)
    }

    pub fn entry(&self, id: u64) -> Result<&Entry, StoreError> {
        self.index
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or(StoreError::NotFound(id))
    }

    pub fn data(&self, id: u64) -> Result<Vec<u8>, StoreError> {
        self.entry(id)?;
        Ok(fs::read(self.blob(id))?)
    }

    pub fn blob_path(&self, id: u64) -> Result<PathBuf, StoreError> {
        self.entry(id)?;
        Ok(self.blob(id))
    }

    pub fn set_pinned(&mut self, id: u64, pinned: bool) -> Result<(), StoreError> {
        self.entry(id)?;
        for e in &mut self.index.entries {
            if e.id == id {
                e.pinned = pinned;
            }
        }
        if !pinned {
            self.prune();
        }
        self.save()
    }

    pub fn delete(&mut self, id: u64) -> Result<(), StoreError> {
        self.entry(id)?;
        self.remove_ids(&[id]);
        self.save()
    }

    /// Remove everything, or everything except the pinned entries. Returns how many went.
    pub fn clear(&mut self, keep_pinned: bool) -> Result<usize, StoreError> {
        let ids: Vec<u64> = self
            .index
            .entries
            .iter()
            .filter(|e| !(keep_pinned && e.pinned))
            .map(|e| e.id)
            .collect();
        self.remove_ids(&ids);
        self.save()?;
        Ok(ids.len())
    }

    pub fn len(&self) -> usize {
        self.index.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.entries.is_empty()
    }
}
