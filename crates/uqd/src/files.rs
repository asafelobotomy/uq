//! uqd's file sync outside its own state (spec §6, §7.3): the session variables file, the
//! Chromium flag files, the launcher-file overrides and the hidden autostart copies.
//!
//! Spec deviation 7: apart from `environment.d/60-uq.conf`, which uqd owns by name, uqd only
//! writes or deletes files that carry uQ's marker. A file without it, or a symlink in its place,
//! is never changed. The marker is checked again immediately before every write lands and every
//! delete (F13), so a file the user puts there in between survives.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use uq_core::atomic::atomic_write;
use uq_core::autostart::scan::{read_capped, read_capped_if};
use uq_core::compat::Compat;
use uq_core::compat::browsers::{BROWSERS, FLAGS_MARKER, flags_text};
use uq_core::desktop_entry::{random_suffix, read_entry, valid_desktop_id};
use uq_core::env::{ENVIRONMENT_D_HEADER, environment_d};
use uq_core::paths::StartupPaths;
use uq_core::startup_explain::BrowserFile;

mod sync;
pub(crate) use sync::{
    marked_overrides, override_source, override_target, remove_overrides, sync_hidden_copies,
    sync_overrides,
};

/// Which marker makes a file uQ's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkerKind {
    /// A launcher-file override: `X-uQ-Override=true` in `[Desktop Entry]`.
    Override,
    /// A browser flags file: [`FLAGS_MARKER`] as the first line.
    Flags,
    /// A hidden autostart copy: `X-uQ-Autostart=true` in `[Desktop Entry]`.
    Autostart,
}

impl MarkerKind {
    fn marks(self, text: &str) -> bool {
        match self {
            MarkerKind::Override => read_entry(text).uq_override,
            MarkerKind::Flags => text.lines().next() == Some(FLAGS_MARKER),
            MarkerKind::Autostart => read_entry(text).uq_autostart,
        }
    }
}

/// How a marked write ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Landed {
    Written,
    /// Something that isn't uQ's is at the target (now, or by the time the write landed), so
    /// nothing was written.
    OwnFile,
}

/// The session variables file uqd owns (F40).
pub(crate) fn environment_d_path(paths: &StartupPaths) -> PathBuf {
    paths.config_home.join("environment.d/60-uq.conf")
}

/// Whether anything (a file, a symlink, a folder) is at `path`, without following symlinks.
pub(crate) fn present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Whether `path` is a regular file (never a symlink) that carries `kind`'s marker. Read with
/// the bounded, non-blocking read (F14), and only when the file opened is the one the no-follow
/// check saw (same device and inode), so a symlink swapped in between is never read through.
pub(crate) fn is_ours(path: &Path, kind: MarkerKind) -> bool {
    use std::os::unix::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_file() => {
            hook::fire_read(path);
            read_capped_if(path, |o| (o.dev(), o.ino()) == (m.dev(), m.ino()))
                .is_some_and(|t| kind.marks(&t))
        }
        _ => false,
    }
}

/// `dir/id`, only for a valid desktop id whose joined path stays directly in `dir` with the id
/// as its file name.
pub(crate) fn safe_join(dir: &Path, id: &str) -> Option<PathBuf> {
    if !valid_desktop_id(id) {
        return None;
    }
    let p = dir.join(id);
    (p.parent() == Some(dir) && p.file_name() == Some(OsStr::new(id))).then_some(p)
}

/// Tests reach between the first check and the write or delete through this hook (F13).
#[cfg(test)]
pub(crate) mod hook {
    use std::cell::RefCell;
    use std::path::Path;

    type Hook = Box<dyn FnOnce(&Path)>;
    thread_local! {
        static HOOK: RefCell<Option<Hook>> = RefCell::new(None);
        static READ_HOOK: RefCell<Option<Hook>> = RefCell::new(None);
    }

    /// Run `f` once, at the next point between a check and a write or delete.
    pub(crate) fn once(f: impl FnOnce(&Path) + 'static) {
        HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    }

    /// Run `f` once, in the next [`super::is_ours`], between its no-follow check and its read.
    pub(crate) fn on_read(f: impl FnOnce(&Path) + 'static) {
        READ_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    }

    pub(super) fn fire(path: &Path) {
        if let Some(f) = HOOK.with(|h| h.borrow_mut().take()) {
            f(path);
        }
    }

    pub(super) fn fire_read(path: &Path) {
        if let Some(f) = READ_HOOK.with(|h| h.borrow_mut().take()) {
            f(path);
        }
    }
}

#[cfg(not(test))]
mod hook {
    pub(super) fn fire(_: &std::path::Path) {}
    pub(super) fn fire_read(_: &std::path::Path) {}
}

/// Write `text` to `path` only if `path` is absent or uQ's (F13): a temp file in the same
/// folder, then, immediately before it lands, the target is checked again. Still uQ's: `rename`
/// over it. Still absent: `hard_link` into the name, which fails if something appeared. Else
/// nothing is written. A symlink at the target is never followed or replaced.
pub(crate) fn write_marked(path: &Path, text: &str, kind: MarkerKind) -> io::Result<Landed> {
    if present(path) && !is_ours(path, kind) {
        return Ok(Landed::OwnFile);
    }
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no file name"));
    };
    fs::create_dir_all(dir)?;
    let mut tmp_name = OsStr::new(".").to_os_string();
    tmp_name.push(name);
    tmp_name.push(format!(
        ".uq-{}-{:08x}",
        std::process::id(),
        random_suffix()
    ));
    let tmp = dir.join(tmp_name);
    let res = write_new(&tmp, text).and_then(|()| {
        hook::fire(path);
        land(&tmp, path, kind)
    });
    // After a rename the temp name is gone; after a hard link or a refusal it is removed here.
    let _ = fs::remove_file(&tmp);
    res
}

fn write_new(tmp: &Path, text: &str) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()
}

fn land(tmp: &Path, path: &Path, kind: MarkerKind) -> io::Result<Landed> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => match fs::hard_link(tmp, path) {
            Ok(()) => Ok(Landed::Written),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(Landed::OwnFile),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
        Ok(_) if is_ours(path, kind) => fs::rename(tmp, path).map(|()| Landed::Written),
        Ok(_) => Ok(Landed::OwnFile),
    }
}

/// Delete `path` only if it is uQ's, checked immediately before the delete. `Ok(false)` when
/// nothing was deleted.
pub(crate) fn remove_marked(path: &Path, kind: MarkerKind) -> io::Result<bool> {
    hook::fire(path);
    if !is_ours(path, kind) {
        return Ok(false);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Whether `path` holds exactly `text` and is uQ's: a rewrite would change nothing.
fn unchanged(path: &Path, text: &str, kind: MarkerKind) -> bool {
    is_ours(path, kind) && read_capped(path).as_deref() == Some(text)
}

/// Write `environment.d/60-uq.conf` (F18, F40): rewritten atomically whether or not it carries
/// the header, never created when there are no lines, and removed then only when it starts
/// with uQ's header.
pub(crate) fn write_environment_d(
    paths: &StartupPaths,
    lines: &[(String, String)],
) -> io::Result<()> {
    let path = environment_d_path(paths);
    if lines.is_empty() {
        let ours = fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file())
            && read_capped(&path).is_some_and(|t| t.starts_with(ENVIRONMENT_D_HEADER));
        return match ours.then(|| fs::remove_file(&path)) {
            Some(Err(e)) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let text = environment_d(lines);
    if read_capped(&path).as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    atomic_write(&path, &text)
}

/// Each browser's flags file (§6, F37): with the switch on, written for each installed browser;
/// with it off, uQ's files are deleted. A file of the user's own is never touched. Returns the
/// state of each installed browser's file, by name.
pub(crate) fn sync_flags(
    paths: &StartupPaths,
    compat: &Compat,
    installed: &dyn Fn(&str) -> bool,
) -> Vec<(String, BrowserFile)> {
    let on = compat.chromium_wayland;
    let text = flags_text();
    let mut out = Vec::new();
    for b in &BROWSERS {
        let path = paths.config_home.join(b.flags_file);
        let inst = b.desktop_ids.iter().any(|id| installed(id));
        let state = match (on, inst) {
            // An uninstalled browser's file is left as it is: harmless, and right again if the
            // browser comes back.
            (true, false) => continue,
            (true, true) if unchanged(&path, &text, MarkerKind::Flags) => BrowserFile::Ours,
            (true, true) => match write_marked(&path, &text, MarkerKind::Flags) {
                Ok(Landed::Written) => BrowserFile::Ours,
                Ok(Landed::OwnFile) => BrowserFile::OwnFile,
                Err(e) => BrowserFile::Error(format!("uQ couldn't write {}: {e}", b.flags_file)),
            },
            (false, _) => match remove_marked(&path, MarkerKind::Flags) {
                Err(e) => BrowserFile::Error(format!("uQ couldn't remove {}: {e}", b.flags_file)),
                Ok(_) if present(&path) => BrowserFile::OwnFile,
                Ok(_) => BrowserFile::None,
            },
        };
        if inst {
            out.push((b.name.to_string(), state));
        }
    }
    out
}

#[cfg(test)]
mod tests;
