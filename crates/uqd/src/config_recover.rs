//! Hyprland reloads its config whenever a file it read changes. A package update replaces
//! `/usr/share/uq/hypr/base.lua`, and a reload that lands while it does fails its `require`:
//! Hyprland drops into emergency mode with none of uQ's binds ("a lua config resulted in no
//! binds"; seen on `uq-dev promote uq` and on a `bindleup` that updated uq). The reload either
//! finds no file ("module '<name>' not found") or a half-written one (Hyprland's
//! `require("<name>"): error loading module …`, or whatever error a cut-off file raises).
//!
//! After each reload uqd reads the config errors and collects the `.lua` files a `require`
//! failed on. It then looks at them every [`WATCH_EVERY`] for up to [`WATCH_FOR`] and reloads
//! once they are all there, have stopped changing, and no pacman transaction is running
//! (Hyprland reloads when a file goes, not when it comes back, and a big package takes seconds
//! to unpack). Each version of a file (its modification time and size) gets one recovery
//! reload at most, so a file that is really broken can't make a reload loop.

use crate::service::Desktop;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// How often to look at the files.
pub(crate) const WATCH_EVERY: Duration = Duration::from_secs(1);

/// How long to keep looking before giving up (the file really is gone, or it is the user's).
pub(crate) const WATCH_FOR: Duration = Duration::from_secs(120);

/// Module names worth looking at; anything longer is not one of uQ's paths.
const MAX_NAME: usize = 1024;

/// pacman's lock: there while a transaction runs.
const PACMAN_LOCK: &str = "/var/lib/pacman/db.lck";

/// One version of a file: where it is, when it was last written and how big it is.
type Stamp = (PathBuf, SystemTime, u64);

/// What uqd remembers between reloads.
#[derive(Debug)]
pub(crate) struct RecoverState {
    /// Whether a task is already waiting for the files to settle.
    pub watching: Arc<AtomicBool>,
    /// File versions that already had their recovery reload.
    pub tried: Arc<Mutex<HashSet<Stamp>>>,
    /// While this file exists, a package transaction is running: wait.
    pub busy: PathBuf,
}

impl Default for RecoverState {
    fn default() -> Self {
        RecoverState {
            watching: Arc::default(),
            tried: Arc::default(),
            busy: PathBuf::from(PACMAN_LOCK),
        }
    }
}

/// The names in `<open><name><close>`, in order.
fn quoted(errors: &[String], open: &str, close: &str) -> Vec<String> {
    let mut out = Vec::new();
    for e in errors {
        let mut rest = e.as_str();
        while let Some(i) = rest.find(open) {
            rest = &rest[i + open.len()..];
            let Some(j) = rest.find(close) else { break };
            let name = &rest[..j];
            if !name.is_empty() && name.len() <= MAX_NAME && !name.contains(['\'', '"']) {
                out.push(name.to_string());
            }
            rest = &rest[j + close.len()..];
        }
    }
    out
}

/// The names in Lua's "module '<name>' not found" messages, in order.
pub(crate) fn missing_modules(errors: &[String]) -> Vec<String> {
    quoted(errors, "module '", "' not found")
}

/// Every module a `require` failed on: missing ones, and the ones Hyprland reports as
/// `require("<name>"): <error>` (a file that is there but didn't load), without repeats.
pub(crate) fn failed_modules(errors: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in missing_modules(errors)
        .into_iter()
        .chain(quoted(errors, "require(\"", "\"):"))
    {
        if !out.contains(&n) {
            out.push(n);
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

/// The files of the failed `require`s worth watching (uQ-style `.lua` paths).
pub(crate) fn failed_files(errors: &[String], hypr_dir: &Path) -> Vec<PathBuf> {
    failed_modules(errors)
        .iter()
        .filter_map(|n| module_path(n, hypr_dir))
        .collect()
}

/// Each file's version, or `None` while any of them is missing.
fn stamps(files: &[PathBuf]) -> Option<Vec<Stamp>> {
    files
        .iter()
        .map(|f| {
            let m = std::fs::metadata(f).ok().filter(|m| m.is_file())?;
            Some((f.clone(), m.modified().ok()?, m.len()))
        })
        .collect()
}

impl Desktop {
    /// Watch `files` until they are all there and unchanged for one look, with no package
    /// transaction running, then reload once; give up after [`WATCH_FOR`].
    fn watch_files(&self, files: Vec<PathBuf>) {
        if self.recover.watching.swap(true, Ordering::SeqCst) {
            return;
        }
        let comp = self.comp.clone();
        let flag = self.recover.watching.clone();
        let tried = self.recover.tried.clone();
        let busy = self.recover.busy.clone();
        tokio::spawn(async move {
            let looks = WATCH_FOR.as_secs() / WATCH_EVERY.as_secs().max(1);
            let mut last: Option<Vec<Stamp>> = None;
            for _ in 0..looks {
                tokio::time::sleep(WATCH_EVERY).await;
                if busy.exists() {
                    last = None;
                    continue;
                }
                let now = stamps(&files);
                if now.is_none() || now != last {
                    last = now;
                    continue;
                }
                let now = now.unwrap_or_default();
                let fresh = {
                    let mut t = tried.lock().unwrap_or_else(|e| e.into_inner());
                    let fresh = now.iter().any(|s| !t.contains(s));
                    t.extend(now);
                    fresh
                };
                if fresh {
                    tracing::warn!(
                        "[HYPR-005] the config files Hyprland failed to load are in place; reloading"
                    );
                    if let Err(e) = comp.reload().await {
                        tracing::warn!("[HYPR-003] reloading Hyprland: {e}");
                    }
                }
                break;
            }
            flag.store(false, Ordering::SeqCst);
        });
    }

    /// Hyprland reloaded its config: if a `require` of a `.lua` file failed (a file missing or
    /// half-written while a package replaced it), reload again once the files settle.
    pub async fn config_reloaded(&mut self) {
        let errors = match self.comp.config_errors().await {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("not checking the reload: {e}");
                return;
            }
        };
        let files = failed_files(&errors, &self.paths.hypr_dir);
        if files.is_empty() {
            return;
        }
        if let Some(now) = stamps(&files) {
            let t = self.recover.tried.lock().unwrap_or_else(|e| e.into_inner());
            if now.iter().all(|s| t.contains(s)) {
                tracing::warn!(
                    "[HYPR-005] Hyprland's config still fails on files already reloaded for; leaving it"
                );
                return;
            }
        }
        self.watch_files(files);
    }
}

#[cfg(test)]
mod tests;
