//! Reading what callers pass: wallpaper fds, capped reads, monitor JSON and config files.

use super::*;

/// The passed fd must be a regular file within the size cap (`fstat`; directories, FIFOs,
/// sockets and devices are refused before anything is read).
pub fn check_wallpaper_fd(file: &File) -> Result<u64, HelperError> {
    let meta = file
        .metadata()
        .map_err(|e| HelperError::InvalidArgument(format!("can't stat the wallpaper: {e}")))?;
    if !meta.file_type().is_file() {
        return Err(HelperError::InvalidArgument(
            "the wallpaper must be a regular file".into(),
        ));
    }
    if meta.len() > MAX_WALLPAPER {
        return Err(HelperError::InvalidArgument(
            "the wallpaper is larger than 20 MiB".into(),
        ));
    }
    Ok(meta.len())
}

/// Read a whole file with `pread` from offset 0, refusing it once it passes `cap` bytes (it may
/// have grown since the `fstat`).
pub fn read_capped(file: &File, cap: u64) -> Result<Vec<u8>, HelperError> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut off = 0u64;
    loop {
        let n = match file.read_at(&mut buf, off) {
            Ok(0) => return Ok(out),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(HelperError::InvalidArgument(format!(
                    "can't read the wallpaper: {e}"
                )));
            }
        };
        if out.len() as u64 + n as u64 > cap {
            return Err(HelperError::InvalidArgument(
                "the wallpaper is larger than 20 MiB".into(),
            ));
        }
        out.extend_from_slice(&buf[..n]);
        off += n as u64;
    }
}

/// Parse and validate a `monitors` section given as JSON.
pub fn parse_monitors(json: &str) -> Result<Monitors, HelperError> {
    if json.len() > MAX_MONITORS_JSON {
        return Err(HelperError::InvalidArgument(
            "the monitor layout is too large".into(),
        ));
    }
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| HelperError::InvalidArgument(format!("monitor layout isn't JSON: {e}")))?;
    let parsed =
        Monitors::from_section(&v).map_err(|e| HelperError::InvalidArgument(e.to_string()))?;
    // The greeter file holds the layout only; names, identities and the rest never reach it.
    Ok(Monitors::default().with_layout(parsed.outputs))
}

/// Read one of the helper's own config files: a regular file, not through a symlink, capped.
pub(crate) fn read_config(path: &Path, cap: u64) -> Result<String, HelperError> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => HelperError::Failed(format!(
                "{} is missing; is the uQ greeter installed?",
                path.display()
            )),
            _ => HelperError::Failed(format!("couldn't open {}: {e}", path.display())),
        })?;
    let meta = f
        .metadata()
        .map_err(|e| HelperError::Failed(format!("{}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > cap {
        return Err(HelperError::Failed(format!(
            "{} isn't a regular file of at most {cap} bytes",
            path.display()
        )));
    }
    let mut s = String::new();
    f.take(cap + 1)
        .read_to_string(&mut s)
        .map_err(|e| HelperError::Failed(format!("couldn't read {}: {e}", path.display())))?;
    if s.len() as u64 > cap {
        return Err(HelperError::Failed(format!(
            "{} is larger than {cap} bytes",
            path.display()
        )));
    }
    Ok(s)
}
