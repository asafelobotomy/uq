//! Desktop-wide settings outside Hyprland: GTK colour scheme, icons, cursor, fonts.

use std::path::PathBuf;
use uq_core::theme::{Theme, Variant};

pub trait SystemSettings: Send + Sync {
    fn apply(&self, theme: &Theme) -> std::io::Result<()>;
    /// One key's current value (`gsettings get`), as plain text: `'Inter 11'` reads `Inter 11`
    /// and `uint32 24` reads `24`. `None` when gsettings is missing or fails.
    fn read(&self, schema: &str, key: &str) -> Option<String>;
    /// Restart uQ's shell (`uq-shell.service`) if it is running, so it starts again with the
    /// icon theme in the `shell.env` just written: Quickshell reads it only at start.
    fn restart_shell(&self) -> std::io::Result<()>;
}

/// `gsettings get` output as plain text: no type prefix, no quotes around a string.
fn gvariant_text(out: &str) -> String {
    let s = out.trim();
    let s = ["uint32 ", "int32 ", "uint64 ", "int64 ", "double "]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(s);
    s.strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(s)
        .to_string()
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

    fn read(&self, schema: &str, key: &str) -> Option<String> {
        let out = std::process::Command::new(&self.binary)
            .args(["get", schema, key])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| gvariant_text(&String::from_utf8_lossy(&out.stdout)))
    }

    fn restart_shell(&self) -> std::io::Result<()> {
        // try-restart: a stopped shell (a dev session, or one the user stopped) stays stopped.
        // --no-block: uqd must not wait on a unit that may be waiting on uqd's bus.
        let status = std::process::Command::new("systemctl")
            .args(["--user", "--no-block", "try-restart", "uq-shell.service"])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "systemctl --user try-restart uq-shell.service failed: {status}"
            )))
        }
    }
}

/// Records the themes it was asked to apply.
#[cfg(any(test, feature = "fake"))]
#[derive(Debug, Default)]
pub struct FakeSystem {
    pub applied: std::sync::Mutex<Vec<String>>,
    /// Makes `apply` fail, as a missing or broken gsettings would.
    pub fail: std::sync::atomic::AtomicBool,
    /// How many times `restart_shell` was called.
    pub shell_restarts: std::sync::atomic::AtomicUsize,
    /// What `read` answers per `(schema, key)`.
    pub values: std::sync::Mutex<std::collections::HashMap<(String, String), String>>,
}

#[cfg(any(test, feature = "fake"))]
impl FakeSystem {
    pub fn set_value(&self, schema: &str, key: &str, value: &str) {
        self.values
            .lock()
            .unwrap()
            .insert((schema.to_string(), key.to_string()), value.to_string());
    }
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
    fn read(&self, schema: &str, key: &str) -> Option<String> {
        self.values
            .lock()
            .unwrap()
            .get(&(schema.to_string(), key.to_string()))
            .cloned()
    }
    fn restart_shell(&self) -> std::io::Result<()> {
        self.shell_restarts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
    fn gsettings_read_strips_type_and_quotes() {
        assert_eq!(gvariant_text("'Inter 11'\n"), "Inter 11");
        assert_eq!(gvariant_text("uint32 24\n"), "24");
        assert_eq!(gvariant_text("24"), "24");
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("gsettings");
        crate::test_util::write_script(
            &bin,
            "#!/bin/sh\n[ \"$1 $2 $3\" = \"get org.gnome.desktop.interface cursor-theme\" ] || exit 1\necho \"'Breeze'\"\n",
        );
        let g = GSettings::with_binary(&bin);
        // `read` maps every failure to None, so retry a fresh script's ETXTBSY by hand.
        let got = (0..250).find_map(|_| {
            let v = g.read("org.gnome.desktop.interface", "cursor-theme");
            if v.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            v
        });
        assert_eq!(got.as_deref(), Some("Breeze"));
        assert_eq!(
            GSettings::with_binary("/nonexistent/gsettings").read("a", "b"),
            None
        );
    }

    #[test]
    fn gsettings_failure_is_an_error() {
        let res = GSettings::with_binary("/nonexistent/gsettings")
            .apply(&theme("dracula", Variant::Dark));
        assert!(res.is_err());
    }
}
