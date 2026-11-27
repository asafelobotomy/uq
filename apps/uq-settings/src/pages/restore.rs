//! The Restore page's logic: every uQ setting at once (reset to defaults, a backup file saved
//! and brought back, each after a restore point), and Bindle's recovery (Rebase, Factory
//! Restore, Roll back) and snapper snapshots, through the `bindle` command. Restore points
//! themselves are the History page's model, which the page shares.
//!
//! The state is `{bindle: {found, status, error}, snapshots: [...] | null, snapper: {...} | null,
//! needsPassword, snapshotsError, run: {state, kind, log, restart}, notice: {seq, text}}`.
//! Bindle is asked once when the page opens and again on Check again; snapshots and their
//! settings need the password unless snapper lets this user read them, so the page asks first.

pub mod bindle;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use uq_core::client::DesktopProxy;

use self::bindle::{NEEDS_PASSWORD, Run};
use super::runner::PageLogic;
use super::wallpaper::local_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Refresh,
    /// Ask Bindle again what this computer allows, and read the snapshots without asking.
    Probe,
    /// Read the snapshots and their settings, asking for the password if need be.
    LoadSnapshots,
    CreateSnapshot {
        description: String,
    },
    DeleteSnapshot {
        number: u32,
    },
    SetSnapper {
        key: String,
        value: String,
    },
    /// Start a recovery action (`bindle::recover_args`); its output follows on the page.
    Recover {
        kind: String,
        number: u32,
    },
    /// Restart the computer after a rollback or a prepared Factory Restore.
    Reboot,
    ResetAll,
    /// Save the settings to a dialog's `file://` URL.
    SaveBackup {
        url: String,
    },
    /// Make a backup file's settings the current ones.
    RestoreBackup {
        url: String,
    },
}

#[derive(Default)]
struct Shown {
    status: Option<Value>,
    status_error: Option<String>,
    snapshots: Option<Value>,
    snapper: Option<Value>,
    needs_password: bool,
    snapshots_error: Option<String>,
    notice_seq: u64,
    notice: String,
}

pub struct RestoreState {
    conn: zbus::Connection,
    /// `bindle`, or `None` when Bindle isn't installed.
    bindle: Option<PathBuf>,
    reboot: Vec<String>,
    write: Mutex<()>,
    probed: AtomicBool,
    shown: std::sync::Mutex<Shown>,
    pub run: Arc<Run>,
}

fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

impl RestoreState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with(
            conn,
            bindle::find(),
            vec!["systemctl".into(), "reboot".into()],
        )
    }

    /// With another `bindle` and restart command (tests).
    pub fn with(conn: zbus::Connection, bindle: Option<PathBuf>, reboot: Vec<String>) -> Self {
        Self {
            conn,
            bindle,
            reboot,
            write: Mutex::new(()),
            probed: AtomicBool::new(false),
            shown: Default::default(),
            run: Arc::new(Run::default()),
        }
    }

    fn notice(&self, text: &str) {
        let mut s = self.shown.lock().unwrap();
        s.notice_seq += 1;
        s.notice = text.to_string();
    }

    fn exe(&self) -> zbus::Result<&PathBuf> {
        self.bindle
            .as_ref()
            .ok_or_else(|| failure("Bindle isn't installed"))
    }

    async fn probe(&self) {
        self.probed.store(true, Ordering::SeqCst);
        let Some(exe) = &self.bindle else { return };
        let status = bindle::json(exe, &["recovery", "--status"]).await;
        let snapper = status.as_ref().is_ok_and(|s| s["snapper"] == true);
        {
            let mut s = self.shown.lock().unwrap();
            match status {
                Ok(v) => (s.status, s.status_error) = (Some(v), None),
                // Bindle before 0.1.47 has no --status.
                Err(o) => {
                    s.status = None;
                    s.status_error = Some(format!(
                        "This Bindle can't be asked from here ({}). Update Bindle.",
                        o.why()
                    ));
                }
            }
        }
        if snapper {
            self.load_snapshots(false).await;
        }
    }

    /// The snapshots and their settings; `ask`: through the password when snapper won't
    /// answer this user.
    async fn load_snapshots(&self, ask: bool) {
        let Some(exe) = &self.bindle else { return };
        let mut list = vec!["snapshot", "list", "--json"];
        let mut settings = vec!["snapshot", "settings", "--json"];
        if !ask {
            list.push("--no-ask");
            settings.push("--no-ask");
        }
        let snaps = bindle::json(exe, &list).await;
        let config = match &snaps {
            Ok(_) => bindle::json(exe, &settings).await.ok(),
            Err(_) => None,
        };
        let mut s = self.shown.lock().unwrap();
        s.needs_password = false;
        s.snapshots_error = None;
        match snaps {
            Ok(v) => (s.snapshots, s.snapper) = (Some(v), config),
            Err(o) if o.code == Some(NEEDS_PASSWORD) => s.needs_password = true,
            Err(o) => s.snapshots_error = Some(o.why()),
        }
    }

    /// Run a quick `bindle` change; its complaint is the error.
    async fn change(&self, args: &[&str]) -> zbus::Result<String> {
        let o = bindle::output(self.exe()?, args).await;
        if !o.ok {
            return Err(failure(o.why()));
        }
        Ok(o.stdout)
    }

    fn path(url: &str) -> zbus::Result<PathBuf> {
        local_path(url)
            .map(PathBuf::from)
            .ok_or_else(|| failure("choose a file on this computer"))
    }
}

impl PageLogic for RestoreState {
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
            Op::Refresh if !self.probed.load(Ordering::SeqCst) => self.probe().await,
            Op::Refresh => {}
            Op::Probe => self.probe().await,
            Op::LoadSnapshots => self.load_snapshots(true).await,
            Op::CreateSnapshot { description } => {
                let d = description.trim();
                let d = if d.is_empty() { "Saved in Settings" } else { d };
                self.change(&["snapshot", "create", d]).await?;
                self.load_snapshots(true).await;
                self.notice("Snapshot taken");
            }
            Op::DeleteSnapshot { number } => {
                self.change(&["snapshot", "delete", &number.to_string(), "--yes"])
                    .await?;
                self.load_snapshots(true).await;
                self.notice("Snapshot deleted");
            }
            Op::SetSnapper { key, value } => {
                self.change(&["snapshot", "set", &key, &value]).await?;
                self.load_snapshots(true).await;
            }
            Op::Recover { kind, number } => {
                let args = bindle::recover_args(&kind, number)
                    .ok_or_else(|| failure("unknown recovery action"))?;
                self.run.start(self.exe()?, args, &kind).map_err(failure)?;
            }
            Op::Reboot => {
                let (cmd, args) = self
                    .reboot
                    .split_first()
                    .ok_or_else(|| failure("no restart command"))?;
                tokio::process::Command::new(cmd)
                    .args(args)
                    .spawn()
                    .map_err(|e| failure(format!("couldn't restart: {e}")))?;
            }
            Op::ResetAll => {
                DesktopProxy::new(&self.conn).await?.reset_all().await?;
                self.notice("Every setting is back to uQ's defaults");
            }
            Op::SaveBackup { url } => {
                let path = Self::path(&url)?;
                let text = DesktopProxy::new(&self.conn)
                    .await?
                    .export_settings()
                    .await?;
                uq_core::atomic::atomic_write(&path, &text)
                    .map_err(|e| failure(format!("couldn't save the backup: {e}")))?;
                self.notice("Settings saved to the file");
            }
            Op::RestoreBackup { url } => {
                let path = Self::path(&url)?;
                let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if len > 1024 * 1024 {
                    return Err(failure("that file is too big to be a uQ settings backup"));
                }
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| failure(format!("couldn't read the file: {e}")))?;
                DesktopProxy::new(&self.conn)
                    .await?
                    .import_settings(&text)
                    .await?;
                self.notice("Settings brought back from the file");
            }
        }
        Ok(())
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let s = self.shown.lock().unwrap();
        Ok(json!({
            "bindle": {
                "found": self.bindle.is_some(),
                "status": s.status,
                "error": s.status_error,
            },
            "snapshots": s.snapshots,
            "snapper": s.snapper,
            "needsPassword": s.needs_password,
            "snapshotsError": s.snapshots_error,
            "run": self.run.get(),
            "notice": {"seq": s.notice_seq, "text": s.notice},
        }))
    }

    async fn watch(&self, changed: Arc<Notify>) {
        loop {
            self.run.changed.notified().await;
            changed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
