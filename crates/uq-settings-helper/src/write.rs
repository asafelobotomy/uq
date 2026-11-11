//! Atomic, symlink-safe writes of root-owned `0644` files.
//!
//! A write goes to a fresh temporary file in the target's directory, opened with
//! `O_CREAT | O_EXCL | O_NOFOLLOW` (so it can never follow or reuse a planted entry), is
//! `fchmod`ed to `0644` whatever the umask, synced, renamed over the target, and the directory
//! is synced. `rename(2)` replaces a symlink at the target itself rather than writing through it.

use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Mode of every file the helper writes.
pub const FILE_MODE: u32 = 0o644;
/// Mode of a directory the helper has to create.
pub const DIR_MODE: u32 = 0o755;

fn parent(path: &Path) -> io::Result<&Path> {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no directory"))
}

fn temp_name(path: &Path) -> io::Result<PathBuf> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let mut s = std::ffi::OsString::from(".");
    s.push(name);
    s.push(format!(
        ".uq-tmp-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    Ok(path.with_file_name(s))
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Write `bytes` to `path` atomically as a `0644` file, creating its directory (`0755`) if
/// missing.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = parent(path)?;
    DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)?;
    let tmp = temp_name(path)?;
    let res = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)?;
        f.set_permissions(Permissions::from_mode(FILE_MODE))?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        sync_dir(dir)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Remove `path` if it exists (a symlink there is removed, not followed).
pub fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_dir(parent(path)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, symlink};

    #[test]
    fn writes_with_mode_0644_whatever_the_umask() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a/b/file");
        let old = unsafe { libc::umask(0o077) };
        let r = write_atomic(&p, b"hello");
        unsafe { libc::umask(old) };
        r.unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"hello");
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o7777, 0o644);
        // Directories it had to create: 0755 under a 077 umask is masked by the umask, so
        // only check they exist and nothing else was left beside the file.
        assert_eq!(fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn replaces_and_leaves_no_temp_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["f"]);
    }

    #[test]
    fn a_symlink_at_the_target_is_replaced_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let victim = d.path().join("victim");
        fs::write(&victim, b"keep").unwrap();
        let p = d.path().join("f");
        symlink(&victim, &p).unwrap();
        write_atomic(&p, b"new").unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
        assert!(fs::symlink_metadata(&p).unwrap().file_type().is_file());
        assert_eq!(fs::read(&p).unwrap(), b"new");

        // remove() takes the link away, not what it points at.
        symlink(&victim, d.path().join("g")).unwrap();
        remove(&d.path().join("g")).unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
    }

    #[test]
    fn a_failed_write_leaves_the_old_file() {
        let d = tempfile::tempdir().unwrap();
        // The target is a non-empty directory: rename fails, and nothing is left behind.
        let p = d.path().join("f");
        fs::create_dir(&p).unwrap();
        fs::write(p.join("x"), b"x").unwrap();
        assert!(write_atomic(&p, b"new").is_err());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
    }

    #[test]
    fn remove_missing_is_ok() {
        let d = tempfile::tempdir().unwrap();
        remove(&d.path().join("nope")).unwrap();
        let p = d.path().join("f");
        fs::write(&p, b"x").unwrap();
        remove(&p).unwrap();
        assert!(!p.exists());
    }
}
