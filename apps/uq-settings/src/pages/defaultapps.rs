//! The Default apps page (spec §2.6): which app opens web links, mail, folders, text, pictures,
//! videos, music and PDFs, kept in the user's `~/.config/mimeapps.list`, and the terminal, which
//! is uQ's `shell.terminal` in uqd. Only the keys being set are touched; every other line of
//! `mimeapps.list` stays, and the file is replaced atomically (through a symlink to its target).

pub(crate) mod mimeapps;

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::desktop_entry::{clean_display, read_entry, valid_desktop_id};

use super::runner::{PageLogic, parse_json, patch_section};
use super::sysbus::{self, failure};
use crate::desktop_entries;

/// One kind of file or link: its id, its label, and the types it covers (the first decides
/// which apps are offered).
pub struct Kind {
    pub id: &'static str,
    pub label: &'static str,
    pub mimes: &'static [&'static str],
}

pub const KINDS: [Kind; 8] = [
    Kind {
        id: "browser",
        label: "Web browser",
        mimes: &[
            "x-scheme-handler/http",
            "x-scheme-handler/https",
            "text/html",
            "application/xhtml+xml",
        ],
    },
    Kind {
        id: "mail",
        label: "Email",
        mimes: &["x-scheme-handler/mailto"],
    },
    Kind {
        id: "files",
        label: "Files",
        mimes: &["inode/directory"],
    },
    Kind {
        id: "text",
        label: "Text editor",
        mimes: &["text/plain"],
    },
    Kind {
        id: "images",
        label: "Pictures",
        mimes: &[
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/tiff",
            "image/svg+xml",
        ],
    },
    Kind {
        id: "video",
        label: "Videos",
        mimes: &[
            "video/mp4",
            "video/x-matroska",
            "video/webm",
            "video/mpeg",
            "video/quicktime",
            "video/x-msvideo",
        ],
    },
    Kind {
        id: "music",
        label: "Music",
        mimes: &[
            "audio/mpeg",
            "audio/flac",
            "audio/ogg",
            "audio/x-vorbis+ogg",
            "audio/x-wav",
            "audio/mp4",
            "audio/aac",
        ],
    },
    Kind {
        id: "pdf",
        label: "PDF documents",
        mimes: &["application/pdf"],
    },
];

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Kind `kind` (an id of [`KINDS`]) opens with desktop id `app`.
    SetDefault {
        kind: String,
        app: String,
    },
    /// The terminal for apps that run in one; `None` is uQ's default.
    SetTerminal {
        app: Option<String>,
    },
}

/// An installed app and the types it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mimes: Vec<String>,
}

/// The applications in `dirs` (the first directory that has an id decides it), with their
/// `MimeType`s. Hidden entries are skipped but still hide later ones.
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
            out.push(Handler {
                id,
                name,
                icon: clean_display(e.icon.as_deref().unwrap_or(""), 200),
                mimes: e.mime_types,
            });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub use uq_core::mimeapps::{read_capped, system_files, user_file};

pub struct DefaultAppsState {
    session: Option<zbus::Connection>,
    app_dirs: Vec<PathBuf>,
    file: PathBuf,
    system_files: Vec<PathBuf>,
    write: Mutex<()>,
}

impl DefaultAppsState {
    pub fn new(session: zbus::Connection) -> Self {
        Self::with(
            Some(session),
            desktop_entries::default_dirs(),
            user_file(),
            system_files(),
        )
    }

    pub fn with(
        session: Option<zbus::Connection>,
        app_dirs: Vec<PathBuf>,
        file: PathBuf,
        system_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            session,
            app_dirs,
            file,
            system_files,
            write: Mutex::new(()),
        }
    }

    async fn handlers(&self) -> Vec<Handler> {
        let dirs = self.app_dirs.clone();
        tokio::task::spawn_blocking(move || scan_handlers(&dirs))
            .await
            .unwrap_or_default()
    }

    async fn uqd(&self) -> zbus::Result<DesktopProxy<'static>> {
        let conn = self
            .session
            .as_ref()
            .ok_or_else(|| failure("uQ's settings service isn't running."))?;
        DesktopProxy::new(conn).await
    }

    /// The current default for `mime`: the user's choice, else the system's, with where it
    /// came from (`you` or `system`).
    fn current(&self, user: &str, mime: &str) -> (Option<String>, &'static str) {
        if let Some(id) = mimeapps::default_for(user, mime) {
            return (Some(id), "you");
        }
        for f in &self.system_files {
            if let Some(id) = mimeapps::default_for(&read_capped(f), mime) {
                return (Some(id), "system");
            }
        }
        (None, "")
    }

    /// Write `mimeapps.list` atomically; a symlinked file is written at its target.
    fn save(&self, text: &str) -> zbus::Result<()> {
        let target = match std::fs::symlink_metadata(&self.file) {
            Ok(m) if m.file_type().is_symlink() => {
                std::fs::canonicalize(&self.file).map_err(|e| {
                    failure(format!(
                        "The list of default apps couldn't be followed: {e}"
                    ))
                })?
            }
            _ => self.file.clone(),
        };
        uq_core::atomic::atomic_write(&target, text)
            .map_err(|e| failure(format!("The list of default apps couldn't be saved: {e}")))
    }

    async fn terminal(&self) -> Option<Value> {
        let proxy = self.uqd().await.ok()?;
        let section = parse_json(&sysbus::within(proxy.get_section("shell")).await.ok()?).ok()?;
        Some(section.get("terminal").cloned().unwrap_or(Value::Null))
    }
}

impl PageLogic for DefaultAppsState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetDefault { kind, app } => {
                let kind = KINDS
                    .iter()
                    .find(|k| k.id == kind)
                    .ok_or_else(|| failure("That kind of file isn't on this page."))?;
                let handlers = self.handlers().await;
                if !handlers
                    .iter()
                    .any(|h| h.id == app && h.mimes.iter().any(|m| m == kind.mimes[0]))
                {
                    return Err(failure(
                        "That app can't open these, or isn't installed any more.",
                    ));
                }
                // Never replace a file this page can't read whole.
                if std::fs::metadata(&self.file)
                    .is_ok_and(|m| !m.is_file() || m.len() > uq_core::mimeapps::MAX_FILE)
                {
                    return Err(failure(
                        "The list of default apps is too large to change here.",
                    ));
                }
                // Read just before writing, so a change made elsewhere meanwhile is kept.
                let text = read_capped(&self.file);
                self.save(&mimeapps::set_defaults(&text, kind.mimes, &app))
            }
            Op::SetTerminal { app } => {
                let proxy = self.uqd().await?;
                patch_section(&proxy, "shell", |o| {
                    o.insert("terminal".into(), json!(app));
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let handlers = self.handlers().await;
        let user = read_capped(&self.file);
        let kinds: Vec<Value> = KINDS
            .iter()
            .map(|k| {
                let (current, source) = self.current(&user, k.mimes[0]);
                let apps: Vec<Value> = handlers
                    .iter()
                    .filter(|h| h.mimes.iter().any(|m| m == k.mimes[0]))
                    .map(|h| json!({"id": h.id, "name": h.name, "icon": h.icon}))
                    .collect();
                json!({"id": k.id, "label": k.label, "current": current,
                    "source": source, "apps": apps})
            })
            .collect();
        let dirs = self.app_dirs.clone();
        let apps = tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .unwrap_or_default();
        let terminals: Vec<Value> = desktop_entries::terminals(&apps)
            .into_iter()
            .map(|a| json!({"id": a.id, "name": a.name, "icon": a.icon}))
            .collect();
        let terminal = self.terminal().await;
        Ok(json!({
            "kinds": kinds,
            "terminal": {
                "available": terminal.is_some(),
                "current": terminal.unwrap_or(Value::Null),
                "apps": terminals,
            },
        }))
    }
}

#[cfg(test)]
mod tests;
