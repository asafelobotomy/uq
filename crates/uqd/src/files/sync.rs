//! The two syncs keyed by desktop id: launcher-file overrides (`elsewhere`, §6) and hidden
//! autostart copies (R15).

use super::{Landed, MarkerKind, is_ours, remove_marked, safe_join, unchanged, write_marked};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uq_core::autostart::Autostart;
use uq_core::autostart::scan::{Found, read_capped, wanted_copies};
use uq_core::compat::launch::override_text;
use uq_core::compat::{AppFix, Compat};
use uq_core::paths::StartupPaths;
use uq_core::startup_explain::OverrideState;

/// The most entries of the user's applications folder the startup scan looks at.
const MAX_LISTED: usize = 4096;
const DEV_SESSION: &str = "Not available in a test session";

/// Where `id`'s override goes: `$XDG_DATA_HOME/applications/<id>`.
pub(crate) fn override_target(paths: &StartupPaths, id: &str) -> Option<PathBuf> {
    safe_join(&paths.data_home.join("applications"), id)
}

/// `id`'s own launcher file: the first `applications/<id>` in `$XDG_DATA_DIRS` that reads as a
/// regular file. The user's own folder never counts.
pub(crate) fn override_source(paths: &StartupPaths, id: &str) -> Option<(PathBuf, String)> {
    paths.data_dirs.iter().find_map(|d| {
        let p = safe_join(&d.join("applications"), id)?;
        let text = read_capped(&p)?;
        Some((p, text))
    })
}

/// Bring one fix's override in line with the fix and its source.
fn sync_one(paths: &StartupPaths, fix: &AppFix) -> OverrideState {
    let Some(target) = override_target(paths, &fix.id) else {
        return OverrideState::Error("uQ can't use this app's name for a file.".into());
    };
    // What the file should say, or why there is none.
    let wanted = if !fix.elsewhere {
        Err(OverrideState::Off)
    } else {
        match override_source(paths, &fix.id) {
            None => Err(OverrideState::NoSource),
            Some((src, text)) => match override_text(&text, &src.to_string_lossy(), fix, &fix.id) {
                Ok(Some(t)) => Ok(t),
                Ok(None) => Err(OverrideState::Off),
                Err(e) => Err(OverrideState::Error(e.to_string())),
            },
        }
    };
    match wanted {
        Ok(text) if unchanged(&target, &text, MarkerKind::Override) => OverrideState::Written,
        Ok(text) => match write_marked(&target, &text, MarkerKind::Override) {
            Ok(Landed::Written) => OverrideState::Written,
            Ok(Landed::OwnFile) => OverrideState::OwnFile,
            Err(e) => {
                OverrideState::Error(format!("uQ couldn't write this app's launcher file: {e}"))
            }
        },
        // A stale override of uQ's goes; a file of the user's own stays.
        Err(state) => match remove_marked(&target, MarkerKind::Override) {
            Err(e) => OverrideState::Error(format!(
                "uQ couldn't remove its old launcher file for this app: {e}"
            )),
            Ok(_) => state,
        },
    }
}

/// Each fix's override: written for a fix with `elsewhere` on and a source, removed otherwise.
/// In a dev session nothing is written or deleted (F38), and each `elsewhere` fix says why.
pub(crate) fn sync_overrides(
    paths: &StartupPaths,
    compat: &Compat,
    dev: bool,
) -> BTreeMap<String, OverrideState> {
    compat
        .apps
        .iter()
        .map(|fix| {
            let state = match (dev, fix.elsewhere) {
                (true, true) => OverrideState::Error(DEV_SESSION.into()),
                (true, false) => OverrideState::Off,
                (false, _) => sync_one(paths, fix),
            };
            (fix.id.clone(), state)
        })
        .collect()
}

/// Remove uQ's overrides for `ids` (fixes that are gone). Returns the failures, by id. Nothing
/// is deleted in a dev session.
pub(crate) fn remove_overrides<'a>(
    paths: &StartupPaths,
    ids: impl IntoIterator<Item = &'a str>,
    dev: bool,
) -> Vec<(String, String)> {
    if dev {
        return Vec::new();
    }
    ids.into_iter()
        .filter_map(|id| {
            let target = override_target(paths, id)?;
            remove_marked(&target, MarkerKind::Override)
                .err()
                .map(|e| (id.to_string(), e.to_string()))
        })
        .collect()
}

/// The ids of uQ's overrides in the user's applications folder (the startup sync's check for
/// overrides whose fix went away while uqd was not running).
pub(crate) fn marked_overrides(paths: &StartupPaths) -> Vec<String> {
    let dir = paths.data_home.join("applications");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    rd.take(MAX_LISTED)
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".desktop"))
        .filter(|n| safe_join(&dir, n).is_some_and(|p| is_ours(&p, MarkerKind::Override)))
        .collect()
}

/// The hidden copies (R15): one for each system entry that is off in state with no file of
/// the user's own in its place; uQ's copies that aren't wanted any more are deleted. Returns a
/// plain message when a copy couldn't be written or removed.
pub(crate) fn sync_hidden_copies(
    paths: &StartupPaths,
    found: &[Found],
    state: &Autostart,
) -> Option<String> {
    let dir = paths.user_autostart();
    let (write, delete) = wanted_copies(found, state);
    let mut failed = None;
    for (id, text) in write {
        let Some(path) = safe_join(&dir, &id) else {
            continue;
        };
        if unchanged(&path, &text, MarkerKind::Autostart) {
            continue;
        }
        // A file of the user's own that appeared meanwhile decides for itself: not an error.
        if let Err(e) = write_marked(&path, &text, MarkerKind::Autostart) {
            failed.get_or_insert(format!(
                "uQ couldn't write {id} in your autostart folder: {e}"
            ));
        }
    }
    for path in delete {
        if path.parent() != Some(dir.as_path()) {
            continue;
        }
        if let Err(e) = remove_marked(&path, MarkerKind::Autostart) {
            failed.get_or_insert(format!(
                "uQ couldn't remove {} from your autostart folder: {e}",
                name_of(&path)
            ));
        }
    }
    failed
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
