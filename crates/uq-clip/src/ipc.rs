//! The socket `uq-clipd` listens on (`$XDG_RUNTIME_DIR/uq/clip.sock`, owner only).
//!
//! One JSON object per line each way. A request is `{"cmd": "...", ...}`; the reply is one line,
//! `{"ok": true, ...}` or `{"ok": false, "error": "..."}`. The exception is `watch`: the
//! connection stays open and a line `{"event": "state", ...}` arrives now and after every change.

use crate::store::{Entry, Kind};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Entries, pinned first, then newest first.
    List {
        #[serde(default)]
        query: String,
        #[serde(default)]
        kind: Option<Kind>,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    /// The text of an entry (up to 1 MiB), or the file of an image.
    Get {
        id: u64,
    },
    /// Make the entry the clipboard again.
    Copy {
        id: u64,
    },
    Pin {
        id: u64,
        pinned: bool,
    },
    Delete {
        id: u64,
    },
    Clear {
        #[serde(default = "yes")]
        keep_pinned: bool,
    },
    /// Stop (or resume) recording. Not remembered across restarts.
    Pause {
        paused: bool,
    },
    Status,
    /// Read `clipboard.toml` again.
    Reload,
    /// Keep the connection and stream state changes.
    Watch,
}

fn default_limit() -> usize {
    50
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Status {
    pub running: bool,
    pub paused: bool,
    pub enabled: bool,
    pub count: usize,
    pub pinned: usize,
    /// "ext-data-control", "wlr-data-control" or "none" when the compositor can't be watched.
    pub protocol: String,
    /// The id of the newest entry, 0 when empty; changes whenever something is copied.
    pub latest: u64,
}

/// Send one request and read the one-line reply.
pub fn call(socket: &Path, req: &Request) -> Result<serde_json::Value, String> {
    let mut s = UnixStream::connect(socket).map_err(|e| {
        format!("uq-clipd isn't running ({e}). Start it with: systemctl --user start uq-clipd")
    })?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut line = serde_json::to_string(req).map_err(|e| e.to_string())?;
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(s)
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&reply).map_err(|e| e.to_string())?;
    if v["ok"] == true {
        Ok(v)
    } else {
        Err(v["error"].as_str().unwrap_or("failed").to_owned())
    }
}

pub fn entries_from(v: &serde_json::Value) -> Vec<Entry> {
    serde_json::from_value(v["entries"].clone()).unwrap_or_default()
}
