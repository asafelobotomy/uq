//! Let Flatpak apps read uQ's theme: read-only access to the theme files, as a global override.
//!
//! A Flatpak app sees none of `~/.config` or `~/.local/state`, so GTK, libadwaita and KDE apps
//! installed from Flathub stayed on their stock look. Flatpak's own way to fix that is an
//! override; uqd keeps uQ's entries in the user's global one (`~/.local/share/flatpak/overrides/
//! global`, the file `flatpak override --user` writes). Only the read-only `filesystems` entries
//! uQ needs are added: anything else in the file, and any access the user granted or denied
//! themselves, is kept. Colour scheme (dark or light) reaches Flatpak and Electron apps through
//! the settings portal (xdg-desktop-portal-gtk reads the gsettings uqd sets).

use crate::atomic::atomic_write;
use crate::paths::Paths;
use std::path::{Path, PathBuf};

/// `~/.local/share/flatpak/overrides/global`, next to `~/.local/share/uq`.
pub fn global_override_path(p: &Paths) -> PathBuf {
    p.user_themes_dir
        .parent()
        .and_then(Path::parent)
        .expect("user themes dir is <data>/uq/themes")
        .join("flatpak/overrides/global")
}

/// `path` as Flatpak's `filesystems` names it: `~/rel` inside `home`, absolute otherwise.
fn fs_entry(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rel) => format!("~/{}:ro", rel.display()),
        None => format!("{}:ro", path.display()),
    }
}

/// The read-only `filesystems` entries uQ's theme needs: the GTK css (which imports the rendered
/// theme), kdeglobals, Kvantum's config and the rendered theme itself.
pub fn wanted_entries(p: &Paths, home: Option<&Path>) -> Vec<String> {
    let mut v: Vec<String> = ["gtk-3.0", "gtk-4.0", "Kvantum", "kdeglobals"]
        .iter()
        .map(|f| format!("xdg-config/{f}:ro"))
        .collect();
    v.push(fs_entry(&p.theme_dir, home));
    v
}

/// The path an entry grants, without its `:ro`/`:rw`/`:create` suffix or a leading `!`.
fn entry_path(e: &str) -> &str {
    let e = e.trim().trim_start_matches('!');
    e.rsplit_once(':')
        .filter(|(_, m)| matches!(*m, "ro" | "rw" | "create"))
        .map_or(e, |(p, _)| p)
}

/// `existing` with `wanted` in `[Context] filesystems=`, or `None` when nothing changes. An entry
/// already there for the same path (granted another way, or denied with `!`) is the user's
/// choice and is kept as it is.
pub fn with_entries(existing: &str, wanted: &[String]) -> Option<String> {
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let mut group = String::new();
    let mut ctx_at = None;
    let mut fs_at = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if t.starts_with('[') && t.ends_with(']') {
            group = t.to_string();
            if group == "[Context]" && ctx_at.is_none() {
                ctx_at = Some(i);
            }
        } else if group == "[Context]"
            && t.split_once('=').map(|(k, _)| k.trim()) == Some("filesystems")
        {
            fs_at = Some(i);
        }
    }
    let current: Vec<String> = fs_at
        .and_then(|i| lines[i].split_once('=').map(|(_, v)| v.to_string()))
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let mut all = current.clone();
    for w in wanted {
        if !all.iter().any(|c| entry_path(c) == entry_path(w)) {
            all.push(w.clone());
        }
    }
    if all == current {
        return None;
    }
    let line = format!("filesystems={};", all.join(";"));
    match (fs_at, ctx_at) {
        (Some(i), _) => lines[i] = line,
        (None, Some(i)) => lines.insert(i + 1, line),
        (None, None) => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Context]".into());
            lines.push(line);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Some(out)
}

/// Whether `flatpak` is on PATH.
fn flatpak_installed() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join("flatpak").is_file()))
}

/// Add uQ's entries to the global Flatpak override when Flatpak is installed. `Ok(true)` when
/// the file changed.
pub fn sync(p: &Paths) -> std::io::Result<bool> {
    if !flatpak_installed() {
        return Ok(false);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    sync_at(
        &global_override_path(p),
        &wanted_entries(p, home.as_deref()),
    )
}

fn sync_at(path: &Path, wanted: &[String]) -> std::io::Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    match with_entries(&existing, wanted) {
        Some(new) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            atomic_write(path, &new).map(|()| true)
        }
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wanted() -> Vec<String> {
        vec![
            "xdg-config/gtk-3.0:ro".into(),
            "~/.local/state/uq/theme:ro".into(),
        ]
    }

    #[test]
    fn creates_the_context_group() {
        assert_eq!(
            with_entries("", &wanted()).unwrap(),
            "[Context]\nfilesystems=xdg-config/gtk-3.0:ro;~/.local/state/uq/theme:ro;\n"
        );
    }

    #[test]
    fn adds_to_existing_entries_and_keeps_the_rest() {
        let old = "[Context]\nsockets=wayland;\nfilesystems=~/Games;\n\n[Environment]\nFOO=1\n";
        let new = with_entries(old, &wanted()).unwrap();
        assert_eq!(
            new,
            "[Context]\nsockets=wayland;\nfilesystems=~/Games;xdg-config/gtk-3.0:ro;~/.local/state/uq/theme:ro;\n\n[Environment]\nFOO=1\n"
        );
        assert_eq!(
            with_entries(&new, &wanted()),
            None,
            "second pass is a no-op"
        );
    }

    #[test]
    fn keeps_what_the_user_chose_for_the_same_path() {
        let old = "[Context]\nfilesystems=!xdg-config/gtk-3.0;~/.local/state/uq/theme:rw;\n";
        assert_eq!(with_entries(old, &wanted()), None);
    }

    #[test]
    fn theme_dir_is_named_from_home() {
        assert_eq!(
            fs_entry(
                Path::new("/home/a/.local/state/uq/theme"),
                Some(Path::new("/home/a"))
            ),
            "~/.local/state/uq/theme:ro"
        );
        assert_eq!(
            fs_entry(Path::new("/srv/uq/theme"), Some(Path::new("/home/a"))),
            "/srv/uq/theme:ro"
        );
    }

    #[test]
    fn sync_writes_the_file() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("flatpak/overrides/global");
        assert!(sync_at(&f, &wanted()).unwrap());
        assert!(!sync_at(&f, &wanted()).unwrap());
        assert!(
            std::fs::read_to_string(&f)
                .unwrap()
                .contains("xdg-config/gtk-3.0:ro")
        );
    }
}
