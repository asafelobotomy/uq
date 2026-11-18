//! `~/.config/uq/clipboard.toml`. Every key is optional; a missing or unreadable file means the
//! defaults. The Settings page and `uqctl clip config` write this file (never the daemon).

use crate::privacy::Privacy;
use crate::store::Limits;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Keep a history at all. Off, the daemon still runs but stores nothing.
    pub enabled: bool,
    /// Unpinned entries kept.
    pub max_items: usize,
    /// Largest image or text stored, in MiB.
    pub max_entry_mb: u64,
    pub privacy: Privacy,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            max_items: Limits::default().max_items,
            max_entry_mb: 16,
            privacy: Privacy::default(),
        }
    }
}

impl Config {
    pub fn limits(&self) -> Limits {
        Limits {
            max_items: self.max_items.clamp(1, 5000),
            max_entry_bytes: self.max_entry_mb.clamp(1, 256) * 1024 * 1024,
        }
    }

    pub fn load(path: &Path) -> Config {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("uq/clipboard.toml")
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("uq/clipboard")
}

pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("uq/clip.sock")
}
