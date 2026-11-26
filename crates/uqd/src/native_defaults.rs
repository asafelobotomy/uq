//! uQ's own apps for the kinds a person never chose (Default apps, the owner's choice of
//! 2026-11-26): once per login, before the startup apps, each kind with one of uQ's apps
//! installed and no choice in the person's lists is given to that app, and remembered, so it is
//! done once per kind. The logic is `uq_core::default_apps::fill`; this reads and writes files.

use std::path::{Path, PathBuf};

use uq_core::default_apps::{desktop_files, fill, scan_handlers};
use uq_core::mimeapps::{MAX_FILE, read_capped};
use uq_core::paths::StartupPaths;

/// The file that remembers which kinds are done, beside `state.toml`.
pub const DONE_FILE: &str = "default-apps-filled";

pub struct Where {
    pub app_dirs: Vec<PathBuf>,
    pub user_list: PathBuf,
    pub overrides: Vec<PathBuf>,
    pub done: PathBuf,
}

impl Where {
    pub fn new(startup: &StartupPaths, state_dir: &Path, desktops: &str) -> Where {
        Where {
            app_dirs: startup.app_dirs(),
            user_list: startup.config_home.join("mimeapps.list"),
            overrides: desktop_files(&startup.config_home, desktops),
            done: state_dir.join(DONE_FILE),
        }
    }
}

/// Give the kinds nobody chose to uQ's own apps; returns the `(kind, app)`s set. A list too
/// large to read whole is never replaced.
pub fn run(w: &Where) -> Vec<(&'static str, String)> {
    if std::fs::metadata(&w.user_list).is_ok_and(|m| !m.is_file() || m.len() > MAX_FILE) {
        return Vec::new();
    }
    let done = fill::parse_done(&std::fs::read_to_string(&w.done).unwrap_or_default());
    let apps = scan_handlers(&w.app_dirs);
    let overrides: Vec<String> = w.overrides.iter().map(|f| read_capped(f)).collect();
    let user = read_capped(&w.user_list);
    let out = fill::fill_native(&user, &overrides, &apps, &done);
    if let Some(text) = &out.text {
        let target = std::fs::canonicalize(&w.user_list).unwrap_or_else(|_| w.user_list.clone());
        if let Err(e) = uq_core::atomic::atomic_write(&target, text) {
            tracing::warn!(
                "[START-004] default apps: writing {}: {e}",
                target.display()
            );
            return Vec::new();
        }
    }
    if out.done != done {
        if let Some(dir) = w.done.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = uq_core::atomic::atomic_write(&w.done, &fill::write_done(&out.done)) {
            tracing::warn!(
                "[START-004] default apps: writing {}: {e}",
                w.done.display()
            );
        }
    }
    for (kind, app) in &out.set {
        tracing::info!("default apps: {kind} now opens with {app}, uQ's own app");
    }
    out.set
}

#[cfg(test)]
mod tests {
    use super::*;
    use uq_core::default_apps::{REELO, UQVIEW};
    use uq_core::mimeapps::default_for;

    #[test]
    fn fills_once_and_keeps_the_persons_choices() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join(UQVIEW),
            "[Desktop Entry]\nType=Application\nName=uQview\nExec=uqview\nMimeType=image/png;image/avif;\n",
        )
        .unwrap();
        std::fs::write(
            apps.join(REELO),
            "[Desktop Entry]\nType=Application\nName=reelo\nExec=reelo\nMimeType=video/mp4;audio/mpeg;\n",
        )
        .unwrap();
        let cfg = d.path().join("config");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("hyprland-mimeapps.list"),
            "[Default Applications]\naudio/mpeg=mpv.desktop;\n",
        )
        .unwrap();
        let w = Where {
            app_dirs: vec![apps],
            user_list: cfg.join("mimeapps.list"),
            overrides: desktop_files(&cfg, "Hyprland"),
            done: d.path().join("state").join(DONE_FILE),
        };
        let set = run(&w);
        assert_eq!(
            set,
            [("images", UQVIEW.to_string()), ("video", REELO.to_string())]
        );
        let user = std::fs::read_to_string(&w.user_list).unwrap();
        assert_eq!(default_for(&user, "image/avif").as_deref(), Some(UQVIEW));
        assert_eq!(
            default_for(&user, "audio/mpeg"),
            None,
            "their desktop list chose music"
        );
        // The person goes back to another viewer; the next login leaves it.
        std::fs::write(
            &w.user_list,
            "[Default Applications]\nimage/png=feh.desktop;\n",
        )
        .unwrap();
        assert!(run(&w).is_empty());
        assert!(
            std::fs::read_to_string(&w.user_list)
                .unwrap()
                .contains("image/png=feh.desktop;")
        );
    }
}
