//! `DefaultApps`: the user's terminal, file manager and text editor, for the desktop menu's
//! Open entries. The terminal is uQ's `shell.terminal` (else kitty, the terminal a launch falls
//! back to); the others are the `inode/directory` and `text/plain` defaults in `mimeapps.list`,
//! the user's file before the system's. Each is `{id, name, icon}`, or null when none is set or
//! its desktop entry is missing, hidden, not an app, or its `TryExec` isn't installed. Nothing
//! here launches anything: the shell launches an id through `Launch`.

use super::desktop_entry::{Dirs, binary_exists, clean_display, read_entry};
use super::uqd::Link;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use uq_core::mimeapps;
use uq_core::settings::valid_pinned_id;

/// The terminal a launch falls back to (launch.rs `FALLBACK_TERMINAL`).
const FALLBACK_TERMINAL: &str = "kitty.desktop";
const MAX_NAME: usize = 64;

/// `{id, name, icon}` for a launchable desktop entry, else null.
fn app(dirs: &Dirs, id: &str) -> Value {
    if !valid_pinned_id(id) {
        return Value::Null;
    }
    let Some(path) = dirs.find(id) else {
        return Value::Null;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Value::Null;
    };
    let e = read_entry(&text);
    if e.hidden
        || e.exec.is_none()
        || e.kind.as_deref().is_some_and(|k| k != "Application")
        || e.try_exec.as_deref().is_some_and(|t| !binary_exists(t))
    {
        return Value::Null;
    }
    let fallback = id.strip_suffix(".desktop").unwrap_or(id);
    let name = clean_display(e.name.as_deref().unwrap_or(fallback), MAX_NAME);
    let name = if name.trim().is_empty() {
        clean_display(fallback, MAX_NAME)
    } else {
        name
    };
    json!({ "id": id, "name": name, "icon": e.icon.unwrap_or_default() })
}

/// The reply for these app folders, configured terminal and `mimeapps.list` files.
pub fn resolve(dirs: &Dirs, terminal: Option<&str>, user: &Path, system: &[PathBuf]) -> Value {
    let term = terminal
        .map(|t| app(dirs, t))
        .filter(|v| !v.is_null())
        .unwrap_or_else(|| app(dirs, FALLBACK_TERMINAL));
    let by_mime = |mime| {
        mimeapps::lookup(user, system, mime)
            .map(|id| app(dirs, &id))
            .unwrap_or(Value::Null)
    };
    json!({
        "terminal": term,
        "files": by_mime("inode/directory"),
        "text": by_mime("text/plain"),
    })
}

/// The bridge's `DefaultApps` call (no arguments).
pub fn default_apps(link: &Link, args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("DefaultApps takes no arguments".into());
    }
    let terminal = link
        .settings_rx()
        .borrow()
        .as_ref()
        .and_then(|s| s.shell.terminal.clone());
    Ok(resolve(
        &Dirs::from_env(),
        terminal.as_deref(),
        &mimeapps::user_file(),
        &mimeapps::system_files(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "[Desktop Entry]\nType=Application\n";

    fn setup(files: &[(&str, &str)], user: &str) -> (tempfile::TempDir, Dirs, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let apps = t.path().join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        for (name, text) in files {
            std::fs::write(apps.join(name), text).unwrap();
        }
        let list = t.path().join("mimeapps.list");
        std::fs::write(&list, user).unwrap();
        (t, Dirs::new(None, vec![apps]), list)
    }

    #[test]
    fn resolves_names_and_icons() {
        let (_t, dirs, list) = setup(
            &[
                (
                    "foot.desktop",
                    &format!("{APP}Name=Foot\nIcon=foot\nExec=foot\n"),
                ),
                (
                    "org.gnome.Nautilus.desktop",
                    &format!("{APP}Name=Files\nIcon=org.gnome.Nautilus\nExec=nautilus %U\n"),
                ),
                (
                    "org.kde.kate.desktop",
                    &format!("{APP}Name=Kate\nExec=kate %U\n"),
                ),
            ],
            "[Default Applications]\ninode/directory=org.gnome.Nautilus.desktop;\ntext/plain=org.kde.kate.desktop\n",
        );
        let v = resolve(&dirs, Some("foot.desktop"), &list, &[]);
        assert_eq!(
            v["terminal"],
            json!({"id": "foot.desktop", "name": "Foot", "icon": "foot"})
        );
        assert_eq!(v["files"]["name"], "Files");
        assert_eq!(v["files"]["icon"], "org.gnome.Nautilus");
        assert_eq!(
            v["text"],
            json!({"id": "org.kde.kate.desktop", "name": "Kate", "icon": ""})
        );
    }

    #[test]
    fn unset_or_broken_defaults_are_null() {
        let (_t, dirs, list) = setup(
            &[
                (
                    "hidden.desktop",
                    &format!("{APP}Name=H\nExec=h\nHidden=true\n"),
                ),
                (
                    "link.desktop",
                    "[Desktop Entry]\nType=Link\nName=L\nURL=x\n",
                ),
                (
                    "gone.desktop",
                    &format!("{APP}Name=G\nExec=g\nTryExec=/nonexistent/uq-test\n"),
                ),
            ],
            "[Default Applications]\ninode/directory=hidden.desktop\ntext/plain=link.desktop\n",
        );
        let v = resolve(&dirs, Some("gone.desktop"), &list, &[]);
        assert_eq!(v, json!({"terminal": null, "files": null, "text": null}));
        let v = resolve(&dirs, Some("../etc/passwd"), &list, &[]);
        assert!(v["terminal"].is_null());
    }

    #[test]
    fn terminal_falls_back_to_kitty() {
        let (_t, dirs, list) = setup(
            &[(
                "kitty.desktop",
                &format!("{APP}Name=kitty\nIcon=kitty\nExec=kitty\n"),
            )],
            "",
        );
        assert_eq!(
            resolve(&dirs, None, &list, &[])["terminal"]["id"],
            "kitty.desktop"
        );
        assert_eq!(
            resolve(&dirs, Some("missing.desktop"), &list, &[])["terminal"]["id"],
            "kitty.desktop"
        );
    }
}
