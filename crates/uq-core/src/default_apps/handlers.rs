//! The installed apps that open files or links, read from their launcher files.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::desktop_entry::{clean_display, read_entry, valid_desktop_id};

/// An installed app and the types it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mimes: Vec<String>,
}

impl Handler {
    pub fn opens(&self, mime: &str) -> bool {
        self.mimes.iter().any(|m| m == mime)
    }
}

/// The most types one app is read with; a file listing more is cut there.
pub const MAX_TYPES_PER_APP: usize = 1000;

/// The applications in `dirs` (the first directory that has an id decides it), with their
/// `MimeType`s, sorted by name. Hidden entries are skipped but still hide later ones.
pub fn scan_handlers(dirs: &[PathBuf]) -> Vec<Handler> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for path in files {
            let Some(id) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if !id.ends_with(".desktop") || !valid_desktop_id(&id) || !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let e = read_entry(&text);
            if e.hidden || e.kind.as_deref() != Some("Application") || e.mime_types.is_empty() {
                continue;
            }
            let name = clean_display(e.name.as_deref().unwrap_or(&id), 200)
                .trim()
                .to_string();
            let mut mimes = e.mime_types;
            mimes.truncate(MAX_TYPES_PER_APP);
            out.push(Handler {
                id,
                name,
                icon: clean_display(e.icon.as_deref().unwrap_or(""), 200),
                mimes,
            });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}
