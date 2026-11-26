//! Every setting at once: "Reset all settings" (`ResetAll`) puts each back to uQ's defaults,
//! and a settings backup (`ExportSettings`, `ImportSettings`) is `state.toml` as text, brought
//! back as a restore point is. Each saves a restore point of the current settings first, so
//! History › Restore points can undo it; launch counts and Welcome's bookkeeping stay.

use crate::service::{Desktop, DesktopError};
use uq_core::history::AutoReason;
use uq_core::state::State;

/// The largest backup `ImportSettings` reads (a real one is a few KiB).
pub const MAX_BACKUP_BYTES: usize = 1024 * 1024;

/// The first line of every backup, so a file can be told apart from any TOML.
pub const BACKUP_HEADER: &str = "# uQ settings backup";

impl Desktop {
    /// Every setting back to uQ's defaults. Returns plain notices, as `restore` does.
    pub async fn reset_all(&mut self) -> Result<Vec<String>, DesktopError> {
        self.replace_all(
            State::default(),
            "Before resetting all settings",
            AutoReason::BeforeReset,
        )
        .await
    }

    /// The current settings as a backup: `state.toml`'s text without launch counts,
    /// Welcome's bookkeeping or the schedule's last flip, after a header line.
    pub fn export_settings(&self) -> Result<String, DesktopError> {
        let mut s = self.state.clone();
        s.launch_counts.clear();
        s.welcome = Default::default();
        s.schedule_mark = None;
        let body = s
            .to_toml()
            .map_err(|e| DesktopError::Io(format!("writing the backup: {e}")))?;
        Ok(format!(
            "{BACKUP_HEADER} (uQ {}). Bring it back in Settings › Restore, or `uqctl backup restore FILE`.\n{body}",
            env!("CARGO_PKG_VERSION")
        ))
    }

    /// Make a backup's settings the current ones. A file that isn't a backup, or one whose
    /// theme isn't installed, changes nothing.
    pub async fn import_settings(&mut self, text: &str) -> Result<Vec<String>, DesktopError> {
        if text.len() > MAX_BACKUP_BYTES {
            return Err(DesktopError::InvalidArgument(
                "that file is too big to be a uQ settings backup".into(),
            ));
        }
        if !text.starts_with(BACKUP_HEADER) {
            return Err(DesktopError::InvalidArgument(
                "that file isn't a uQ settings backup".into(),
            ));
        }
        let next = State::parse_strict(text).map_err(|e| {
            DesktopError::InvalidArgument(format!("that backup can't be read: {}", e.message()))
        })?;
        self.replace_all(
            next,
            "Before restoring a settings backup",
            AutoReason::BeforeBackup,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::fake::FakeCompositor;
    use crate::system::FakeSystem;
    use std::path::PathBuf;
    use std::sync::Arc;
    use uq_core::history::{Origin, PointKind};
    use uq_core::paths::Paths;

    async fn desktop(dir: &std::path::Path) -> Desktop {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths = Paths::under(dir);
        paths.themes_dir = repo.join("themes");
        paths.templates_dir = repo.join("templates");
        let comp = Arc::new(FakeCompositor::running());
        let mut d = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
        d.startup().await;
        d.set_section("shell", r#"{"clock_24h":false}"#, Origin::Settings)
            .await
            .unwrap();
        d
    }

    #[tokio::test]
    async fn reset_saves_a_point_then_puts_back_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = desktop(dir.path()).await;
        d.state.launch_counts.insert("firefox".into(), 3);
        d.reset_all().await.unwrap();
        assert!(d.state.settings.shell.clock_24h);
        assert_eq!(d.state.launch_counts.get("firefox"), Some(&3));
        let p = &d.restore.list()[0];
        assert_eq!(p.kind, PointKind::Auto(AutoReason::BeforeReset));
        assert!(!d.restore.load(&p.id).unwrap().settings.shell.clock_24h);
        assert!(State::load(&d.paths().state_file).settings.shell.clock_24h);
    }

    #[tokio::test]
    async fn a_backup_comes_back_and_anything_else_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = desktop(dir.path()).await;
        let backup = d.export_settings().unwrap();
        assert!(backup.starts_with(BACKUP_HEADER), "{backup}");
        d.reset_all().await.unwrap();
        assert!(d.state.settings.shell.clock_24h);
        let points = d.restore.list().len();
        for bad in ["theme = \"x\"\n", "# uQ settings backup\ntheme = [", ""] {
            assert!(d.import_settings(bad).await.is_err(), "{bad:?}");
        }
        assert_eq!(d.restore.list().len(), points);
        d.import_settings(&backup).await.unwrap();
        assert!(!d.state.settings.shell.clock_24h);
        let p = &d.restore.list()[0];
        assert_eq!(p.kind, PointKind::Auto(AutoReason::BeforeBackup));
    }
}
