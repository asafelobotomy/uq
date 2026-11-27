//! "Restore last session" and the commands run at logout (Settings › Startup › Session).
//!
//! While restore is on, uqd notes every [`RECORD_EVERY`] which apps have windows open and on
//! which workspace ([`snapshot`]), in `last-session.json` beside `state.toml`. At the next login
//! the runner starts those apps with the startup apps (one that is a startup app already isn't
//! started twice) and moves each one's first windows to its workspace. Turning restore off
//! deletes the file. Logout commands run when the session ends (`SIGUSR1` from base.lua's
//! shutdown hook, then `SIGTERM`), each in its own scope, for at most [`LOGOUT_WAIT`] in all.

pub mod snapshot;

use crate::autostart::spawn::{Spawner, SystemSpawner};
use crate::dbus::SharedDesktop;
use crate::service::Desktop;
use snapshot::{Known, SessionApp};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;
use uq_core::autostart::scan::{Plan, Resolved, resolve};
use uq_core::autostart::{Added, Autostart};
use uq_core::desktop_entry::{normalise_id, random_suffix, unit_id, valid_desktop_id};

pub const FILE: &str = "last-session.json";
/// How often the open apps are noted.
pub const RECORD_EVERY: Duration = Duration::from_secs(15);
/// Nothing is noted for this long after login, so the session being restored isn't overwritten
/// by its first half.
pub const RECORD_AFTER: Duration = Duration::from_secs(60);
/// How long logout commands may take, together.
pub const LOGOUT_WAIT: Duration = Duration::from_secs(10);
/// Installed apps are read again at most this often.
const RESCAN: Duration = Duration::from_secs(300);

/// uQ's own windows, never reopened as apps.
const SKIP: [&str; 2] = ["uq-settings.desktop", "org.uq.Settings.desktop"];

impl Desktop {
    pub(crate) fn session_file(&self) -> PathBuf {
        self.paths
            .state_file
            .parent()
            .map(|d| d.join(FILE))
            .unwrap_or_default()
    }

    /// The runs that reopen `apps`, each with its workspace, as the runner takes them. An app
    /// that `taken` (the startup plan) starts already is left out.
    pub(crate) fn restore_plan(
        &self,
        apps: &[SessionApp],
        taken: &BTreeSet<String>,
    ) -> Vec<(Resolved, i64)> {
        let wanted: Vec<&SessionApp> = apps.iter().filter(|a| !taken.contains(&a.id)).collect();
        let state = Autostart {
            added: wanted
                .iter()
                .map(|a| Added::App {
                    id: format!("restore-{}", a.id),
                    name: a.id.clone(),
                    app: a.id.clone(),
                    enabled: true,
                    delay: None,
                    minimize: false,
                })
                .collect(),
            ..Autostart::default()
        };
        let paths = &self.paths.startup;
        let app_dirs = paths.app_dirs();
        let lookup = |id: &str| -> Option<(PathBuf, String)> {
            if !valid_desktop_id(id) {
                return None;
            }
            let file = normalise_id(id);
            let found = app_dirs.iter().find_map(|d| {
                let p = d.join(&file);
                uq_core::autostart::scan::read_capped(&p).map(|t| (p, t))
            })?;
            Some(self.follow_override(id, found))
        };
        let exists = crate::startup::program_exists;
        resolve(&[], &state, &self.state.compat, &lookup, &exists)
            .into_iter()
            .zip(wanted)
            .filter_map(|(mut r, a)| {
                if let Plan::Run { cwd, .. } = &mut r.plan {
                    let dir = cwd.take().filter(|p| p.is_dir());
                    *cwd = Some(dir.unwrap_or_else(|| paths.home.clone()));
                    return Some((r, a.workspace));
                }
                None
            })
            .collect()
    }
}

/// Note the open apps every [`RECORD_EVERY`] while restore is on; delete the note when it's off.
pub async fn record(desktop: SharedDesktop) {
    tokio::time::sleep(RECORD_AFTER).await;
    let mut known: Vec<Known> = Vec::new();
    let mut scanned: Option<tokio::time::Instant> = None;
    let mut last: Option<Vec<SessionApp>> = None;
    loop {
        let (on, file, comp, dirs) = {
            let d = desktop.lock().await;
            (
                d.state.autostart.restore,
                d.session_file(),
                d.comp.clone(),
                d.paths.startup.app_dirs(),
            )
        };
        if !on {
            if file.exists() {
                let _ = std::fs::remove_file(&file);
            }
            last = None;
        } else if let Ok(reply) = comp.clients().await {
            if scanned.is_none_or(|t| t.elapsed() >= RESCAN) {
                known = tokio::task::spawn_blocking(move || snapshot::known_apps(&dirs))
                    .await
                    .unwrap_or_default();
                scanned = Some(tokio::time::Instant::now());
            }
            let now = snapshot::from_clients(&reply, &known, &SKIP);
            if last.as_ref() != Some(&now) {
                let text = serde_json::to_string(&now).unwrap_or_else(|_| "[]".into());
                match uq_core::atomic::atomic_write(&file, &text) {
                    Ok(()) => last = Some(now),
                    Err(e) => {
                        tracing::warn!(
                            "[START-004] restore last session: writing {}: {e}",
                            file.display()
                        );
                    }
                }
            }
        }
        tokio::time::sleep(RECORD_EVERY).await;
    }
}

/// Note the open apps once, now (the session is ending), when restore is on.
pub async fn record_now(desktop: &SharedDesktop) {
    let (on, file, comp, dirs) = {
        let d = desktop.lock().await;
        (
            d.state.autostart.restore,
            d.session_file(),
            d.comp.clone(),
            d.paths.startup.app_dirs(),
        )
    };
    if !on {
        return;
    }
    let Ok(reply) = comp.clients().await else {
        return;
    };
    let known = tokio::task::spawn_blocking(move || snapshot::known_apps(&dirs))
        .await
        .unwrap_or_default();
    let now = snapshot::from_clients(&reply, &known, &SKIP);
    let text = serde_json::to_string(&now).unwrap_or_else(|_| "[]".into());
    if let Err(e) = uq_core::atomic::atomic_write(&file, &text) {
        tracing::warn!(
            "[START-004] restore last session: writing {}: {e}",
            file.display()
        );
    }
}

/// Run the enabled logout commands, each in its own scope, and wait for them, at most
/// [`LOGOUT_WAIT`] in all. Without systemd-run nothing runs.
pub async fn run_logout(desktop: &SharedDesktop) {
    let (commands, env, home) = {
        let d = desktop.lock().await;
        let commands: Vec<(String, Vec<String>)> = d
            .state
            .autostart
            .logout
            .iter()
            .filter_map(|a| match a {
                Added::Command {
                    name,
                    argv,
                    enabled: true,
                    ..
                } => Some((name.clone(), argv.clone())),
                _ => None,
            })
            .collect();
        (commands, d.child_env(), d.paths.startup.home.clone())
    };
    if commands.is_empty() {
        return;
    }
    let Some(spawner) = SystemSpawner::new() else {
        tracing::warn!("[START-001] systemd-run isn't on uqd's PATH; logout commands won't run");
        return;
    };
    let mut waits = Vec::new();
    for (name, argv) in commands {
        let sid = unit_id(&name);
        let unit = format!("app-uq-logout-{sid}-{:08x}", random_suffix());
        match spawner
            .spawn(&unit, &format!("uq-logout-{sid}"), &argv, &env, &home)
            .await
        {
            Ok(child) => waits.push(child.wait),
            Err(e) => tracing::warn!(
                "[START-002] running the logout command {name}: {}",
                e.kind()
            ),
        }
    }
    let deadline = tokio::time::Instant::now() + LOGOUT_WAIT;
    for wait in waits {
        if tokio::time::timeout_at(deadline, wait).await.is_err() {
            tracing::info!("logout commands still running after 10 s; logging out anyway");
            break;
        }
    }
}
