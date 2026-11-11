//! The run record (R7, F15): `$XDG_RUNTIME_DIR/uq/autostart-run.json`, which says what this
//! login already started, so a restarted uqd starts nothing twice.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// One started entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Started {
    /// The scope's unit name, without `.scope`.
    pub unit: String,
    pub pid: u32,
    /// When it started, in Unix seconds.
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub session: String,
    /// The runner handled every entry of this session.
    pub ran: bool,
    pub started: BTreeMap<String, Started>,
}

/// The largest record read; anything bigger is treated as missing.
const MAX_BYTES: u64 = 1 << 20;

impl Record {
    /// The record at `path`; empty when it is missing, too big or unreadable.
    pub fn load(path: &Path) -> Record {
        let Ok(meta) = std::fs::metadata(path) else {
            return Record::default();
        };
        if !meta.is_file() || meta.len() > MAX_BYTES {
            return Record::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write it atomically; a failure is logged, never fatal.
    pub fn save(&self, path: &Path) {
        let text = serde_json::to_string(self).expect("the record serialises");
        if let Err(e) = uq_core::atomic::atomic_write(path, &text) {
            tracing::warn!("writing the autostart run record: {e}");
        }
    }
}

/// `[A-Za-z0-9_]{1,128}`: a session id safe to compare and store.
pub fn valid_session(s: &str) -> bool {
    (1..=128).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
