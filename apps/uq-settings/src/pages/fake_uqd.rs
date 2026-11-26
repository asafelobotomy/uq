//! One fake uqd for every page model's tests: `org.uq.Desktop1` over a real
//! [`uq_core::state::State`] on a private bus, so refusals carry uq-core's own messages. Writes
//! are journalled like uqd does (one entry per changed domain, with the write's origin), and
//! `Undo`, the restore points and the profiles follow uqd's rules.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use uq_core::client::DESKTOP_NAME;
use uq_core::history::{AutoReason, Entry, PointKind, RestorePoint, diff, revert_changed};
use uq_core::profile::holds;
use uq_core::schedule::Schedule;
use uq_core::settings::provenance;
use uq_core::settings::renders_section;
use uq_core::startup_explain::FileReport;
use uq_core::state::State;
use uq_core::test_bus::{Bus, connect, private_bus};
use zbus::object_server::SignalEmitter;

mod autostart;
mod binds;
mod compat;
mod devices;
mod explain;
mod history;
mod monitors;
mod profiles;
mod themes;
mod windows;
pub use autostart::FakeAutostart;
pub use binds::FakeBinds;
pub use devices::Displays;
pub use profiles::FakeProfiles;
pub use themes::FakeThemes;
pub use windows::FakeWindows;
mod rig;
pub use rig::*;
mod journal;
pub use journal::*;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.Desktop1.Error")]
pub enum FakeError {
    #[zbus(error)]
    ZBus(zbus::Error),
    InvalidSetting(String),
    InvalidArgument(String),
    ConfigInvalid(String),
    Io(String),
    Conflict(String),
    UnknownTheme(String),
}

fn invalid(e: impl ToString) -> FakeError {
    FakeError::InvalidSetting(e.to_string())
}

pub struct FakeUqd {
    pub state: Arc<Mutex<State>>,
    /// When set, the next `SetSection` of a domain that renders config fails with
    /// `ConfigInvalid`, as uqd does when Hyprland rejects the generated config.
    pub verify_error: Arc<Mutex<Option<String>>>,
    /// When set, the next `SetSection` fails with `InvalidSetting` and this text, as uqd's
    /// checks of the files outside its state refuse (F32).
    pub refuse_next: Arc<Mutex<Option<String>>>,
    pub explain: Explain,
    pub history: History,
    pub displays: Displays,
    pub profiles: FakeProfiles,
    pub windows: FakeWindows,
    pub autostart: FakeAutostart,
    pub themes: FakeThemes,
    pub binds: FakeBinds,
    /// What uqd's file sync found, for `Explain("env")` and `Explain("autostart")`.
    pub files: Arc<Mutex<FileReport>>,
}

#[zbus::interface(name = "org.uq.Desktop1")]
impl FakeUqd {
    fn get_theme(&self) -> (String, String) {
        let s = self.state.lock().unwrap();
        (s.theme.clone(), s.variant.to_string())
    }

    // Writes take uqd's last `origin` argument, and journal what they change under it.
    fn set_theme(&self, name: &str, variant: &str, origin: &str) -> Result<(), FakeError> {
        let variant = variant.parse().map_err(invalid)?;
        self.write(origin, |s| {
            s.theme = name.to_string();
            s.variant = variant;
            Ok(())
        })?;
        Ok(())
    }

    fn list_themes(&self) -> Vec<String> {
        self.themes.ids()
    }

    fn list_theme_info(&self) -> String {
        let active = self.state.lock().unwrap().theme.clone();
        self.themes.info(&active)
    }

    fn export_theme(&self, id: &str, with_look: bool) -> Result<String, FakeError> {
        self.themes.export(self, id, with_look)
    }

    async fn import_theme(
        &self,
        text: &str,
        replace: bool,
        with_look: bool,
        origin: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let id = self.themes.import(self, text, replace, with_look, origin)?;
        let _ = Self::themes_changed(&e).await;
        Ok(id)
    }

    async fn delete_theme(
        &self,
        id: &str,
        origin: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let report = self.themes.delete(self, id, origin)?;
        let _ = Self::themes_changed(&e).await;
        Ok(report)
    }

    #[zbus(signal)]
    async fn themes_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

    fn get_schedule(&self) -> String {
        serde_json::to_string(&self.state.lock().unwrap().schedule).unwrap()
    }

    fn set_schedule(&self, json: &str, origin: &str) -> Result<(), FakeError> {
        let schedule: Schedule = serde_json::from_str(json).map_err(invalid)?;
        schedule.validate().map_err(invalid)?;
        self.write(origin, |s| {
            s.schedule = schedule;
            Ok(())
        })?;
        Ok(())
    }

    async fn get_settings(&self) -> String {
        let read = serde_json::to_string(&self.state.lock().unwrap().settings).unwrap();
        // Slow, like `get_section`, so an unserialised read-modify-write of pins would lose one.
        tokio::time::sleep(Duration::from_millis(30)).await;
        read
    }

    fn set_setting(&self, key: &str, json: &str, origin: &str) -> Result<(), FakeError> {
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        self.write(origin, |s| s.settings.apply(key, value).map_err(invalid))?;
        Ok(())
    }

    async fn get_section(&self, domain: &str) -> Result<String, FakeError> {
        let read = self
            .state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        // A slow read, so an unserialised read-modify-write would lose an update.
        tokio::time::sleep(Duration::from_millis(30)).await;
        Ok(read.to_string())
    }

    fn set_section(&self, domain: &str, json: &str, origin: &str) -> Result<(), FakeError> {
        if domain == "monitors" {
            return self.set_monitors_section(json, origin);
        }
        if let Some(text) = self.refuse_next.lock().unwrap().take() {
            return Err(FakeError::InvalidSetting(text));
        }
        let value =
            serde_json::from_str(json).map_err(|e| invalid(format!("value is not JSON: {e}")))?;
        let done = self.write(origin, |next| {
            next.set_section(domain, value).map_err(invalid)?;
            if renders_section(domain)
                && let Some(message) = self.verify_error.lock().unwrap().take()
            {
                return Err(FakeError::ConfigInvalid(format!(
                    "Hyprland rejected the new config: {message}"
                )));
            }
            Ok(())
        });
        if domain == "rules" {
            match &done {
                Ok(_) => {}
                Err(FakeError::ConfigInvalid(m)) => {
                    *self.explain.rules_rejected.lock().unwrap() = Some(m.clone());
                }
                Err(_) => {}
            }
        }
        if domain == "keybinds"
            && let Err(FakeError::ConfigInvalid(m)) = &done
        {
            *self.explain.keybinds_rejected.lock().unwrap() = Some(m.clone());
        }
        if matches!(domain, "env" | "compat" | "autostart") {
            let mut rejected = self.explain.startup_rejected.lock().unwrap();
            match &done {
                Ok(_) if rejected.as_ref().is_some_and(|(d, _)| d == domain) => *rejected = None,
                Err(FakeError::ConfigInvalid(m)) => *rejected = Some((domain.into(), m.clone())),
                _ => {}
            }
        }
        done?;
        Ok(())
    }

    fn list_outputs(&self) -> String {
        let stored = self.state.lock().unwrap().settings.monitors.clone();
        self.displays.list(&stored)
    }

    async fn apply_monitors(
        &self,
        json: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let stored = self.state.lock().unwrap().settings.monitors.clone();
        let token = self.displays.apply(json, &stored)?;
        let _ = Self::monitors_changed(&e).await;
        Ok(token)
    }

    async fn remember_monitors(
        &self,
        name: &str,
        origin: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let id = self.remember(name, origin)?;
        let _ = Self::monitors_changed(&e).await;
        Ok(id)
    }

    async fn confirm_monitors(
        &self,
        token: &str,
        origin: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        let candidate = self.displays.take(token)?;
        self.write(origin, |s| {
            s.settings.monitors = candidate;
            Ok(())
        })?;
        let _ = Self::monitors_changed(&e).await;
        Ok(())
    }

    async fn revert_monitors(
        &self,
        token: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        self.displays.take(token)?;
        let _ = Self::monitors_changed(&e).await;
        Ok(())
    }

    fn list_windows(&self) -> String {
        self.windows.list()
    }

    fn list_integration_rules(&self) -> String {
        let on = self.state.lock().unwrap().integration_rules.enabled.clone();
        self.windows.list_app_rules(&on)
    }

    fn pick_window(&self) -> Result<String, FakeError> {
        self.windows.pick()
    }

    fn list_binds(&self) -> String {
        let kb = self.state.lock().unwrap().keybinds.clone();
        self.binds.list(&kb)
    }

    fn pause_shortcuts(&self) -> Result<String, FakeError> {
        self.binds.pause()
    }

    fn resume_shortcuts(&self, token: &str) -> Result<(), FakeError> {
        self.binds.resume(token)
    }

    fn list_input_devices(&self) -> String {
        devices::input_devices()
    }

    #[zbus(signal)]
    async fn monitors_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

    fn list_history(&self, limit: u32) -> String {
        let entries = self.history.entries.lock().unwrap();
        let newest_first: Vec<&Entry> = entries.iter().rev().take(limit as usize).collect();
        serde_json::to_string(&newest_first).unwrap()
    }

    /// uqd's rules: `Conflict` when a newer entry changed the same domain, unless `force`; only
    /// the keys the entry changed go back; the undo is a write with origin `restore`; returns the last id it journalled ("" if none).
    fn undo(&self, entry_id: &str, force: bool) -> Result<String, FakeError> {
        self.undo_now(entry_id, force)
    }

    fn list_restore_points(&self) -> String {
        let points = self.history.points.lock().unwrap();
        let newest_first: Vec<&RestorePoint> = points.iter().rev().map(|(p, _)| p).collect();
        serde_json::to_string(&newest_first).unwrap()
    }

    fn save_restore_point(&self, name: &str) -> String {
        let state = self.state.lock().unwrap().clone();
        self.history.add_point(name, PointKind::Manual, state)
    }

    fn delete_restore_point(&self, id: &str) {
        self.history
            .points
            .lock()
            .unwrap()
            .retain(|(p, _)| p.id != id);
    }

    fn restore(&self, id: &str) -> Result<(), FakeError> {
        self.restore_now(id)
    }

    fn explain(&self, domain: &str) -> Result<String, FakeError> {
        self.explain_now(domain)
    }

    fn reapply(&self, domain: &str) -> Result<(), FakeError> {
        self.state
            .lock()
            .unwrap()
            .section(domain)
            .map_err(invalid)?;
        self.explain
            .reapplied
            .lock()
            .unwrap()
            .push(domain.to_string());
        Ok(())
    }

    fn list_autostart(&self) -> Result<String, FakeError> {
        let state = self.state.lock().unwrap().autostart.clone();
        self.autostart.list(&state)
    }

    fn run_autostart(&self, id: &str) -> Result<(), FakeError> {
        self.autostart
            .actions
            .lock()
            .unwrap()
            .push(format!("run:{id}"));
        Ok(())
    }

    fn stop_autostart(&self, id: &str) -> Result<(), FakeError> {
        self.autostart
            .actions
            .lock()
            .unwrap()
            .push(format!("stop:{id}"));
        Ok(())
    }

    fn shell_ready(&self) {
        self.autostart.shell_ready.fetch_add(1, Ordering::SeqCst);
    }

    fn list_profiles(&self) -> String {
        self.profiles.list()
    }

    fn get_profile(&self, id: &str) -> Result<String, FakeError> {
        self.profiles.get(id)
    }

    async fn set_profile(
        &self,
        toml: &str,
        replace: bool,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let id = self.profiles.set(toml, replace, &self.list_themes())?;
        let _ = Self::profile_changed(&e, &self.profiles.active_id()).await;
        Ok(id)
    }

    async fn delete_profile(
        &self,
        id: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<(), FakeError> {
        self.profiles.delete(id)?;
        let _ = Self::profile_changed(&e, &self.profiles.active_id()).await;
        Ok(())
    }

    async fn apply_profile(
        &self,
        id: &str,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let report = self.profiles.apply(self, id)?;
        let _ = Self::profile_changed(&e, id).await;
        Ok(report)
    }

    async fn profile_off(
        &self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> Result<String, FakeError> {
        let report = self.profiles.off(self)?;
        let _ = Self::profile_changed(&e, "").await;
        Ok(report)
    }

    #[zbus(signal)]
    async fn profile_changed(e: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;

    fn clear_launch_counts(&self) {
        self.state.lock().unwrap().launch_counts.clear();
    }

    fn get_welcome(&self) -> String {
        self.state.lock().unwrap().welcome.status_json().to_string()
    }

    fn set_welcome_done(&self, version: u32) -> Result<(), FakeError> {
        if version == 0 || version > uq_core::welcome::CURRENT_WELCOME_VERSION {
            return Err(FakeError::InvalidArgument(format!("bad version {version}")));
        }
        self.state.lock().unwrap().welcome = uq_core::welcome::Welcome {
            done: true,
            version,
        };
        Ok(())
    }

    fn reset_welcome(&self) {
        self.state.lock().unwrap().welcome = uq_core::welcome::Welcome::default();
    }

    fn get_launch_counts(&self) -> HashMap<String, u32> {
        let s = self.state.lock().unwrap();
        s.launch_counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}
