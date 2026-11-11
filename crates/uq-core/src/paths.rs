//! Filesystem locations uQ reads and writes (spec §2.1, §2.5).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `~/.config/hypr`
    pub hypr_dir: PathBuf,
    /// `~/.config/hypr/hyprland.lua` — the three-line stub.
    pub main_lua: PathBuf,
    /// `~/.config/hypr/uq/generated.lua` — written only by uqd.
    pub generated_lua: PathBuf,
    /// `~/.config/hypr/uq/user.lua` — the user's file; never rewritten.
    pub user_lua: PathBuf,
    /// `$UQ_DATA_DIR/hypr/base.lua`
    pub base_lua: PathBuf,
    /// `~/.local/state/uq/theme`
    pub theme_dir: PathBuf,
    /// `~/.local/state/uq/state.toml`
    pub state_file: PathBuf,
    /// `~/.local/state/uq/history`: the settings journal.
    pub history_dir: PathBuf,
    /// `~/.local/state/uq/restore`: restore points.
    pub restore_dir: PathBuf,
    /// `~/.local/state/uq/profiles`: one `<id>.toml` per profile.
    pub profiles_dir: PathBuf,
    /// `~/.local/state/uq/profile-active.json`: which profile is on, and what it replaced.
    pub profile_active: PathBuf,
    /// `$UQ_DATA_DIR/themes`
    pub themes_dir: PathBuf,
    /// `$UQ_DATA_DIR/templates`
    pub templates_dir: PathBuf,
    /// `/usr/share/hypr/hyprland.lua`: Hyprland's stock config. A copy of it in `main_lua` is not
    /// the user's own work, so `uqctl setup` may replace it.
    pub hypr_default: PathBuf,
    /// The XDG folders the Startup page reads and writes (autostart, applications, runtime).
    pub startup: StartupPaths,
}

/// The XDG base directories autostart and launcher lookups use. `Paths::under` puts every one
/// inside the test root, so no test reads or writes the real home (F1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupPaths {
    /// `$XDG_CONFIG_HOME`, default `~/.config`.
    pub config_home: PathBuf,
    /// `$XDG_CONFIG_DIRS`, default `/etc/xdg`.
    pub config_dirs: Vec<PathBuf>,
    /// `$XDG_DATA_HOME`, default `~/.local/share`.
    pub data_home: PathBuf,
    /// `$XDG_DATA_DIRS`, default `/usr/local/share:/usr/share`.
    pub data_dirs: Vec<PathBuf>,
    /// `$XDG_RUNTIME_DIR`; `None` when unset.
    pub runtime_dir: Option<PathBuf>,
    /// `$HOME`: an autostart entry's working directory when it names none.
    pub home: PathBuf,
}

impl StartupPaths {
    /// Resolve from an environment lookup. Relative values are ignored, as the XDG base
    /// directory spec asks.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> StartupPaths {
        let abs = |k: &str| get(k).map(PathBuf::from).filter(|p| p.is_absolute());
        let list = |k: &str, default: &str| {
            let dirs: Vec<PathBuf> = get(k)
                .unwrap_or_default()
                .split(':')
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .collect();
            if dirs.is_empty() {
                default.split(':').map(PathBuf::from).collect()
            } else {
                dirs
            }
        };
        let home = PathBuf::from(get("HOME").unwrap_or_else(|| "/".into()));
        StartupPaths {
            config_home: abs("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            config_dirs: list("XDG_CONFIG_DIRS", "/etc/xdg"),
            data_home: abs("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
            data_dirs: list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            runtime_dir: abs("XDG_RUNTIME_DIR"),
            home,
        }
    }

    /// Everything under one root, for tests.
    pub fn under(root: &Path) -> StartupPaths {
        StartupPaths {
            config_home: root.join("config"),
            config_dirs: vec![root.join("xdg")],
            data_home: root.join("data"),
            data_dirs: vec![root.join("share-dirs")],
            runtime_dir: Some(root.join("run")),
            home: root.join("home"),
        }
    }

    /// The user's autostart folder.
    pub fn user_autostart(&self) -> PathBuf {
        self.config_home.join("autostart")
    }

    /// The system autostart folders, in lookup order.
    pub fn system_autostart(&self) -> Vec<PathBuf> {
        self.config_dirs
            .iter()
            .map(|d| d.join("autostart"))
            .collect()
    }

    /// The applications folders, data home first, in lookup order.
    pub fn app_dirs(&self) -> Vec<PathBuf> {
        std::iter::once(&self.data_home)
            .chain(&self.data_dirs)
            .map(|d| d.join("applications"))
            .collect()
    }
}

impl Paths {
    pub fn from_env() -> Paths {
        Paths::from_vars(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
    }

    /// Resolve paths from an environment lookup (testable without touching the process env).
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Paths {
        let home = PathBuf::from(get("HOME").unwrap_or_else(|| "/".into()));
        let config = get("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let state = get("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let data = get("UQ_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/share/uq"));
        Paths::build(
            &config,
            &state,
            &data,
            Path::new("/usr/share/hypr/hyprland.lua"),
            StartupPaths::from_vars(get),
        )
    }

    /// Everything under one root, for tests.
    pub fn under(root: &Path) -> Paths {
        Paths::build(
            &root.join("config"),
            &root.join("state"),
            &root.join("share"),
            &root.join("hypr-share/hyprland.lua"),
            StartupPaths::under(root),
        )
    }

    fn build(
        config: &Path,
        state: &Path,
        data: &Path,
        hypr_default: &Path,
        startup: StartupPaths,
    ) -> Paths {
        let hypr_dir = config.join("hypr");
        Paths {
            main_lua: hypr_dir.join("hyprland.lua"),
            generated_lua: hypr_dir.join("uq/generated.lua"),
            user_lua: hypr_dir.join("uq/user.lua"),
            hypr_dir,
            base_lua: data.join("hypr/base.lua"),
            theme_dir: state.join("uq/theme"),
            state_file: state.join("uq/state.toml"),
            history_dir: state.join("uq/history"),
            restore_dir: state.join("uq/restore"),
            profiles_dir: state.join("uq/profiles"),
            profile_active: state.join("uq/profile-active.json"),
            themes_dir: data.join("themes"),
            templates_dir: data.join("templates"),
            hypr_default: hypr_default.to_path_buf(),
            startup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_vars_honours_xdg_and_data_dir() {
        let p = Paths::from_vars(|k| match k {
            "HOME" => Some("/home/u".into()),
            "XDG_CONFIG_HOME" => Some("/cfg".into()),
            "XDG_STATE_HOME" => Some("/st".into()),
            "UQ_DATA_DIR" => Some("/opt/uq".into()),
            _ => None,
        });
        assert_eq!(p.main_lua, PathBuf::from("/cfg/hypr/hyprland.lua"));
        assert_eq!(p.generated_lua, PathBuf::from("/cfg/hypr/uq/generated.lua"));
        assert_eq!(p.user_lua, PathBuf::from("/cfg/hypr/uq/user.lua"));
        assert_eq!(p.theme_dir, PathBuf::from("/st/uq/theme"));
        assert_eq!(p.state_file, PathBuf::from("/st/uq/state.toml"));
        assert_eq!(p.history_dir, PathBuf::from("/st/uq/history"));
        assert_eq!(p.restore_dir, PathBuf::from("/st/uq/restore"));
        assert_eq!(p.base_lua, PathBuf::from("/opt/uq/hypr/base.lua"));
        assert_eq!(p.themes_dir, PathBuf::from("/opt/uq/themes"));
        assert_eq!(p.templates_dir, PathBuf::from("/opt/uq/templates"));
    }

    #[test]
    fn from_vars_defaults() {
        let p = Paths::from_vars(|k| (k == "HOME").then(|| "/home/u".to_string()));
        assert_eq!(p.hypr_dir, PathBuf::from("/home/u/.config/hypr"));
        assert_eq!(
            p.state_file,
            PathBuf::from("/home/u/.local/state/uq/state.toml")
        );
        assert_eq!(p.base_lua, PathBuf::from("/usr/share/uq/hypr/base.lua"));
    }

    #[test]
    fn under_root_puts_everything_inside() {
        let p = Paths::under(Path::new("/tmp/r"));
        for path in [
            &p.main_lua,
            &p.generated_lua,
            &p.user_lua,
            &p.base_lua,
            &p.theme_dir,
            &p.state_file,
            &p.history_dir,
            &p.restore_dir,
            &p.profiles_dir,
            &p.profile_active,
        ] {
            assert!(path.starts_with("/tmp/r"), "{path:?}");
        }
    }

    #[test]
    fn paths_under_sandboxes_startup() {
        let root = Path::new("/tmp/r");
        let s = Paths::under(root).startup;
        let mut all = vec![&s.config_home, &s.data_home, &s.home];
        all.extend(&s.config_dirs);
        all.extend(&s.data_dirs);
        all.extend(&s.runtime_dir);
        assert_eq!(all.len(), 6);
        for path in all {
            assert!(path.starts_with(root), "{path:?}");
        }
        assert!(s.user_autostart().starts_with(root));
        for d in s.system_autostart().iter().chain(&s.app_dirs()) {
            assert!(d.starts_with(root), "{d:?}");
        }
    }

    #[test]
    fn startup_from_vars() {
        let p = Paths::from_vars(|k| match k {
            "HOME" => Some("/home/u".into()),
            "XDG_CONFIG_DIRS" => Some("/a::relative:/b".into()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1".into()),
            "XDG_DATA_HOME" => Some("/dh".into()),
            _ => None,
        });
        let s = &p.startup;
        assert_eq!(s.home, PathBuf::from("/home/u"));
        assert_eq!(s.config_home, PathBuf::from("/home/u/.config"));
        assert_eq!(s.config_dirs, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(s.data_home, PathBuf::from("/dh"));
        assert_eq!(
            s.data_dirs,
            [
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
        assert_eq!(s.runtime_dir, Some(PathBuf::from("/run/user/1")));
        assert_eq!(
            s.user_autostart(),
            PathBuf::from("/home/u/.config/autostart")
        );
        assert_eq!(
            s.system_autostart(),
            [PathBuf::from("/a/autostart"), PathBuf::from("/b/autostart")]
        );
        assert_eq!(s.app_dirs()[0], PathBuf::from("/dh/applications"));
        let d = Paths::from_vars(|k| (k == "HOME").then(|| "/home/u".to_string())).startup;
        assert_eq!(d.config_dirs, [PathBuf::from("/etc/xdg")]);
        assert_eq!(d.data_home, PathBuf::from("/home/u/.local/share"));
        assert_eq!(d.runtime_dir, None);
    }
}
