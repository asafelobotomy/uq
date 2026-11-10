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
    /// `$UQ_DATA_DIR/themes`
    pub themes_dir: PathBuf,
    /// `$UQ_DATA_DIR/templates`
    pub templates_dir: PathBuf,
    /// `/usr/share/hypr/hyprland.lua`: Hyprland's stock config. A copy of it in `main_lua` is not
    /// the user's own work, so `uqctl setup` may replace it.
    pub hypr_default: PathBuf,
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
        )
    }

    /// Everything under one root, for tests.
    pub fn under(root: &Path) -> Paths {
        Paths::build(
            &root.join("config"),
            &root.join("state"),
            &root.join("share"),
            &root.join("hypr-share/hyprland.lua"),
        )
    }

    fn build(config: &Path, state: &Path, data: &Path, hypr_default: &Path) -> Paths {
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
            themes_dir: data.join("themes"),
            templates_dir: data.join("templates"),
            hypr_default: hypr_default.to_path_buf(),
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
        ] {
            assert!(path.starts_with("/tmp/r"), "{path:?}");
        }
    }
}
