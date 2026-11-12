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
use std::path::{Path, PathBuf};

/// `~/.config/qt6ct/qt6ct.conf`, next to `~/.config/hypr`.
pub fn conf_path(p: &Paths) -> PathBuf {
    p.hypr_dir
        .parent()
        .expect("hypr dir has a parent")
        .join("qt6ct/qt6ct.conf")
}

/// `existing` with `icon_theme=<icon>` in its `[Appearance]` section, or `None` when nothing
/// should change: the file doesn't load uQ's colour scheme from `theme_dir`, the name isn't a
/// valid theme name, or it already says so.
pub fn with_icon_theme(existing: &str, theme_dir: &Path, icon: &str) -> Option<String> {
    let marker = format!(
        "color_scheme_path={}",
        theme_dir.join("qt6ct/colors/uq.conf").display()
    );
    if !valid_theme_name(icon) || !existing.lines().any(|l| l.trim() == marker) {
        return None;
    }
    let wanted = format!("icon_theme={icon}");
    let mut out: Vec<String> = Vec::new();
    let mut section = String::new();
    let mut done = false;
    for line in existing.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            section = t.to_string();
            out.push(line.to_string());
            if section == "[Appearance]" && !done {
                out.push(wanted.clone());
                done = true;
            }
            continue;
        }
        // The line placed right after the header replaces any icon_theme already there.
        if section == "[Appearance]" && t.split('=').next().map(str::trim) == Some("icon_theme") {
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        out.push("[Appearance]".into());
        out.push(wanted);
    }
    let mut new = out.join("\n");
    new.push('\n');
    (new != existing).then_some(new)
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
}
