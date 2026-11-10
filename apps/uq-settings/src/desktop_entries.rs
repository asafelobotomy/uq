//! The installed applications, read from `.desktop` files: the Shell page names pinned apps with
//! them and offers the terminal emulators among them. Names come from files other packages
//! install, so they are cut to a sane length with control characters removed, and the page shows
//! them only through `UqLabel`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use uq_core::settings::{valid_pinned_id, valid_terminal};

const MAX_NAME_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppEntry {
    /// The desktop file's name, `kitty.desktop`.
    pub id: String,
    pub name: String,
    pub icon: String,
    pub terminal_emulator: bool,
}

/// `$XDG_DATA_HOME/applications`, then `/usr/share/applications`.
pub fn default_dirs() -> Vec<PathBuf> {
    let data = match std::env::var_os("XDG_DATA_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".local/share"))
            .unwrap_or_default(),
    };
    vec![
        data.join("applications"),
        PathBuf::from("/usr/share/applications"),
    ]
}

fn clean_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

/// One entry from a file's text; `None` when it is hidden, not an application or has no name.
/// A file that is skipped here still hides the same id in later directories ([`scan`]).
fn parse(id: &str, text: &str) -> Option<AppEntry> {
    let (mut inside, mut name, mut icon) = (false, None, String::new());
    let (mut hidden, mut terminal_emulator, mut application) = (false, false, true);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "Name" => name = Some(clean_name(v)),
            "Icon" => icon = clean_name(v),
            "Type" => application = v == "Application",
            "Hidden" | "NoDisplay" => hidden |= v == "true",
            "Categories" => terminal_emulator = v.split(';').any(|c| c == "TerminalEmulator"),
            _ => {}
        }
    }
    if hidden || !application {
        return None;
    }
    let name = name.filter(|n| !n.is_empty())?;
    Some(AppEntry {
        id: id.to_string(),
        name,
        icon,
        terminal_emulator,
    })
}

/// The applications in `dirs`. The first directory that has an id decides it (even when its
/// file is hidden, as in the XDG spec); the result is sorted by name.
pub fn scan(dirs: &[PathBuf]) -> Vec<AppEntry> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for path in files {
            let Some(id) = file_id(&path) else { continue };
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path)
                && let Some(entry) = parse(&id, &text)
            {
                out.push(entry);
            }
        }
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// The terminal emulators the page may offer: only ids uqd would save as `shell.terminal`.
pub fn terminals(apps: &[AppEntry]) -> Vec<&AppEntry> {
    apps.iter()
        .filter(|a| a.terminal_emulator && valid_terminal(&a.id))
        .collect()
}

fn file_id(path: &Path) -> Option<String> {
    let id = path.file_name()?.to_str()?;
    (id.ends_with(".desktop") && valid_pinned_id(id)).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, id: &str, text: &str) {
        std::fs::write(dir.join(id), text).unwrap();
    }

    #[test]
    fn scan_reads_names_and_terminal_category() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "kitty.desktop",
            "[Desktop Entry]\nType=Application\nName=Kitty\nName[de]=Katze\nIcon=kitty\nCategories=System;TerminalEmulator;\n",
        );
        write(
            d.path(),
            "firefox.desktop",
            "[Desktop Entry]\nName=Firefox\nIcon=firefox\nCategories=Network;\n[Desktop Action new]\nName=New window\n",
        );
        write(d.path(), "notes.txt", "[Desktop Entry]\nName=No\n");
        let apps = scan(&[d.path().to_path_buf()]);
        assert_eq!(
            apps,
            [
                AppEntry {
                    id: "firefox.desktop".into(),
                    name: "Firefox".into(),
                    icon: "firefox".into(),
                    terminal_emulator: false
                },
                AppEntry {
                    id: "kitty.desktop".into(),
                    name: "Kitty".into(),
                    icon: "kitty".into(),
                    terminal_emulator: true
                },
            ]
        );
    }

    #[test]
    fn user_dir_wins_and_hidden_skipped() {
        let (user, system) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write(user.path(), "a.desktop", "[Desktop Entry]\nName=Mine\n");
        write(system.path(), "a.desktop", "[Desktop Entry]\nName=Theirs\n");
        write(
            system.path(),
            "b.desktop",
            "[Desktop Entry]\nName=B\nNoDisplay=true\n",
        );
        write(
            system.path(),
            "c.desktop",
            "[Desktop Entry]\nName=C\nHidden=true\n",
        );
        write(system.path(), "d.desktop", "[Desktop Entry]\nName=D\n");
        write(
            user.path(),
            "d.desktop",
            "[Desktop Entry]\nName=D\nHidden=true\n",
        );
        let apps = scan(&[user.path().to_path_buf(), system.path().to_path_buf()]);
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Mine"], "{apps:?}");
    }

    #[test]
    fn only_savable_terminals_are_offered() {
        let d = tempfile::tempdir().unwrap();
        let term = "[Desktop Entry]\nName=T\nCategories=TerminalEmulator;\n";
        for id in ["foot.desktop", "-x.desktop", "a..b.desktop", "_t.desktop"] {
            write(d.path(), id, term);
        }
        write(d.path(), "ed.desktop", "[Desktop Entry]\nName=Ed\n");
        let apps = scan(&[d.path().to_path_buf()]);
        assert_eq!(apps.len(), 5, "every app is still listed (for pins)");
        let ids: Vec<_> = terminals(&apps).iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["foot.desktop"]);
    }

    #[test]
    fn name_control_chars_stripped() {
        let d = tempfile::tempdir().unwrap();
        write(
            d.path(),
            "x.desktop",
            &format!(
                "[Desktop Entry]\nName=Ev\u{1b}[0mil\u{7}{}\n",
                "x".repeat(300)
            ),
        );
        let apps = scan(&[d.path().to_path_buf()]);
        assert_eq!(apps.len(), 1);
        let name = &apps[0].name;
        assert!(name.starts_with("Ev[0mil") && !name.chars().any(char::is_control));
        assert_eq!(name.chars().count(), MAX_NAME_CHARS);
    }
}
