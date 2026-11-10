//! Desktop-wide settings outside Hyprland: GTK colour scheme, icons, cursor, fonts.

use std::path::PathBuf;
use uq_core::theme::{Theme, Variant};

pub trait SystemSettings: Send + Sync {
    fn apply(&self, theme: &Theme) -> std::io::Result<()>;
}

const IFACE: &str = "org.gnome.desktop.interface";

/// `[schema, key, value]` for every gsettings key a theme sets.
pub fn gsettings_commands(t: &Theme) -> Vec<[String; 3]> {
    let scheme = match t.variant {
        Variant::Dark => "prefer-dark",
        Variant::Light => "prefer-light",
    };
    [
        ("color-scheme", scheme.to_string()),
        ("icon-theme", t.icons.theme.clone()),
        ("cursor-theme", t.icons.cursor_theme.clone()),
        ("cursor-size", t.icons.cursor_size.to_string()),
        ("font-name", format!("{} {}", t.fonts.ui, t.fonts.size)),
        (
            "monospace-font-name",
            format!("{} {}", t.fonts.mono, t.fonts.size),
        ),
    ]
    .into_iter()
    .map(|(k, v)| [IFACE.to_string(), k.to_string(), v])
    .collect()
}

#[derive(Debug, Clone)]
pub struct GSettings {
    binary: PathBuf,
}

impl Default for GSettings {
    fn default() -> Self {
        GSettings {
            binary: PathBuf::from("gsettings"),
        }
    }
}

impl GSettings {
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        GSettings {
            binary: binary.into(),
        }
    }
}

impl SystemSettings for GSettings {
    fn apply(&self, theme: &Theme) -> std::io::Result<()> {
        for [schema, key, value] in gsettings_commands(theme) {
            let status = std::process::Command::new(&self.binary)
                .args(["set", &schema, &key, &value])
                .status()?;
            if !status.success() {
                return Err(std::io::Error::other(format!(
                    "gsettings set {schema} {key} failed: {status}"
                )));
            }
        }
        Ok(())
    }
}

/// Records the themes it was asked to apply.
#[cfg(any(test, feature = "fake"))]
#[derive(Debug, Default)]
pub struct FakeSystem {
    pub applied: std::sync::Mutex<Vec<String>>,
    /// Makes `apply` fail, as a missing or broken gsettings would.
    pub fail: std::sync::atomic::AtomicBool,
}

#[cfg(any(test, feature = "fake"))]
impl SystemSettings for FakeSystem {
    fn apply(&self, theme: &Theme) -> std::io::Result<()> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::other("gsettings failed"));
        }
        self.applied
            .lock()
            .unwrap()
            .push(format!("{} {}", theme.id, theme.variant));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uq_core::theme::{ThemeRegistry, Variant};

    fn theme(id: &str, v: Variant) -> Theme {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        ThemeRegistry::load_dir(&dir)
            .unwrap()
            .get(id, v)
            .unwrap()
            .clone()
    }

    #[test]
    fn gsettings_commands_for_dark_theme() {
        let cmds = gsettings_commands(&theme("gruvbox", Variant::Dark));
        let get = |key: &str| cmds.iter().find(|c| c[1] == key).map(|c| c[2].clone());
        assert!(cmds.iter().all(|c| c[0] == "org.gnome.desktop.interface"));
        assert_eq!(get("color-scheme").as_deref(), Some("prefer-dark"));
        assert_eq!(get("icon-theme").as_deref(), Some("Papirus-Dark"));
        assert_eq!(
            get("cursor-theme").as_deref(),
            Some("Bibata-Modern-Classic")
        );
        assert_eq!(get("cursor-size").as_deref(), Some("24"));
        assert_eq!(get("font-name").as_deref(), Some("Inter 11"));
        assert_eq!(
            get("monospace-font-name").as_deref(),
            Some("JetBrains Mono 11")
        );
    }

    #[test]
    fn gsettings_light_prefers_light() {
        let cmds = gsettings_commands(&theme("graphite", Variant::Light));
        assert!(
            cmds.iter()
                .any(|c| c[1] == "color-scheme" && c[2] == "prefer-light")
        );
    }

    #[test]
    fn gsettings_apply_runs_binary_per_key() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let bin = dir.path().join("gsettings");
        crate::test_util::write_script(
            &bin,
            &format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display()),
        );
        let t = theme("dracula", Variant::Dark);
        crate::test_util::retry_busy(|| GSettings::with_binary(&bin).apply(&t)).unwrap();
        let lines = std::fs::read_to_string(&log).unwrap();
        assert_eq!(lines.lines().count(), 6);
        assert!(
            lines
                .lines()
                .any(|l| l == "set org.gnome.desktop.interface color-scheme prefer-dark"),
            "{lines}"
        );
    }

    #[test]
    fn gsettings_failure_is_an_error() {
        let res = GSettings::with_binary("/nonexistent/gsettings")
            .apply(&theme("dracula", Variant::Dark));
        assert!(res.is_err());
    }
}
