//! Reading the default app for a file type from `mimeapps.list`: the user's file, then the
//! system's. Shared by the Settings app's Default apps page (which also edits the user's file)
//! and the bridge (which the shell asks for the default terminal, file manager and text editor).

use std::path::{Path, PathBuf};

pub const GROUP: &str = "[Default Applications]";

/// The largest `mimeapps.list` read.
pub const MAX_FILE: u64 = 1024 * 1024;

/// The first desktop id `[Default Applications]` gives for `mime`.
pub fn default_for(text: &str, mime: &str) -> Option<String> {
    let mut inside = false;
    let mut found = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == GROUP;
            continue;
        }
        if !inside || t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=')
            && k.trim() == mime
            && found.is_none()
        {
            found = v
                .split(';')
                .map(str::trim)
                .find(|id| !id.is_empty())
                .map(str::to_string);
        }
    }
    found
}

/// `$XDG_CONFIG_HOME/mimeapps.list`, or `~/.config/mimeapps.list`.
pub fn user_file() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config"))
            .unwrap_or_default(),
    }
    .join("mimeapps.list")
}

/// The system-wide lists, read when the user's has no answer.
pub fn system_files() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::env::var("XDG_CONFIG_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/etc/xdg".into())
        .split(':')
        .map(|d| Path::new(d).join("mimeapps.list"))
        .collect();
    out.push(PathBuf::from("/usr/share/applications/mimeapps.list"));
    out
}

/// The file's text; empty when it is missing, not a regular file, or larger than [`MAX_FILE`].
pub fn read_capped(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() <= MAX_FILE => {
            std::fs::read_to_string(path).unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// The default for `mime`: the user's file first, then each system file in order.
pub fn lookup(user: &Path, system: &[PathBuf], mime: &str) -> Option<String> {
    std::iter::once(user)
        .chain(system.iter().map(PathBuf::as_path))
        .find_map(|f| default_for(&read_capped(f), mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_file_wins_over_system_files() {
        let d = tempfile::tempdir().unwrap();
        let user = d.path().join("user.list");
        let sys = d.path().join("sys.list");
        std::fs::write(
            &sys,
            "[Default Applications]\ntext/plain=gedit.desktop\ninode/directory=nautilus.desktop\n",
        )
        .unwrap();
        std::fs::write(&user, "[Default Applications]\ntext/plain=kate.desktop;\n").unwrap();
        let system = vec![d.path().join("missing.list"), sys];
        assert_eq!(
            lookup(&user, &system, "text/plain").as_deref(),
            Some("kate.desktop")
        );
        assert_eq!(
            lookup(&user, &system, "inode/directory").as_deref(),
            Some("nautilus.desktop")
        );
        assert_eq!(lookup(&user, &system, "image/png"), None);
    }

    #[test]
    fn a_missing_or_huge_file_reads_as_empty() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_capped(&d.path().join("nope")), "");
        assert_eq!(read_capped(d.path()), "");
        let big = d.path().join("big");
        std::fs::write(&big, vec![b'a'; MAX_FILE as usize + 1]).unwrap();
        assert_eq!(read_capped(&big), "");
    }
}
