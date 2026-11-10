//! The cursor and icon themes installed on this system, for the Appearance page's pickers.

use std::collections::BTreeSet;
use std::path::PathBuf;
use uq_core::settings::valid_theme_name;

/// The folders themes are installed in: system, user, and the legacy `~/.icons`.
pub fn real_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/icons")];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        dirs.push(data.join("icons"));
        dirs.push(home.join(".icons"));
    }
    dirs
}

/// Names of the theme directories under `dirs` that satisfy `keep` and that uqd would accept
/// ([`valid_theme_name`]), sorted and de-duplicated.
fn themes(dirs: &[PathBuf], keep: impl Fn(&std::path::Path) -> bool) -> Vec<String> {
    let mut names = BTreeSet::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && keep(&path)
                && let Some(name) = entry.file_name().to_str()
                && valid_theme_name(name)
            {
                names.insert(name.to_owned());
            }
        }
    }
    names.into_iter().collect()
}

/// Directories that contain `cursors/`.
pub fn cursor_themes(dirs: &[PathBuf]) -> Vec<String> {
    themes(dirs, |p| p.join("cursors").is_dir())
}

/// Directories with an `index.theme` and at least one subdirectory other than `cursors`.
pub fn icon_themes(dirs: &[PathBuf]) -> Vec<String> {
    themes(dirs, |p| {
        p.join("index.theme").is_file()
            && std::fs::read_dir(p).is_ok_and(|rd| {
                rd.flatten()
                    .any(|e| e.path().is_dir() && e.file_name() != "cursors")
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cursor_and_icon_themes_from_dirs() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mk = |root: &std::path::Path, theme: &str, subs: &[&str], index: bool| {
            let t = root.join(theme);
            for s in subs {
                fs::create_dir_all(t.join(s)).unwrap();
            }
            fs::create_dir_all(&t).unwrap();
            if index {
                fs::write(t.join("index.theme"), "[Icon Theme]\n").unwrap();
            }
        };
        mk(a.path(), "Bibata", &["cursors"], true); // cursor only
        mk(a.path(), "Papirus", &["48x48/apps"], true); // icons only
        mk(a.path(), "Mixed", &["cursors", "scalable"], true); // both
        mk(a.path(), "NoIndex", &["16x16"], false); // not an icon theme
        mk(b.path(), "Papirus", &["32x32"], true); // duplicate
        mk(b.path(), "Adwaita", &["cursors"], false); // cursor theme, no index needed
        fs::write(b.path().join("stray-file"), "x").unwrap();
        // Names uqd would refuse are not offered.
        mk(b.path(), "Bad\u{e9}Name", &["cursors", "scalable"], true);
        mk(b.path(), &"L".repeat(65), &["cursors", "scalable"], true);
        mk(b.path(), "has:colon", &["cursors", "scalable"], true);

        let dirs = [
            a.path().to_path_buf(),
            b.path().to_path_buf(),
            a.path().join("missing"),
        ];
        assert_eq!(cursor_themes(&dirs), ["Adwaita", "Bibata", "Mixed"]);
        assert_eq!(icon_themes(&dirs), ["Mixed", "Papirus"]);
    }
}
