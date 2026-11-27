//! The Default apps page (spec §2.6): which app opens each kind of file and link, every single
//! file type, and every link type, kept in the user's `~/.config/mimeapps.list`; and the
//! terminal, which is uQ's `shell.terminal` in uqd. Only the keys being set are touched; every
//! other line of `mimeapps.list` stays, and the file is replaced atomically (through a symlink
//! to its target). The desktop's own list (`~/.config/hyprland-mimeapps.list`) wins over
//! `mimeapps.list`, so a choice made here also takes the same keys out of it.

pub(crate) mod mimeapps;
mod snapshot;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::default_apps::{self as da, Handler};

use super::runner::{PageLogic, parse_json, patch_section};
use super::sysbus::{self, failure};
use crate::desktop_entries;

pub use da::KINDS;
pub use da::{desktop_files, scan_handlers};
pub use uq_core::mimeapps::{read_capped, system_files, user_file};

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Kind `kind` (an id of [`KINDS`]) opens with desktop id `app`.
    SetDefault {
        kind: String,
        app: String,
    },
    /// Kind `kind` goes back to the system's choice.
    ResetDefault {
        kind: String,
    },
    /// Every kind uQ has its own app for, and that app is installed, opens with it.
    UseNative,
    /// One file or link type opens with `app`.
    SetType {
        mime: String,
        app: String,
    },
    /// One file or link type goes back to the system's choice.
    ResetType {
        mime: String,
    },
    /// The terminal for apps that run in one; `None` is uQ's default.
    SetTerminal {
        app: Option<String>,
    },
}

pub struct DefaultAppsState {
    session: Option<zbus::Connection>,
    app_dirs: Vec<PathBuf>,
    file: PathBuf,
    overrides: Vec<PathBuf>,
    system_files: Vec<PathBuf>,
    mime_dirs: Vec<PathBuf>,
    write: Mutex<()>,
}

impl DefaultAppsState {
    pub fn new(session: zbus::Connection) -> Self {
        let file = user_file();
        let sp = uq_core::paths::StartupPaths::from_vars(|k| std::env::var(k).ok());
        let desktops = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let overrides = desktop_files(file.parent().unwrap_or(Path::new(".")), &desktops);
        Self::with(
            Some(session),
            desktop_entries::default_dirs(),
            file,
            system_files(),
        )
        .with_overrides(overrides)
        .with_mime_dirs(da::info::mime_dirs(&sp.data_home, &sp.data_dirs))
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
            overrides: Vec::new(),
            system_files,
            mime_dirs: Vec::new(),
            write: Mutex::new(()),
        }
    }

    pub fn with_overrides(mut self, files: Vec<PathBuf>) -> Self {
        self.overrides = files;
        self
    }

    pub fn with_mime_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.mime_dirs = dirs;
        self
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

    /// Write a list atomically; a symlinked file is written at its target.
    fn save(path: &Path, text: &str) -> zbus::Result<()> {
        let target = match std::fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(path).map_err(|e| {
                failure(format!(
                    "The list of default apps couldn't be followed: {e}"
                ))
            })?,
            _ => path.to_path_buf(),
        };
        uq_core::atomic::atomic_write(&target, text)
            .map_err(|e| failure(format!("The list of default apps couldn't be saved: {e}")))
    }

    /// Never replace a file this page can't read whole.
    fn check_size(path: &Path) -> zbus::Result<()> {
        if std::fs::metadata(path)
            .is_ok_and(|m| !m.is_file() || m.len() > uq_core::mimeapps::MAX_FILE)
        {
            return Err(failure(
                "The list of default apps is too large to change here.",
            ));
        }
        Ok(())
    }

    /// Take `mimes` out of the desktop's own lists, so the choice in `mimeapps.list` counts.
    fn clear_overrides(&self, mimes: &[String]) -> zbus::Result<()> {
        for f in &self.overrides {
            if Self::check_size(f).is_err() {
                continue;
            }
            if let Some(text) = mimeapps::remove_defaults(&read_capped(f), mimes) {
                Self::save(f, &text)?;
            }
        }
        Ok(())
    }

    /// Set `mimes` to `app` in the person's list (read just before writing, so a change made
    /// elsewhere meanwhile is kept), and out of the desktop's own lists.
    fn set(&self, mimes: &[String], app: &str) -> zbus::Result<()> {
        if mimes.is_empty() {
            return Ok(());
        }
        Self::check_size(&self.file)?;
        let text = read_capped(&self.file);
        Self::save(&self.file, &mimeapps::set_defaults(&text, mimes, app))?;
        self.clear_overrides(mimes)
    }

    /// Clear `mimes` from the person's lists, so the system's choice counts.
    fn reset(&self, mimes: &[String]) -> zbus::Result<()> {
        Self::check_size(&self.file)?;
        if let Some(text) = mimeapps::remove_defaults(&read_capped(&self.file), mimes) {
            Self::save(&self.file, &text)?;
        }
        self.clear_overrides(mimes)
    }

    async fn terminal(&self) -> Option<Value> {
        let proxy = self.uqd().await.ok()?;
        let section = parse_json(&sysbus::within(proxy.get_section("shell")).await.ok()?).ok()?;
        Some(section.get("terminal").cloned().unwrap_or(Value::Null))
    }

    fn kind(id: &str) -> zbus::Result<&'static da::Kind> {
        da::kind(id).ok_or_else(|| failure("That kind of file isn't on this page."))
    }

    fn valid_type(mime: &str) -> bool {
        mime.len() <= 255
            && mime.split_once('/').is_some_and(|(a, b)| {
                !a.is_empty() && !b.is_empty() && !mime.contains(['=', ';', '[', '\n', ' '])
            })
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
                let kind = Self::kind(&kind)?;
                let handlers = self.handlers().await;
                let Some(h) = handlers.iter().find(|h| h.id == app && kind.offers(h)) else {
                    return Err(failure(
                        "That app can't open these, or isn't installed any more.",
                    ));
                };
                self.set(&kind.mimes_for(h), &h.id)
            }
            Op::ResetDefault { kind } => {
                let kind = Self::kind(&kind)?;
                let handlers = self.handlers().await;
                self.reset(&kind.mimes_to_clear(&handlers))
            }
            Op::UseNative => {
                let handlers = self.handlers().await;
                let picks: Vec<_> = KINDS
                    .iter()
                    .filter_map(|k| k.native_in(&handlers).map(|h| (k.mimes_for(h), h)))
                    .collect();
                if picks.is_empty() {
                    return Err(failure("None of uQ's own apps is installed."));
                }
                for (mimes, h) in picks {
                    self.set(&mimes, &h.id)?;
                }
                Ok(())
            }
            Op::SetType { mime, app } => {
                if !Self::valid_type(&mime) {
                    return Err(failure("That isn't a file or link type."));
                }
                let handlers = self.handlers().await;
                if !handlers.iter().any(|h| h.id == app && h.opens(&mime)) {
                    return Err(failure(
                        "That app can't open these, or isn't installed any more.",
                    ));
                }
                self.set(&[mime], &app)
            }
            Op::ResetType { mime } => {
                if !Self::valid_type(&mime) {
                    return Err(failure("That isn't a file or link type."));
                }
                self.reset(&[mime])
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
        let mut out = snapshot::build(self, &handlers);
        let dirs = self.app_dirs.clone();
        let apps = tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .unwrap_or_default();
        let terminals: Vec<Value> = desktop_entries::terminals(&apps)
            .into_iter()
            .map(|a| {
                json!({"id": a.id, "name": a.name, "icon": a.icon,
                "native": a.id == da::KYKO})
            })
            .collect();
        let terminal = self.terminal().await;
        out["terminal"] = json!({
            "available": terminal.is_some(),
            "current": terminal.unwrap_or(Value::Null),
            "apps": terminals,
        });
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod types_tests;
