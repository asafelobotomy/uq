//! Name uQ's terminal in `~/.config/kdeglobals`, so Dolphin's "Open Terminal Here" (F4) and other
//! KDE apps open kyko (or kitty, where kyko is not installed).
//!
//! KDE apps launch a terminal through KTerminalLauncherJob, which reads
//! `[General] TerminalApplication` from `kdeglobals` and falls back to Konsole. Outside Plasma
//! nothing writes that key, so on a uQ system without Konsole "Open Terminal Here" did nothing.
//! uqd adds the key whenever it applies a theme, but only when it is missing: a terminal the user
//! chose themselves (in KDE's System Settings, or by hand) is kept, and so is the rest of the file.

use crate::atomic::atomic_write;
use crate::paths::Paths;
use std::path::{Path, PathBuf};

/// The terminals KDE apps should open, in the order uQ's terminal shortcut tries them
/// (`keybinds::defaults::TERMINAL`): the first one installed is named.
pub const TERMINAL_APPS: [&str; 2] = ["kyko", "kitty"];

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

/// Add uQ's terminal to `kdeglobals` when it names none and kyko or kitty is installed.
/// `Ok(true)` when the file changed; a missing file is created.
pub fn ensure_terminal(p: &Paths) -> std::io::Result<bool> {
    match TERMINAL_APPS.into_iter().find(|t| on_path(t).is_some()) {
        Some(t) => ensure_terminal_at(&conf_path(p), t),
        None => Ok(false),
    }
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

/// Split an INI text into `(header, lines)` groups; the lines before any header have header "".
fn groups(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            out.push((t.to_string(), Vec::new()));
        } else if let Some(last) = out.last_mut() {
            last.1.push(line.to_string());
        }
    }
    out
}

/// The colour scheme name uQ's `kde/uq.colors` carries (`[General] ColorScheme=`).
const SCHEME: &str = "uQ";

/// `existing` with uQ's `[Colors:*]` groups from `scheme` (the rendered `kde/uq.colors`) and
/// `[General] ColorScheme=uQ`, or `None` when nothing should change. Kirigami and every other KDE
/// framework read their colours from here (outside Plasma that means Breeze Light otherwise).
/// A file that already has colours of its own (KDE's System Settings wrote them) is theirs: it
/// is only rewritten while it still says `ColorScheme=uQ`. Everything else in it is kept.
pub fn with_colors(existing: &str, scheme: &str) -> Option<String> {
    let ours = groups(scheme);
    if !ours.iter().any(|(h, _)| h.starts_with("[Colors:")) {
        return None;
    }
    let mut theirs = groups(existing);
    let has_colors = theirs.iter().any(|(h, _)| h.starts_with("[Colors:"));
    let is_uq = theirs.iter().any(|(h, l)| {
        h == "[General]"
            && l.iter().any(|l| {
                key_of(l) == Some("ColorScheme")
                    && l.split_once('=').is_some_and(|(_, v)| v.trim() == SCHEME)
            })
    });
    if has_colors && !is_uq {
        return None;
    }
    theirs.retain(|(h, _)| !h.starts_with("[Colors:"));
    // [General] ColorScheme=uQ, first in the group.
    match theirs.iter_mut().find(|(h, _)| h == "[General]") {
        Some((_, lines)) => {
            lines.retain(|l| key_of(l) != Some("ColorScheme"));
            lines.insert(0, format!("ColorScheme={SCHEME}"));
        }
        None => theirs.insert(
            1,
            ("[General]".into(), vec![format!("ColorScheme={SCHEME}")]),
        ),
    }
    theirs.extend(ours.into_iter().filter(|(h, _)| h.starts_with("[Colors:")));
    let out = render_groups(theirs);
    (out != existing).then_some(out)
}

/// The text of `groups`: blank-line separated groups, no trailing blank lines inside a group.
fn render_groups(groups: Vec<(String, Vec<String>)>) -> String {
    let mut out = String::new();
    for (h, lines) in groups {
        let body: Vec<&str> = {
            let mut v: Vec<&str> = lines.iter().map(String::as_str).collect();
            while v.last().is_some_and(|l| l.trim().is_empty()) {
                v.pop();
            }
            v
        };
        if h.is_empty() && body.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        if !h.is_empty() {
            out.push_str(&h);
            out.push('\n');
        }
        for l in body {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// Whether the file still says `ColorScheme=uQ`, or has no colours of its own at all.
fn is_uq_scheme(theirs: &[(String, Vec<String>)]) -> bool {
    let has_colors = theirs.iter().any(|(h, _)| h.starts_with("[Colors:"));
    let is_uq = theirs.iter().any(|(h, l)| {
        h == "[General]"
            && l.iter().any(|l| {
                key_of(l) == Some("ColorScheme")
                    && l.split_once('=').is_some_and(|(_, v)| v.trim() == SCHEME)
            })
    });
    is_uq || !has_colors
}

/// What KDE apps read from `kdeglobals` besides colours: the icon theme, the widget style and
/// the fonts. With the `kde` platform theme (plasma-integration) this file is how Dolphin and
/// every other KDE app learn uQ's look.
pub struct Look<'a> {
    pub icons: &'a str,
    pub ui_font: &'a str,
    pub mono_font: &'a str,
    pub size: u8,
    /// `kvantum` when Kvantum is installed, otherwise `Fusion`.
    pub style: &'a str,
}

/// KDE's font string: `family,pointSize,pixelSize,styleHint,weight,italic,underline,strikeout,
/// fixedPitch,...`.
fn kfont(family: &str, size: u8, fixed: bool) -> String {
    format!(
        "{family},{size},-1,5,400,0,0,0,{},0,0,0,0,0,0,1",
        u8::from(fixed)
    )
}

fn set_key_in(groups: &mut Vec<(String, Vec<String>)>, group: &str, key: &str, value: &str) {
    let line = format!("{key}={value}");
    match groups.iter_mut().find(|(h, _)| h == group) {
        Some((_, lines)) => match lines.iter().position(|l| key_of(l) == Some(key)) {
            Some(i) => lines[i] = line,
            None => {
                // Before the blank lines that separate this group from the next.
                let end = lines
                    .iter()
                    .rposition(|l| !l.trim().is_empty())
                    .map_or(0, |i| i + 1);
                lines.insert(end, line);
            }
        },
        None => groups.push((group.to_string(), vec![line])),
    }
}

/// `existing` with uQ's colours (see [`with_colors`]) and `look`: `[Icons] Theme`, `[KDE]
/// widgetStyle` and the fonts. `None` when nothing changes. Like the colours, the look is only
/// written while the file is uQ's (it still says `ColorScheme=uQ`, or it has no colours): a
/// file KDE's System Settings wrote is theirs.
pub fn with_look(existing: &str, scheme: &str, look: &Look) -> Option<String> {
    let base = with_colors(existing, scheme);
    let text = base.as_deref().unwrap_or(existing);
    let mut g = groups(text);
    if !is_uq_scheme(&g) {
        return base;
    }
    if crate::settings::valid_theme_name(look.icons) {
        set_key_in(&mut g, "[Icons]", "Theme", look.icons);
    }
    if look.style == "kvantum" || look.style == "Fusion" {
        set_key_in(&mut g, "[KDE]", "widgetStyle", look.style);
    }
    if crate::qt6ct::safe_family(look.ui_font) && crate::qt6ct::safe_family(look.mono_font) {
        let ui = kfont(look.ui_font, look.size, false);
        for key in ["font", "menuFont", "toolBarFont"] {
            set_key_in(&mut g, "[General]", key, &ui);
        }
        let small = kfont(look.ui_font, look.size.saturating_sub(2).max(8), false);
        set_key_in(&mut g, "[General]", "smallestReadableFont", &small);
        set_key_in(
            &mut g,
            "[General]",
            "fixed",
            &kfont(look.mono_font, look.size, true),
        );
    }
    // Colours last, as with_colors leaves them, so a second pass changes nothing.
    g.sort_by_key(|(h, _)| h.starts_with("[Colors:"));
    let out = render_groups(g);
    (out != existing).then_some(out)
}

/// Write uQ's colours and look into `~/.config/kdeglobals` (see [`with_look`]). `Ok(true)` when
/// the file changed; no rendered scheme yet is `Ok(false)`.
pub fn sync_look(p: &Paths, look: &Look) -> std::io::Result<bool> {
    let scheme = match std::fs::read_to_string(p.theme_dir.join("kde/uq.colors")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let path = conf_path(p);
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    match with_look(&existing, &scheme, look) {
        Some(new) => atomic_write(&path, &new).map(|()| true),
        None => Ok(false),
    }
}

/// Put uQ's colour scheme into `~/.config/kdeglobals` (see [`with_colors`]). `Ok(true)` when the
/// file changed; no rendered scheme yet is `Ok(false)`.
pub fn sync_colors(p: &Paths) -> std::io::Result<bool> {
    let scheme = match std::fs::read_to_string(p.theme_dir.join("kde/uq.colors")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let path = conf_path(p);
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    match with_colors(&existing, &scheme) {
        Some(new) => atomic_write(&path, &new).map(|()| true),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinds::defaults::TERMINAL;

    #[test]
    fn terminals_are_the_ones_the_shortcut_runs_in_its_order() {
        let [first, second] = TERMINAL_APPS;
        assert!(
            TERMINAL.ends_with(&format!(
                "'command -v {first} >/dev/null && exec {first}; exec {second}'"
            )),
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

    const SCHEME_TXT: &str = "[General]\nColorScheme=uQ\nName=uQ Gruvbox\n\n[Colors:Window]\nBackgroundNormal=40,40,40\n\n[Colors:View]\nBackgroundNormal=60,56,54\n\n[WM]\nactiveBackground=1,2,3\n";

    #[test]
    fn colours_go_into_an_empty_file_without_the_wm_group() {
        let new = with_colors("", SCHEME_TXT).unwrap();
        assert_eq!(
            new,
            "[General]\nColorScheme=uQ\n\n[Colors:Window]\nBackgroundNormal=40,40,40\n\n[Colors:View]\nBackgroundNormal=60,56,54\n"
        );
        assert_eq!(with_colors(&new, SCHEME_TXT), None, "second run: unchanged");
    }

    #[test]
    fn colours_keep_the_rest_and_the_terminal() {
        let old = "[General]\nTerminalApplication=kitty\n\n[Icons]\nTheme=Papirus-Dark\n";
        let new = with_colors(old, SCHEME_TXT).unwrap();
        assert!(
            new.starts_with("[General]\nColorScheme=uQ\nTerminalApplication=kitty\n"),
            "{new}"
        );
        assert!(new.contains("[Icons]\nTheme=Papirus-Dark\n"), "{new}");
        assert!(new.contains("[Colors:Window]"), "{new}");
    }

    #[test]
    fn colours_replace_an_older_uq_scheme() {
        let old = with_colors("", SCHEME_TXT).unwrap();
        let next = SCHEME_TXT.replace("40,40,40", "9,9,9");
        let new = with_colors(&old, &next).unwrap();
        assert!(new.contains("BackgroundNormal=9,9,9"), "{new}");
        assert!(!new.contains("40,40,40"), "{new}");
        assert_eq!(new.matches("[Colors:Window]").count(), 1);
    }

    #[test]
    fn colours_leave_a_users_own_scheme_alone() {
        let theirs =
            "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=1,1,1\n";
        assert_eq!(with_colors(theirs, SCHEME_TXT), None);
    }

    const SCHEME: &str = "[General]\nColorScheme=uQ\n\n[Colors:Window]\nBackgroundNormal=1,2,3\n";

    fn look(icons: &str) -> Look<'_> {
        Look {
            icons,
            ui_font: "Inter",
            mono_font: "JetBrains Mono",
            size: 11,
            style: "kvantum",
        }
    }

    #[test]
    fn look_sets_icons_style_and_fonts_and_keeps_the_rest() {
        let new = with_look(
            "[General]\nTerminalApplication=kitty\n",
            SCHEME,
            &look("Papirus-Dark"),
        )
        .unwrap();
        assert!(new.contains("TerminalApplication=kitty"), "{new}");
        assert!(new.contains("[Icons]\nTheme=Papirus-Dark"), "{new}");
        assert!(new.contains("[KDE]\nwidgetStyle=kvantum"), "{new}");
        assert!(
            new.contains("font=Inter,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"),
            "{new}"
        );
        assert!(
            new.contains("fixed=JetBrains Mono,11,-1,5,400,0,0,0,1,"),
            "{new}"
        );
        assert!(new.contains("[Colors:Window]"), "{new}");
        assert_eq!(
            with_look(&new, SCHEME, &look("Papirus-Dark")),
            None,
            "second pass is a no-op: {new:?}"
        );
    }

    #[test]
    fn look_changes_with_the_theme() {
        let a = with_look("", SCHEME, &look("Papirus-Dark")).unwrap();
        let b = with_look(&a, SCHEME, &look("Papirus-Light")).unwrap();
        assert!(
            b.contains("Theme=Papirus-Light") && !b.contains("Papirus-Dark"),
            "{b}"
        );
    }

    #[test]
    fn look_leaves_a_users_own_scheme_alone() {
        let theirs =
            "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=9,9,9\n";
        assert_eq!(with_look(theirs, SCHEME, &look("Papirus-Dark")), None);
    }
}
