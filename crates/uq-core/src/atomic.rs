//! All-or-nothing writes: a file via temp + rename, a directory via staging + rename.

use crate::render::RenderedFiles;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn sibling(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no file name", path.display()),
        )
    })?;
    let mut s = name.to_os_string();
    s.push(format!(".{suffix}-{}", std::process::id()));
    Ok(path.with_file_name(s))
}

fn write_synced(path: &Path, contents: &str) -> io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(contents.as_bytes())?;
    f.sync_all()
}

fn remove_any(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Write `contents` to `path` atomically (temp file in the same directory, then rename).
pub fn atomic_write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = sibling(path, "tmp")?;
    let res = write_synced(&tmp, contents).and_then(|()| fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Replace directory `target` with exactly `files`. Readers see either the old or the new set.
pub fn atomic_replace_dir(target: &Path, files: &RenderedFiles) -> io::Result<()> {
    let staging = sibling(target, "staging")?;
    let old = sibling(target, "old")?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_any(&staging)?;
    let fill = || -> io::Result<()> {
        fs::create_dir(&staging)?;
        for (rel, body) in &files.0 {
            let path = staging.join(rel);
            if let Some(p) = path.parent() {
                fs::create_dir_all(p)?;
            }
            write_synced(&path, body)?;
        }
        Ok(())
    };
    if let Err(e) = fill() {
        let _ = remove_any(&staging);
        return Err(e);
    }
    let had_target = target.exists();
    if had_target {
        remove_any(&old)?;
        if let Err(e) = fs::rename(target, &old) {
            let _ = remove_any(&staging);
            return Err(e);
        }
    }
    if let Err(e) = fs::rename(&staging, target) {
        if had_target {
            let _ = fs::rename(&old, target);
        }
        let _ = remove_any(&staging);
        return Err(e);
    }
    if had_target {
        remove_any(&old)?;
    }
    Ok(())
}
