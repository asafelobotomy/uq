//! Keep the icon theme in a uQ-written `~/.config/qt6ct/qt6ct.conf` in step with the theme.
//!
//! The session runs Qt through qt6ct (`base.lua`), and qt6ct has no icon theme until its
//! `qt6ct.conf` names one; `uqctl setup --apps` writes that file once, without one. So Qt apps
//! (Dolphin, uQ Settings, Kvantum-styled apps) found no themed icons. uqd sets `icon_theme=`
//! there whenever it applies a theme, as it sets gsettings' `icon-theme` for GTK.
//!
//! Only a file that loads uQ's colour scheme is touched: one the user wrote themselves is theirs.

use crate::atomic::atomic_write;
use crate::paths::Paths;
use crate::settings::valid_theme_name;
use crate::theme::Theme;
use std::path::{Path, PathBuf};

/// `~/.config/qt6ct/qt6ct.conf`, next to `~/.config/hypr`.
pub fn conf_path(p: &Paths) -> PathBuf {
    p.hypr_dir
        .parent()
        .expect("hypr dir has a parent")
        .join("qt6ct/qt6ct.conf")
}

/// The marker line that says `existing` is uQ's own qt6ct.conf (it loads uQ's colour scheme).
fn is_ours(existing: &str, theme_dir: &Path) -> bool {
    let marker = format!(
        "color_scheme_path={}",
        theme_dir.join("qt6ct/colors/uq.conf").display()
    );
    existing.lines().any(|l| l.trim() == marker)
}

/// `existing` with `key=value` first in `[section]` (the section is added at the end when it is
/// missing), and any earlier line for that key in the section dropped.
fn set_key(existing: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let wanted = format!("{key}={value}");
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut done = false;
    for line in existing.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            current = t.to_string();
            out.push(line.to_string());
            if current == header && !done {
                out.push(wanted.clone());
                done = true;
            }
            continue;
        }
        // The line placed right after the header replaces any earlier one for the key.
        if current == header && t.split('=').next().map(str::trim) == Some(key) {
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(header);
        out.push(wanted);
    }
    let mut new = out.join("\n");
    new.push('\n');
    new
}

/// `existing` with `icon_theme=<icon>` in its `[Appearance]` section, or `None` when nothing
/// should change: the file doesn't load uQ's colour scheme from `theme_dir`, the name isn't a
/// valid theme name, or it already says so.
pub fn with_icon_theme(existing: &str, theme_dir: &Path, icon: &str) -> Option<String> {
    if !valid_theme_name(icon) || !is_ours(existing, theme_dir) {
        return None;
    }
    let new = set_key(existing, "Appearance", "icon_theme", icon);
    (new != existing).then_some(new)
}

/// A font family that is safe inside a qt6ct QFont string: no quotes, commas or line breaks.
fn safe_family(f: &str) -> bool {
    !f.trim().is_empty()
        && f.len() < 100
        && !f
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | ',' | '='))
}

/// qt6ct's QFont string: `family,pointSize,pixelSize,styleHint,weight,style,...` (Qt 6 layout).
fn qfont(family: &str, size: u8, fixed: bool) -> String {
    format!(
        "\"{family},{size},-1,{hint},400,0,0,0,{fixed},0,0,0,0,0,0,1\"",
        hint = if fixed { 1 } else { 5 },
        fixed = u8::from(fixed)
    )
}

/// `existing` as it should be for `theme`: icon theme, the theme's UI and monospace fonts, and a
/// `[uQ] theme=` line that changes with every theme, so qt6ct reloads its palette even when the
/// icons and fonts stay the same (it re-reads the file, not the colour scheme it names). `None`
/// when the file isn't uQ's (one the user wrote is theirs) or nothing would change.
pub fn with_look(existing: &str, theme_dir: &Path, theme: &Theme) -> Option<String> {
    if !is_ours(existing, theme_dir) {
        return None;
    }
    let mut new = existing.to_string();
    if valid_theme_name(&theme.icons.theme) {
        new = set_key(&new, "Appearance", "icon_theme", &theme.icons.theme);
    }
    if safe_family(&theme.fonts.ui) && safe_family(&theme.fonts.mono) {
        new = set_key(
            &new,
            "Fonts",
            "fixed",
            &qfont(&theme.fonts.mono, theme.fonts.size, true),
        );
        new = set_key(
            &new,
            "Fonts",
            "general",
            &qfont(&theme.fonts.ui, theme.fonts.size, false),
        );
    }
    new = set_key(
        &new,
        "uQ",
        "theme",
        &format!("{}-{}", theme.id, theme.variant.as_str()),
    );
    (new != existing).then_some(new)
}

/// Apply `theme` to `qt6ct.conf` if it is uQ's. `Ok(true)` when the file changed; a missing file
/// is `Ok(false)`.
pub fn sync(p: &Paths, theme: &Theme) -> std::io::Result<bool> {
    let path = conf_path(p);
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    match with_look(&existing, &p.theme_dir, theme) {
        Some(new) => atomic_write(&path, &new).map(|()| true),
        None => Ok(false),
    }
}

/// Whether `~/.config/qt6ct/qt6ct.conf` is uQ's (`uqctl setup --apps` wrote it): the opt-in the
/// other app-theme files follow.
pub fn is_opted_in(p: &Paths) -> bool {
    std::fs::read_to_string(conf_path(p)).is_ok_and(|s| is_ours(&s, &p.theme_dir))
}

/// Write the icon theme into `qt6ct.conf` if it is uQ's and says otherwise. `Ok(true)` when the
/// file changed; a missing file is `Ok(false)`.
pub fn sync_icon_theme(p: &Paths, icon: &str) -> std::io::Result<bool> {
    let path = conf_path(p);
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    match with_icon_theme(&existing, &p.theme_dir, icon) {
        Some(new) => atomic_write(&path, &new).map(|()| true),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "/h/.local/state/uq/theme";

    fn ours(extra: &str) -> String {
        format!(
            "[Appearance]\ncolor_scheme_path={DIR}/qt6ct/colors/uq.conf\ncustom_palette=true\nstyle=kvantum\n{extra}"
        )
    }

    #[test]
    fn adds_the_icon_theme_to_uqs_file() {
        let new = with_icon_theme(&ours(""), Path::new(DIR), "Papirus-Dark").unwrap();
        assert_eq!(
            new,
            format!(
                "[Appearance]\nicon_theme=Papirus-Dark\ncolor_scheme_path={DIR}/qt6ct/colors/uq.conf\ncustom_palette=true\nstyle=kvantum\n"
            )
        );
    }

    #[test]
    fn replaces_an_older_icon_theme_and_keeps_other_sections() {
        let old = ours("icon_theme=Papirus-Dark\n\n[Fonts]\ngeneral=\"Inter,11\"\n");
        let new = with_icon_theme(&old, Path::new(DIR), "Papirus-Light").unwrap();
        assert_eq!(new.matches("icon_theme=").count(), 1, "{new}");
        assert!(new.contains("icon_theme=Papirus-Light\n"), "{new}");
        assert!(new.ends_with("[Fonts]\ngeneral=\"Inter,11\"\n"), "{new}");
    }

    #[test]
    fn unchanged_when_it_already_matches() {
        let once = with_icon_theme(&ours(""), Path::new(DIR), "Papirus-Dark").unwrap();
        assert_eq!(with_icon_theme(&once, Path::new(DIR), "Papirus-Dark"), None);
    }

    #[test]
    fn leaves_a_users_own_file_alone() {
        let theirs = "[Appearance]\ncolor_scheme_path=/usr/share/qt6ct/colors/darker.conf\n";
        assert_eq!(with_icon_theme(theirs, Path::new(DIR), "Papirus"), None);
        // uQ's path only in a comment or another key doesn't count either.
        let comment = format!("[Appearance]\n# color_scheme_path={DIR}/qt6ct/colors/uq.conf\n");
        assert_eq!(with_icon_theme(&comment, Path::new(DIR), "Papirus"), None);
    }

    #[test]
    fn refuses_an_unsafe_name() {
        for bad in ["Evil\n[Fonts]", "", "a=b\nc"] {
            assert_eq!(
                with_icon_theme(&ours(""), Path::new(DIR), bad),
                None,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn sync_writes_only_uqs_file() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::under(d.path());
        assert!(
            !sync_icon_theme(&p, "Papirus-Dark").unwrap(),
            "no file: nothing to do"
        );
        let path = conf_path(&p);
        let body = format!(
            "[Appearance]\ncolor_scheme_path={}\n",
            p.theme_dir.join("qt6ct/colors/uq.conf").display()
        );
        atomic_write(&path, &body).unwrap();
        assert!(sync_icon_theme(&p, "Papirus-Dark").unwrap());
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("icon_theme=Papirus-Dark")
        );
        assert!(
            !sync_icon_theme(&p, "Papirus-Dark").unwrap(),
            "second sync: unchanged"
        );
    }

    fn gruvbox() -> Theme {
        Theme::from_toml(
            crate::theme::tests::FIXTURE,
            "gruvbox",
            crate::theme::Variant::Dark,
        )
        .unwrap()
    }

    #[test]
    fn look_sets_icons_fonts_and_a_theme_marker() {
        let t = gruvbox();
        let new = with_look(&ours(""), Path::new(DIR), &t).unwrap();
        assert!(
            new.contains(&format!("icon_theme={}\n", t.icons.theme)),
            "{new}"
        );
        assert!(
            new.contains("[Fonts]\ngeneral=\"Inter,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\"\n"),
            "{new}"
        );
        assert!(
            new.contains("fixed=\"JetBrains Mono,11,-1,1,400,0,0,0,1,"),
            "{new}"
        );
        assert!(new.contains("[uQ]\ntheme=gruvbox-dark\n"), "{new}");
        assert_eq!(
            with_look(&new, Path::new(DIR), &t),
            None,
            "second run: unchanged"
        );
    }

    #[test]
    fn look_changes_when_only_the_palette_does() {
        let mut t = gruvbox();
        let once = with_look(&ours(""), Path::new(DIR), &t).unwrap();
        t.id = "dracula".into();
        let twice = with_look(&once, Path::new(DIR), &t).unwrap();
        assert!(twice.contains("theme=dracula-dark"), "{twice}");
        assert_eq!(twice.matches("theme=dracula-dark").count(), 1);
    }

    #[test]
    fn look_skips_unsafe_fonts_and_users_files() {
        let mut t = gruvbox();
        t.fonts.ui = "Bad\"Font,x".into();
        let new = with_look(&ours(""), Path::new(DIR), &t).unwrap();
        assert!(!new.contains("[Fonts]"), "{new}");
        let theirs = "[Appearance]\ncolor_scheme_path=/x.conf\n";
        assert_eq!(with_look(theirs, Path::new(DIR), &gruvbox()), None);
    }
}
