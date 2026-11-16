//! Name uQ's terminal in `~/.config/kdeglobals`, so Dolphin's "Open Terminal Here" (F4) and other
//! KDE apps open kitty.
//!
//! KDE apps launch a terminal through KTerminalLauncherJob, which reads
//! `[General] TerminalApplication` from `kdeglobals` and falls back to Konsole. Outside Plasma
//! nothing writes that key, so on a uQ system without Konsole "Open Terminal Here" did nothing.
//! uqd adds the key whenever it applies a theme, but only when it is missing: a terminal the user
//! chose themselves (in KDE's System Settings, or by hand) is kept, and so is the rest of the file.

use crate::atomic::atomic_write;
use crate::paths::Paths;
use std::path::{Path, PathBuf};

/// The terminal KDE apps should open: the program uQ's terminal shortcut runs
/// (`keybinds::defaults::TERMINAL`).
pub const TERMINAL_APP: &str = "kitty";

const KEY: &str = "TerminalApplication";

/// `~/.config/kdeglobals`, next to `~/.config/hypr`.
pub fn conf_path(p: &Paths) -> PathBuf {
    p.hypr_dir
        .parent()
        .expect("hypr dir has a parent")
        .join("kdeglobals")
}

/// The key of a `key=value` line, without KConfig's flags and locale (`Key[$e]`, `Key[de]`).
fn key_of(line: &str) -> Option<&str> {
    let (k, _) = line.split_once('=')?;
    Some(k.split('[').next().unwrap_or(k).trim())
}

/// `existing` with `TerminalApplication=<terminal>` in its `[General]` group, or `None` when it
/// already names a terminal (whichever one) and nothing should change. An empty value counts as
/// missing: KDE treats it as unset too.
pub fn with_terminal(existing: &str, terminal: &str) -> Option<String> {
    let mut group = "";
    let mut general_at = None;
    for (i, line) in existing.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            group = t;
            if group == "[General]" && general_at.is_none() {
                general_at = Some(i);
            }
            continue;
        }
        if group == "[General]"
            && key_of(t) == Some(KEY)
            && t.split_once('=').is_some_and(|(_, v)| !v.trim().is_empty())
        {
            return None;
        }
    }
    let wanted = format!("{KEY}={terminal}");
    let mut out: Vec<&str> = Vec::new();
    let mut group = "";
    for (i, line) in existing.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            group = t;
        } else if group == "[General]" && key_of(t) == Some(KEY) {
            // An empty TerminalApplication= gives way to the line added after the header.
            continue;
        }
        out.push(line);
        if Some(i) == general_at {
            out.push(&wanted);
        }
    }
    let mut new = if general_at.is_some() {
        out.join("\n")
    } else {
        // KDE writes [General] first; put it there so a user reading the file finds it.
        let rest = existing.trim_start_matches('\n');
        if rest.is_empty() {
            format!("[General]\n{wanted}")
        } else {
            format!("[General]\n{wanted}\n\n{}", rest.trim_end_matches('\n'))
        }
    };
    new.push('\n');
    Some(new)
}

/// Where `bin` is on PATH.
fn on_path(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(bin))
            .find(|p| p.is_file())
    })
}

/// Add uQ's terminal to `kdeglobals` when it names none and kitty is installed. `Ok(true)` when
/// the file changed; a missing file is created.
pub fn ensure_terminal(p: &Paths) -> std::io::Result<bool> {
    if on_path(TERMINAL_APP).is_none() {
        return Ok(false);
    }
    ensure_terminal_at(&conf_path(p), TERMINAL_APP)
}

fn ensure_terminal_at(path: &Path, terminal: &str) -> std::io::Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    match with_terminal(&existing, terminal) {
        Some(new) => atomic_write(path, &new).map(|()| true),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::defaults::TERMINAL;

    #[test]
    fn terminal_is_the_one_the_shortcut_runs() {
        assert!(
            TERMINAL.ends_with(&format!(" -- {TERMINAL_APP}")),
            "{TERMINAL}"
        );
    }

    #[test]
    fn creates_the_group_in_an_empty_file() {
        assert_eq!(
            with_terminal("", "kitty").unwrap(),
            "[General]\nTerminalApplication=kitty\n"
        );
    }

    #[test]
    fn adds_the_key_to_an_existing_general_group() {
        let old = "[General]\nfixed=JetBrains Mono,10\n\n[Icons]\nTheme=Papirus-Dark\n";
        assert_eq!(
            with_terminal(old, "kitty").unwrap(),
            "[General]\nTerminalApplication=kitty\nfixed=JetBrains Mono,10\n\n[Icons]\nTheme=Papirus-Dark\n"
        );
    }

    #[test]
    fn puts_a_new_general_group_first_and_keeps_the_rest() {
        let old = "[KDE]\nSingleClick=false\n";
        assert_eq!(
            with_terminal(old, "kitty").unwrap(),
            "[General]\nTerminalApplication=kitty\n\n[KDE]\nSingleClick=false\n"
        );
    }

    #[test]
    fn keeps_a_terminal_the_user_chose() {
        for theirs in [
            "[General]\nTerminalApplication=konsole\n",
            "[General]\nTerminalApplication = alacritty\n",
            "[General]\nTerminalApplication[$e]=$HOME/bin/term\n",
            "[General]\nTerminalApplication=kitty\n",
        ] {
            assert_eq!(with_terminal(theirs, "kitty"), None, "{theirs:?}");
        }
    }

    #[test]
    fn fills_an_empty_value() {
        let old = "[General]\nTerminalApplication=\nfixed=Mono,10\n";
        assert_eq!(
            with_terminal(old, "kitty").unwrap(),
            "[General]\nTerminalApplication=kitty\nfixed=Mono,10\n"
        );
    }

    #[test]
    fn the_key_in_another_group_does_not_count() {
        let old = "[General]\nfixed=Mono,10\n\n[Other]\nTerminalApplication=konsole\n";
        let new = with_terminal(old, "kitty").unwrap();
        assert_eq!(
            new,
            "[General]\nTerminalApplication=kitty\nfixed=Mono,10\n\n[Other]\nTerminalApplication=konsole\n"
        );
        assert_eq!(with_terminal(&new, "kitty"), None, "second run: unchanged");
    }

    #[test]
    fn writes_once_and_then_leaves_the_file_alone() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("kdeglobals");
        assert!(ensure_terminal_at(&path, "kitty").unwrap(), "created");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[General]\nTerminalApplication=kitty\n"
        );
        assert!(!ensure_terminal_at(&path, "kitty").unwrap(), "unchanged");
        std::fs::write(&path, "[General]\nTerminalApplication=konsole\n").unwrap();
        assert!(!ensure_terminal_at(&path, "kitty").unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[General]\nTerminalApplication=konsole\n"
        );
    }

    #[test]
    fn conf_path_is_in_the_config_dir() {
        let p = Paths::under(Path::new("/h"));
        assert_eq!(
            conf_path(&p),
            p.hypr_dir.parent().unwrap().join("kdeglobals")
        );
    }
}
