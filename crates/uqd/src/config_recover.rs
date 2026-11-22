//! Hyprland reloads its config whenever a file it read changes. A package update replaces
//! `/usr/share/uq/hypr/base.lua` by removing and rewriting it, and a reload that lands in between
//! fails its `require`: Hyprland drops into emergency mode with none of uQ's binds ("a lua config
//! resulted in no binds"; seen on `uq-dev promote uq`). After each reload uqd reads the config
//! errors, and a `require` that failed for a file that is there again gets one more reload, at
//! most once every [`RETRY_GAP`] so a file that keeps vanishing can't make a reload loop.
//!
//! Hyprland reloads when the file goes, not when it comes back, and a package update can leave
//! the file missing for longer than one check (a big package takes seconds to unpack): when the
//! file is still missing at the check, uqd keeps looking for it for [`WATCH_FOR`] and reloads
//! once it is back.

use crate::service::Desktop;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The shortest time between two recovery reloads.
pub(crate) const RETRY_GAP: Duration = Duration::from_secs(10);

/// How often to look for a file that is still missing.
pub(crate) const WATCH_EVERY: Duration = Duration::from_secs(1);

/// How long to keep looking before giving up (the file really is gone, or it is the user's).
pub(crate) const WATCH_FOR: Duration = Duration::from_secs(120);

/// Module names worth looking at; anything longer is not one of uQ's paths.
const MAX_NAME: usize = 1024;

/// What uqd remembers between reloads.
#[derive(Debug, Default)]
pub(crate) struct RecoverState {
    /// When uqd last reloaded to recover.
    pub last: Option<Instant>,
    /// Whether a task is already waiting for a missing file to come back.
    pub watching: Arc<AtomicBool>,
}

/// The names in Lua's "module '<name>' not found" messages, in order.
pub(crate) fn missing_modules(errors: &[String]) -> Vec<String> {
    const OPEN: &str = "module '";
    const CLOSE: &str = "' not found";
    let mut out = Vec::new();
    for e in errors {
        let mut rest = e.as_str();
        while let Some(i) = rest.find(OPEN) {
            rest = &rest[i + OPEN.len()..];
            let Some(j) = rest.find(CLOSE) else { break };
            let name = &rest[..j];
            if !name.is_empty() && name.len() <= MAX_NAME && !name.contains('\'') {
                out.push(name.to_string());
            }
            rest = &rest[j + CLOSE.len()..];
        }
    }
    out
}

/// The file a `require` name stands for: an absolute path as it is, a relative one under the
/// Hyprland config folder. Only `.lua` paths (uQ's form); a dotted module name is `None`.
pub(crate) fn module_path(name: &str, hypr_dir: &Path) -> Option<PathBuf> {
    if !name.ends_with(".lua") {
        return None;
    }
    let p = Path::new(name);
    Some(if p.is_absolute() {
        p.to_path_buf()
    } else {
        hypr_dir.join(p)
    })
}

/// Whether a reload would now find every module the last one missed (and it missed some).
pub(crate) fn worth_reloading(errors: &[String], hypr_dir: &Path) -> bool {
    let names = missing_modules(errors);
    !names.is_empty()
        && names
            .iter()
            .all(|n| module_path(n, hypr_dir).is_some_and(|p| p.is_file()))
}

/// Whether the last reload failed only for uQ-style `.lua` files that are not there right now.
pub(crate) fn waiting_for_files(errors: &[String], hypr_dir: &Path) -> bool {
    let names = missing_modules(errors);
    !names.is_empty()
        && names.iter().all(|n| module_path(n, hypr_dir).is_some())
        && !worth_reloading(errors, hypr_dir)
}

impl Desktop {
    /// Look for the missing files every [`WATCH_EVERY`] and reload once they are all back; stops
    /// when the errors change (someone fixed it), after the reload, or after [`WATCH_FOR`].
    fn watch_for_files(&self) {
        if self.recover.watching.swap(true, Ordering::SeqCst) {
            return;
        }
        let comp = self.comp.clone();
        let dir = self.paths.hypr_dir.clone();
        let flag = self.recover.watching.clone();
        tokio::spawn(async move {
            let tries = WATCH_FOR.as_secs() / WATCH_EVERY.as_secs().max(1);
            for _ in 0..tries {
                tokio::time::sleep(WATCH_EVERY).await;
                let Ok(errors) = comp.config_errors().await else {
                    break;
                };
                if !waiting_for_files(&errors, &dir) && !worth_reloading(&errors, &dir) {
                    break;
                }
                if worth_reloading(&errors, &dir) {
                    tracing::warn!("[HYPR-005] a config file Hyprland missed is back; reloading");
                    if let Err(e) = comp.reload().await {
                        tracing::warn!("[HYPR-003] reloading Hyprland: {e}");
                    }
                    break;
                }
            }
            flag.store(false, Ordering::SeqCst);
        });
    }

    /// Hyprland reloaded its config: if it failed only because a file was missing for a moment,
    /// reload again.
    pub async fn config_reloaded(&mut self) {
        let errors = match self.comp.config_errors().await {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("not checking the reload: {e}");
                return;
            }
        };
        if !worth_reloading(&errors, &self.paths.hypr_dir) {
            if waiting_for_files(&errors, &self.paths.hypr_dir) {
                self.watch_for_files();
            }
            return;
        }
        if self.recover.last.is_some_and(|t| t.elapsed() < RETRY_GAP) {
            tracing::warn!(
                "[HYPR-005] Hyprland's config still misses a file that is there; not reloading again yet"
            );
            return;
        }
        self.recover.last = Some(Instant::now());
        tracing::warn!(
            "[HYPR-005] Hyprland's last reload missed a config file that is back now; reloading"
        );
        if let Err(e) = self.comp.reload().await {
            tracing::warn!("[HYPR-003] reloading Hyprland: {e}");
        }
    }
}

#[cfg(test)]
mod tests;
