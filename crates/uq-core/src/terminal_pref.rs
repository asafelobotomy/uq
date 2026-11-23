//! The terminal chosen on Settings › Default apps (`shell.terminal`), passed on to the other
//! apps that open a terminal, so one choice holds everywhere:
//!
//! - KDE apps (Dolphin's "Open Terminal Here", F4) read `[General] TerminalService` (a desktop
//!   id) and `TerminalApplication` (a command) from `~/.config/kdeglobals`.
//! - GTK apps, `gio launch` of `Terminal=true` entries and anything else using
//!   [xdg-terminal-exec](https://gitlab.freedesktop.org/terminal-wg/specifications) take the
//!   first entry of `~/.config/xdg-terminals.list`.
//!
//! A chosen terminal is written into both and replaces what was there. With no choice (uQ's
//! default: kyko, else kitty) uQ's default terminal is named only where nothing names one, as
//! before; and going back to the default takes out the terminal uQ wrote for the old choice,
//! leaving one the user set elsewhere alone.

use crate::atomic::atomic_write;
use crate::desktop_entry::{parse_exec, quote_command, read_entry};
use crate::kdeglobals;
use crate::paths::Paths;
use std::io;
use std::path::{Path, PathBuf};

/// `~/.config/xdg-terminals.list`, next to `kdeglobals`.
pub fn list_path(p: &Paths) -> PathBuf {
    kdeglobals::conf_path(p).with_file_name("xdg-terminals.list")
}

/// A terminal as KDE names it: its desktop id and the command its entry runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminal {
    pub id: String,
    pub command: String,
}

/// Find `id` in the applications folders (first folder wins) and read its command. `None` when
/// it is not installed, is hidden or has no usable `Exec`.
pub fn find(dirs: &[PathBuf], id: &str) -> Option<Terminal> {
    let text = dirs
        .iter()
        .find_map(|d| std::fs::read_to_string(d.join(id)).ok())?;
    let e = read_entry(&text);
    if e.hidden {
        return None;
    }
    let argv = parse_exec(e.exec.as_deref()?).ok()?;
    Some(Terminal {
        id: id.to_string(),
        command: quote_command(&argv),
    })
}

/// The entry id of an `xdg-terminals.list` line (`id`, or `id:action`), or `None` for a blank
/// line or a comment.
fn list_entry(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(
        t.split(':')
            .next()
            .unwrap_or(t)
            .trim_start_matches(['+', '-']),
    )
}

/// `existing` with `id` as its first entry (any other line naming it removed), or `None` when it
/// is first already.
pub fn list_with_first(existing: &str, id: &str) -> Option<String> {
    if existing.lines().find_map(list_entry) == Some(id) {
        return None;
    }
    let mut out = vec![id.to_string()];
    out.extend(
        existing
            .lines()
            .filter(|l| list_entry(l) != Some(id))
            .map(str::to_string),
    );
    let mut s = out.join("\n").trim_end().to_string();
    s.push('\n');
    Some(s)
}

/// `existing` without the lines naming `id`, or `None` when none does.
pub fn list_without(existing: &str, id: &str) -> Option<String> {
    if !existing.lines().any(|l| list_entry(l) == Some(id)) {
        return None;
    }
    let kept: Vec<&str> = existing
        .lines()
        .filter(|l| list_entry(l) != Some(id))
        .collect();
    let s = kept.join("\n");
    let s = s.trim_end();
    Some(if s.is_empty() {
        String::new()
    } else {
        format!("{s}\n")
    })
}

fn read(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Apply `edit` to the file at `path` and write it when it returns a change.
fn edit(path: &Path, edit: impl FnOnce(&str) -> Option<String>) -> io::Result<bool> {
    let existing = read(path)?;
    match edit(&existing) {
        Some(new) if new != existing => atomic_write(path, &new).map(|()| true),
        _ => Ok(false),
    }
}

/// Pass the terminal choice on to KDE and xdg-terminal-exec. `chosen` is `shell.terminal` now,
/// `previous` what it was before this change (the same as `chosen` when re-applying at login).
/// `Ok(true)` when a file changed.
pub fn apply(
    p: &Paths,
    dirs: &[PathBuf],
    chosen: Option<&str>,
    previous: Option<&str>,
) -> io::Result<bool> {
    let kde = kdeglobals::conf_path(p);
    let list = list_path(p);
    let mut changed = false;
    // A choice that is no longer installed falls back to the default, as the launcher does.
    match chosen.and_then(|id| find(dirs, id)) {
        Some(t) => {
            changed |= edit(&kde, |s| {
                kdeglobals::with_chosen_terminal(s, &t.command, &t.id)
            })?;
            changed |= edit(&list, |s| list_with_first(s, &t.id))?;
        }
        None => {
            if let Some(old) = previous.filter(|old| Some(*old) != chosen) {
                changed |= edit(&kde, |s| kdeglobals::without_chosen_terminal(s, old))?;
                changed |= edit(&list, |s| list_without(s, old))?;
            }
            changed |= kdeglobals::ensure_terminal(p)?;
            let default = kdeglobals::TERMINAL_APPS
                .into_iter()
                .map(|t| format!("{t}.desktop"))
                .find(|id| find(dirs, id).is_some());
            if let Some(id) = default {
                changed |= edit(&list, |s| {
                    if s.lines().any(|l| list_entry(l).is_some()) {
                        None
                    } else {
                        list_with_first(s, &id)
                    }
                })?;
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests;
