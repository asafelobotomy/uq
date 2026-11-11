//! The fixed paths the helper reads and writes. Every one is a constant under a root prefix:
//! `/` in production, a temporary directory in tests. Nothing a caller sends becomes a path,
//! except a validated theme name and variant, which only select a shipped directory to check.

use crate::values::{GreeterVariant, ThemeName};
use std::path::{Path, PathBuf};

/// logind drop-in for the lid and power-key actions.
pub const LOGIND_DROPIN: &str = "etc/systemd/logind.conf.d/50-uq.conf";
/// The greeter's theme, variant and options.
pub const GREETER_TOML: &str = "etc/uq/greeter.toml";
/// The greeter's optional monitor layout.
pub const GREETER_MONITORS: &str = "etc/uq/greeter-monitors.toml";
/// uQ's greetd config; the helper edits only its `[initial_session]` table.
pub const GREETD_TOML: &str = "etc/uq/greetd.toml";
/// The greeter's wallpaper: a byte copy of the user's image, never decoded here.
pub const GREETER_WALLPAPER: &str = "var/lib/uq-greeter/wallpaper";
/// Pre-rendered theme variants shipped by the package, one directory per `<theme>-<variant>`.
pub const RENDERED_THEMES: &str = "usr/share/uq/themes/rendered";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    root: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self::system()
    }
}

impl Config {
    /// The real system: every path under `/`.
    pub fn system() -> Self {
        Self {
            root: PathBuf::from("/"),
        }
    }

    /// Every path under `root` instead of `/` (tests).
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn logind_dropin(&self) -> PathBuf {
        self.at(LOGIND_DROPIN)
    }

    pub fn greeter_toml(&self) -> PathBuf {
        self.at(GREETER_TOML)
    }

    pub fn greeter_monitors(&self) -> PathBuf {
        self.at(GREETER_MONITORS)
    }

    pub fn greetd_toml(&self) -> PathBuf {
        self.at(GREETD_TOML)
    }

    pub fn greeter_wallpaper(&self) -> PathBuf {
        self.at(GREETER_WALLPAPER)
    }

    /// `tokens.json` of a shipped pre-rendered variant. `theme` is already validated as
    /// `[a-z0-9-]{1,40}`, so the joined name has no separator or dot segment.
    pub fn rendered_tokens(&self, theme: &ThemeName, variant: GreeterVariant) -> PathBuf {
        self.at(RENDERED_THEMES)
            .join(format!("{}-{}", theme.as_str(), variant.as_str()))
            .join("tokens.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_paths_are_the_fixed_ones() {
        let c = Config::system();
        assert_eq!(
            c.logind_dropin(),
            Path::new("/etc/systemd/logind.conf.d/50-uq.conf")
        );
        assert_eq!(c.greeter_toml(), Path::new("/etc/uq/greeter.toml"));
        assert_eq!(
            c.greeter_monitors(),
            Path::new("/etc/uq/greeter-monitors.toml")
        );
        assert_eq!(c.greetd_toml(), Path::new("/etc/uq/greetd.toml"));
        assert_eq!(
            c.greeter_wallpaper(),
            Path::new("/var/lib/uq-greeter/wallpaper")
        );
        let t = ThemeName::parse("dracula").unwrap();
        assert_eq!(
            c.rendered_tokens(&t, GreeterVariant::Dark),
            Path::new("/usr/share/uq/themes/rendered/dracula-dark/tokens.json")
        );
    }

    #[test]
    fn a_root_prefix_moves_every_path() {
        let c = Config::with_root("/tmp/x");
        for p in [
            c.logind_dropin(),
            c.greeter_toml(),
            c.greeter_monitors(),
            c.greetd_toml(),
            c.greeter_wallpaper(),
        ] {
            assert!(p.starts_with("/tmp/x/"), "{}", p.display());
        }
    }
}
