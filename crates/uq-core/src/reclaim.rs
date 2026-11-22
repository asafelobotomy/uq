//! Take other apps' theme files back for uQ while "Theme other apps" is on.
//!
//! `uqctl setup --apps` (and uqd at every theme change) only creates the files that are missing
//! and leaves any that exist alone. That left apps off uQ's look for good as soon as something
//! else saved one of them: opening the qt6ct app and pressing OK writes Fusion, its own colour
//! scheme and DejaVu fonts into `qt6ct.conf`, and from then on every Qt app that goes through
//! qt6ct (qBittorrent, for one) drew grey Fusion widgets. With the setting on, uqd puts uQ's keys
//! back into those files: its colour scheme, palette and widget style in `qt6ct.conf`, `theme=uq`
//! in Kvantum's config and the theme import at the top of GTK's `gtk.css`. Everything else in
//! the file (qt6ct's interface options, the user's own GTK rules after the import) is kept, and
//! the file as it was is copied next to it first (see [`backup`]).

use crate::atomic::atomic_write;
use crate::paths::Paths;
use crate::qt6ct;
use std::io;
use std::path::{Path, PathBuf};

/// Copy `path` before uQ changes a file it didn't write: to `<name>.before-uq` the first time
/// (kept for good: it is what the user had before uQ), to `<name>.before-uq.last` after that
/// (replaced each time). Returns where the copy went.
pub fn backup(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?
        .to_string_lossy()
        .into_owned();
    let first = path.with_file_name(format!("{name}.before-uq"));
    let to = if first.exists() {
        path.with_file_name(format!("{name}.before-uq.last"))
    } else {
        first
    };
    let text = std::fs::read_to_string(path)?;
    atomic_write(&to, &text)?;
    Ok(to)
}

/// `existing` (a `qt6ct.conf`) with uQ's colour scheme, palette and `style` (`kvantum` or
/// `Fusion`) in `[Appearance]`, or `None` when it already has them. Icons and fonts are
/// [`qt6ct::with_look`]'s, once the file is uQ's again.
pub fn qt6ct_conf(existing: &str, theme_dir: &Path, style: &str) -> Option<String> {
    let scheme = theme_dir.join("qt6ct/colors/uq.conf").display().to_string();
    let mut new = existing.to_string();
    for (key, value) in [
        ("style", style),
        ("custom_palette", "true"),
        ("color_scheme_path", scheme.as_str()),
    ] {
        if !has_key(&new, "Appearance", key, value) {
            new = qt6ct::set_key(&new, "Appearance", key, value);
        }
    }
    (new != existing).then_some(new)
}

/// `existing` (Kvantum's `kvantum.kvconfig`) with `theme=uq` in `[General]`, or `None`.
pub fn kvconfig(existing: &str) -> Option<String> {
    (!has_key(existing, "General", "theme", "uq"))
        .then(|| qt6ct::set_key(existing, "General", "theme", "uq"))
}

/// `existing` (a `gtk.css`) with `import` (uQ's `@import` line) first, or `None` when it is
/// there already. The user's own rules stay below it, so they still win over uQ's.
pub fn gtk_css(existing: &str, import: &str) -> Option<String> {
    let import = import.trim_end();
    if existing.lines().any(|l| l.trim() == import) {
        return None;
    }
    Some(if existing.trim().is_empty() {
        format!("{import}\n")
    } else {
        format!("{import}\n{existing}")
    })
}

/// Whether `[section]` has exactly `key=value` (spaces around `=` allowed).
fn has_key(text: &str, section: &str, key: &str, value: &str) -> bool {
    let header = format!("[{section}]");
    let mut current = "";
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            current = t;
        } else if current == header
            && let Some((k, v)) = t.split_once('=')
            && k.trim() == key
        {
            return v.trim() == value;
        }
    }
    false
}

/// Put uQ's keys back into the app theme files that exist and lost them (see the module docs).
/// `kvantum`: Kvantum is installed and switched on. Returns the files that changed. A file that
/// doesn't exist is left to [`crate::stub::install_consumers`], which creates it.
pub fn reclaim(p: &Paths, kvantum: bool) -> io::Result<Vec<PathBuf>> {
    let cfg = p.hypr_dir.parent().expect("hypr dir has a parent");
    let theme = p.theme_dir.to_string_lossy().into_owned();
    let style = if kvantum { "kvantum" } else { "Fusion" };
    let mut changed = Vec::new();
    let mut fix =
        |path: PathBuf, f: &dyn Fn(&str) -> Option<String>, ours: &dyn Fn(&str) -> bool| {
            let existing = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
            };
            let Some(new) = f(&existing) else {
                return Ok(());
            };
            if !ours(&existing) {
                backup(&path)?;
            }
            atomic_write(&path, &new)?;
            changed.push(path);
            Ok::<(), io::Error>(())
        };
    let theme_dir = p.theme_dir.clone();
    fix(
        cfg.join("qt6ct/qt6ct.conf"),
        &|s| qt6ct_conf(s, &theme_dir, style),
        &|s| qt6ct::is_ours(s, &theme_dir),
    )?;
    if kvantum {
        fix(cfg.join("Kvantum/kvantum.kvconfig"), &kvconfig, &|_| false)?;
    }
    for gtk in ["gtk-3.0", "gtk-4.0"] {
        let import = format!("@import url(\"file://{theme}/{gtk}/gtk.css\");");
        fix(
            cfg.join(format!("{gtk}/gtk.css")),
            &|s| gtk_css(s, &import),
            // Adding a line on top changes nothing the user wrote: no copy needed.
            &|_| true,
        )?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const DIR: &str = "/h/.local/state/uq/theme";

    /// What the qt6ct app writes when you press OK (the owner's file, 2026-11-19).
    const QT6CT_APP: &str = "[Appearance]\ncolor_scheme_path=/usr/share/qt6ct/colors/darker.conf\ncustom_palette=true\nstandard_dialogs=gtk3\nstyle=Fusion\n\n[Fonts]\nfixed=\"DejaVu LGC Sans,12,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,,0,0\"\ngeneral=\"DejaVu LGC Sans,12,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,,0,0\"\n\n[Interface]\nactivate_item_on_single_click=1\n";

    #[test]
    fn qt6ct_apps_file_gets_uqs_scheme_and_style_back() {
        let new = qt6ct_conf(QT6CT_APP, Path::new(DIR), "kvantum").unwrap();
        assert!(qt6ct::is_ours(&new, Path::new(DIR)), "{new}");
        assert_eq!(new.matches("color_scheme_path=").count(), 1, "{new}");
        assert_eq!(new.matches("style=").count(), 1, "{new}");
        assert!(new.contains("style=kvantum\n"), "{new}");
        assert!(new.contains("custom_palette=true\n"), "{new}");
        // Everything else is kept.
        assert!(new.contains("standard_dialogs=gtk3\n"), "{new}");
        assert!(
            new.contains("[Interface]\nactivate_item_on_single_click=1\n"),
            "{new}"
        );
        assert_eq!(
            qt6ct_conf(&new, Path::new(DIR), "kvantum"),
            None,
            "second pass"
        );
        // The style follows Kvantum's switch.
        let fusion = qt6ct_conf(&new, Path::new(DIR), "Fusion").unwrap();
        assert!(fusion.contains("style=Fusion\n") && !fusion.contains("style=kvantum"));
    }

    #[test]
    fn kvconfig_and_gtk_css() {
        assert_eq!(kvconfig("[General]\ntheme=uq\n"), None);
        assert_eq!(
            kvconfig("[General]\ntheme=KvArcDark\n").unwrap(),
            "[General]\ntheme=uq\n"
        );
        assert_eq!(kvconfig("").unwrap(), "[General]\ntheme=uq\n");
        let imp = "@import url(\"file:///t/gtk-3.0/gtk.css\");";
        assert_eq!(gtk_css(&format!("{imp}\n"), imp), None);
        assert_eq!(
            gtk_css("button { color: red; }\n", imp).unwrap(),
            format!("{imp}\nbutton {{ color: red; }}\n")
        );
        assert_eq!(gtk_css("", imp).unwrap(), format!("{imp}\n"));
    }

    #[test]
    fn reclaim_rewrites_changed_files_and_keeps_copies() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::under(d.path());
        let cfg = p.hypr_dir.parent().unwrap().to_path_buf();
        let qt = cfg.join("qt6ct/qt6ct.conf");
        fs::create_dir_all(qt.parent().unwrap()).unwrap();
        fs::write(&qt, QT6CT_APP).unwrap();
        let kv = cfg.join("Kvantum/kvantum.kvconfig");
        fs::create_dir_all(kv.parent().unwrap()).unwrap();
        fs::write(&kv, "[General]\ntheme=KvGnomeDark\n").unwrap();

        let changed = reclaim(&p, true).unwrap();
        assert_eq!(changed, vec![qt.clone(), kv.clone()]);
        assert!(qt6ct::is_ours(
            &fs::read_to_string(&qt).unwrap(),
            &p.theme_dir
        ));
        assert_eq!(
            fs::read_to_string(qt.with_file_name("qt6ct.conf.before-uq")).unwrap(),
            QT6CT_APP
        );
        assert_eq!(fs::read_to_string(&kv).unwrap(), "[General]\ntheme=uq\n");
        assert!(kv.with_file_name("kvantum.kvconfig.before-uq").exists());
        assert!(reclaim(&p, true).unwrap().is_empty(), "second run");

        // Saved over again: the first copy stays, the newest goes to .last.
        fs::write(&qt, QT6CT_APP.replace("darker", "airy")).unwrap();
        reclaim(&p, true).unwrap();
        assert_eq!(
            fs::read_to_string(qt.with_file_name("qt6ct.conf.before-uq")).unwrap(),
            QT6CT_APP
        );
        assert!(
            fs::read_to_string(qt.with_file_name("qt6ct.conf.before-uq.last"))
                .unwrap()
                .contains("airy")
        );
    }

    #[test]
    fn reclaim_leaves_missing_files_alone() {
        let d = tempfile::tempdir().unwrap();
        let p = Paths::under(d.path());
        assert!(reclaim(&p, true).unwrap().is_empty());
        assert!(!p.hypr_dir.parent().unwrap().join("qt6ct").exists());
    }
}
