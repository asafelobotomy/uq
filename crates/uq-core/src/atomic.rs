//! All-or-nothing writes: a file via temp + rename, a directory via staging + rename.

use crate::render::RenderedFiles;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Makes each temp file name in this process unique, so concurrent writers never share one.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

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

/// Create a new temp file next to `path`, named `<name>.tmp-<n>-<pid>`. It is opened with
/// `O_EXCL | O_NOFOLLOW`, so it is always a new regular file of ours, never something (or a
/// symlink) already there.
fn create_temp(path: &Path) -> io::Result<(PathBuf, File)> {
    let mut tries = 0;
    loop {
        let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = sibling(path, &format!("tmp-{n}"))?;
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp);
        match opened {
            Ok(f) => return Ok((tmp, f)),
            // Left over from an earlier process with our pid: take the next number.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && tries < 100 => tries += 1,
            Err(e) => return Err(e),
        }
    }
}

/// fsync a directory, so a rename in it survives a crash. Filesystems that can't sync a
/// directory say so with `EINVAL`; that is not a failed write.
fn sync_dir(dir: &Path) -> io::Result<()> {
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    match File::open(dir).and_then(|d| d.sync_all()) {
        Err(e) if e.raw_os_error() == Some(libc::EINVAL) => Ok(()),
        r => r,
    }
}

/// Write `contents` to `path` atomically: a new temp file in the same directory (unique to this
/// process and call), synced, renamed over `path`, then the directory synced.
///
/// When `path` is a symlink, the link itself is replaced by a regular file; the file it pointed
/// to is left alone (`rename(2)` never follows the last component). Callers that keep a config
/// file as a symlink into, say, a dotfiles repository lose the link on the first write.
pub fn atomic_write(path: &Path, contents: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new(""));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }
    let (tmp, mut file) = create_temp(path)?;
    let res = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&tmp, path)
        });
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
        return res;
    }
    sync_dir(parent)
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
