//! Desktop state and the commit sequence (spec §4, §5). No D-Bus here.

use crate::compositor::{Compositor, CompositorError};
use crate::power::{NoPowerProfiles, POWER_TIMEOUT, PowerError, PowerProfiles};
use crate::system::SystemSettings;
use chrono::{DateTime, Local, TimeZone, Utc};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uq_core::atomic::{atomic_replace_dir, atomic_write};
use uq_core::history::{
    AutoReason, Entry, Journal, MAX_ENTRIES, Origin, PointKind, RestorePoint, RestoreStore, diff,
    revert_changed,
};
use uq_core::lua::{
    Layout, StartupRender, emit_generated_with_apps, emit_workspace_rule, lua_string,
    monitor_render, startup_render,
};
use uq_core::paths::Paths;
use uq_core::profile::{ActiveProfile, ProfileStore, holds, load_active};
use uq_core::render::{Motion, RenderedFiles, kvantum_base, render_with};
use uq_core::schedule::{Schedule, last_event, next_event, variant_at};
use uq_core::settings::{
    SECTION_DOMAINS, Settings, Strategy, plan, provenance, renders_config, renders_section,
    set_rule, valid_pinned_id,
};
use uq_core::startup_explain::FileReport;
use uq_core::state::State;
use uq_core::theme::{ThemeRegistry, Variant};
mod launch;
pub use launch::*;
mod commit;
mod explain;
mod history;
mod schedule;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ThemeChanged {
        name: String,
        variant: Variant,
        tokens_path: PathBuf,
    },
    LayoutChanged {
        workspace: String,
        layout: Layout,
    },
    ConfigError(String),
    /// The full settings (never the launch counts).
    SettingsChanged(Box<Settings>),
    LaunchCountsChanged(BTreeMap<String, u32>),
    /// One whole settings section (`appearance`, `wallpaper`) changed; `json` is its new content.
    SectionChanged {
        domain: String,
        json: String,
    },
    /// The journal gained entries.
    HistoryChanged,
    /// A display change was applied, confirmed or reverted: `ListOutputs` has the new picture.
    MonitorsChanged,
    /// The active profile or the list of profiles changed; the active profile's id, or `""`.
    ProfileChanged(String),
    /// A window pick started; the token goes to the shell, which answers `PickDone`.
    PickRequested(String),
    /// A startup app's status changed: `ListAutostart` has the new picture.
    AutostartChanged,
    /// The user's themes changed: `ListThemeInfo` has the new picture.
    ThemesChanged,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DesktopError {
    #[error("That theme isn't installed any more.")]
    UnknownTheme(String),
    #[error("unknown layout {0:?} (expected dwindle, master or scrolling)")]
    InvalidLayout(String),
    #[error("workspace name is empty")]
    EmptyWorkspace,
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("Hyprland rejected the new config: {0}")]
    ConfigInvalid(String),
    #[error("{0}")]
    InvalidSetting(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    InvalidArgument(String),
    /// An undo whose domain changed again since; the caller may force it.
    #[error("{0}")]
    Conflict(String),
}

pub(crate) fn io_err(context: &str) -> impl Fn(std::io::Error) -> DesktopError + '_ {
    move |e| DesktopError::Io(format!("{context}: {e}"))
}

/// What a commit changed, which decides how it is applied to the live session.
pub(crate) enum Change {
    Theme,
    /// A setting rendered into generated.lua (mouse): reload Hyprland, report SettingsChanged.
    Input,
    Layout(String, Layout),
    /// A `look` or `layouts` section write: reload Hyprland, send no events (`set_section`
    /// sends `SectionChanged` and `SettingsChanged` itself).
    Reload,
    Quiet,
}

/// Where the system's Kvantum base themes live.
pub const KVANTUM_DIR: &str = "/usr/share/Kvantum";

pub struct Desktop {
    pub(crate) paths: Paths,
    pub(crate) themes: ThemeRegistry,
    pub(crate) state: State,
    pub(crate) comp: Arc<dyn Compositor>,
    sys: Arc<dyn SystemSettings>,
    pub(crate) events: broadcast::Sender<Event>,
    kvantum_dir: PathBuf,
    /// When launch counts were last written; `None` until the first save.
    last_launch_save: Option<Instant>,
    launch_dirty: bool,
    launch_saves: u32,
    /// After a failed launch-count write: not before this again.
    launch_retry_at: Option<Instant>,
    journal: Journal,
    pub(crate) restore: RestoreStore,
    /// Ids of the journal entries the latest `persist` appended; empty when it journalled nothing.
    last_entry_ids: Vec<String>,
    /// `(domain, Hyprland's message)` of the last section write that failed verification.
    pub(crate) last_rejected: Option<(String, String)>,
    /// The display change waiting for `ConfirmMonitors`, if any.
    pub(crate) pending: Option<crate::monitors::Pending>,
    /// How long an unconfirmed display change stays ([`crate::monitors::REVERT_AFTER`]).
    pub(crate) revert_after: Duration,
    pub(crate) power: Arc<dyn PowerProfiles>,
    /// How long a power call may take ([`POWER_TIMEOUT`]).
    pub(crate) power_timeout: Duration,
    pub(crate) profiles: ProfileStore,
    /// The profile that is on, and what it replaced; kept in `paths.profile_active`.
    pub(crate) active: Option<ActiveProfile>,
    /// A `uq-dev session` (`UQ_DEV_SESSION`, R20).
    pub(crate) dev_session: bool,
    /// The main screen's scale the installed config was rendered with (R13).
    pub(crate) main_scale: Option<f64>,
    /// What the file sync last found (`crate::startup`).
    pub(crate) files: FileReport,
    /// What the user's themes folder looked like when it was last read (`crate::themes`).
    pub(crate) user_themes: uq_core::theme::user::Fingerprint,
    /// The theme folders the last scan left out, and why.
    pub(crate) skipped_themes: Vec<uq_core::theme::user::Skipped>,
    /// Says things to the user on its own (a skipped display arrangement); silent until `main`.
    pub(crate) notifier: Arc<dyn crate::notify::Notifier>,
    /// Tells X11 apps which display is the main one; does nothing until `main` gives it a binary.
    pub(crate) xrandr: Arc<dyn crate::xrandr::Xrandr>,
    /// What the screen-change matcher remembers between events.
    pub(crate) hotplug: crate::hotplug::HotplugState,
    /// When uqd last reloaded Hyprland to recover from a file missing for a moment.
    pub(crate) recover: crate::config_recover::RecoverState,
}

/// How a section's write is applied live: `appearance` re-themes, `look` and `layouts` reload
/// Hyprland, and the shell reads the rest itself.
pub(crate) fn change_for(domain: &str) -> Change {
    match domain {
        "appearance" => Change::Theme,
        d if renders_section(d) => Change::Reload,
        _ => Change::Quiet,
    }
}

/// A rendered, verified candidate config, left on disk until `install` writes it.
pub(crate) struct Candidate {
    pub(crate) files: RenderedFiles,
    /// The candidate's text; Task 5's `apply_monitors` diffs and evals from it.
    #[allow(dead_code)]
    pub(crate) generated: String,
    pub(crate) path: PathBuf,
    /// The theme resolved for `next`, so `install` needs no second lookup.
    pub(crate) theme: uq_core::theme::Theme,
    /// The main screen's scale `generated` was rendered with (F4).
    pub(crate) main_scale: Option<f64>,
}

impl Desktop {
    pub fn new(
        paths: Paths,
        comp: Arc<dyn Compositor>,
        sys: Arc<dyn SystemSettings>,
    ) -> Result<Self, DesktopError> {
        let themes = ThemeRegistry::load_dir(&paths.themes_dir)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        let state = State::load(&paths.state_file);
        let (events, _) = broadcast::channel(64);
        let journal = Journal::open(&paths.history_dir);
        let restore = RestoreStore::open(&paths.restore_dir);
        let profiles = ProfileStore::open(paths.profiles_dir.clone());
        let active = load_active(&paths.profile_active);
        let mut desktop = Desktop {
            paths,
            themes,
            state,
            comp,
            sys,
            events,
            kvantum_dir: PathBuf::from(KVANTUM_DIR),
            last_launch_save: None,
            launch_dirty: false,
            launch_saves: 0,
            launch_retry_at: None,
            journal,
            restore,
            last_entry_ids: Vec::new(),
            last_rejected: None,
            pending: None,
            revert_after: crate::monitors::REVERT_AFTER,
            power: Arc::new(NoPowerProfiles),
            power_timeout: POWER_TIMEOUT,
            profiles,
            active,
            dev_session: false,
            main_scale: None,
            files: FileReport::default(),
            user_themes: Vec::new(),
            skipped_themes: Vec::new(),
            notifier: Arc::new(crate::notify::NoNotifier),
            xrandr: Arc::new(crate::xrandr::NoXrandr),
            hotplug: Default::default(),
            recover: Default::default(),
        };
        // The user's themes first, so a saved `user-…` theme survives a restart.
        desktop.load_user_themes();
        if desktop
            .themes
            .get(&desktop.state.theme, desktop.state.variant)
            .is_none()
        {
            tracing::warn!(
                "[THEME-001] theme {:?} not installed; using {}",
                desktop.state.theme,
                State::default().theme
            );
            desktop.state.theme = State::default().theme;
        }
        Ok(desktop)
    }

    /// Say things through `n` instead of nowhere (the session bus in `main`, a fake in tests).
    pub fn with_notifier(mut self, n: Arc<dyn crate::notify::Notifier>) -> Self {
        self.notifier = n;
        self
    }

    /// Set the notifier once the session connection exists (`main`, after `serve_with`).
    pub fn set_notifier(&mut self, n: Arc<dyn crate::notify::Notifier>) {
        self.notifier = n;
    }

    /// Set the X11 main display through `x` instead of not at all.
    pub fn with_xrandr(mut self, x: Arc<dyn crate::xrandr::Xrandr>) -> Self {
        self.xrandr = x;
        self
    }

    /// [`with_xrandr`](Self::with_xrandr) for a desktop that is already shared.
    pub fn set_xrandr(&mut self, x: Arc<dyn crate::xrandr::Xrandr>) {
        self.xrandr = x;
    }

    /// Use `p` for power modes instead of none (the system's service in `main`, a fake in tests).
    pub fn with_power(mut self, p: Arc<dyn PowerProfiles>) -> Self {
        self.power = p;
        self
    }

    /// Wait at most `d` for a power call instead of [`POWER_TIMEOUT`] (tests).
    pub fn with_power_timeout(mut self, d: Duration) -> Self {
        self.power_timeout = d;
        self
    }

    /// The current power mode; `Timeout` when the power service doesn't answer in time.
    pub(crate) async fn power_active(&self) -> Result<String, PowerError> {
        tokio::time::timeout(self.power_timeout, self.power.active())
            .await
            .unwrap_or(Err(PowerError::Timeout))
    }

    /// Set the power mode; `Timeout` when the power service doesn't answer in time.
    pub(crate) async fn power_set(&self, mode: &str) -> Result<(), PowerError> {
        tokio::time::timeout(self.power_timeout, self.power.set_active(mode))
            .await
            .unwrap_or(Err(PowerError::Timeout))
    }

    /// Look for Kvantum base themes in `dir` instead of `/usr/share/Kvantum` (tests).
    pub fn with_kvantum_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.kvantum_dir = dir.into();
        self
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn theme_ids(&self) -> Vec<String> {
        self.themes.ids().into_iter().map(str::to_string).collect()
    }

    pub fn get_theme(&self) -> (String, Variant) {
        (self.state.theme.clone(), self.state.variant)
    }

    pub fn get_layouts(&self) -> BTreeMap<String, Layout> {
        self.state.layouts.clone()
    }

    pub fn schedule(&self) -> &Schedule {
        &self.state.schedule
    }

    pub fn settings(&self) -> &Settings {
        &self.state.settings
    }

    /// Validate and store one setting. Most keys take the light commit path (spec §2.4): save
    /// `state.toml`, emit, no render, no verify. Keys that live in Hyprland's config (mouse)
    /// render, verify and reload like a theme change.
    pub async fn set_setting(
        &mut self,
        key: &str,
        value: serde_json::Value,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        self.set_settings(vec![(key.to_string(), value)], origin)
            .await
    }

    /// [`Self::set_setting`] for several keys as one write. Keys are applied in whatever order
    /// passes validation, so an undo can move idle stages past each other.
    async fn set_settings(
        &mut self,
        values: Vec<(String, serde_json::Value)>,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let renders = values.iter().any(|(k, _)| renders_config(k));
        let mut next = self.state.clone();
        let mut pending = values;
        while !pending.is_empty() {
            let mut err = None;
            let left = pending.len();
            pending.retain(|(k, v)| match next.settings.apply(k, v.clone()) {
                Ok(()) => false,
                Err(e) => {
                    err = Some(e);
                    true
                }
            });
            if let Some(e) = err.filter(|_| pending.len() == left) {
                return Err(DesktopError::InvalidSetting(e.to_string()));
            }
        }
        if renders {
            return self.commit(next, Change::Input, origin).await;
        }
        self.persist(next, origin)?;
        let _ = self.events.send(Event::SettingsChanged(Box::new(
            self.state.settings.clone(),
        )));
        Ok(())
    }

    /// Ensure `user.lua` exists (never overwriting it), then render everything from state.
    pub async fn startup(&mut self) {
        // A restart in the middle of a key capture would leave Hyprland in the empty
        // `uq_capture` submap, with every shortcut off and no token to resume. Always leave it.
        if let Err(e) = self
            .comp
            .eval("hl.dispatch(hl.dsp.submap(\"reset\"))")
            .await
        {
            tracing::debug!("leaving the capture submap at start-up: {e}");
        }
        if let Err(e) = self.ensure_user_lua() {
            tracing::warn!("[START-004] {e}");
        }
        self.save_new_version_point();
        // A display change left unconfirmed by a crash: the stored config never changed, so
        // the reload below puts the old layout back.
        let crashed_mid_apply = self.clear_stale_monitor_marker();
        if let Err(e) = self
            .commit(self.state.clone(), Change::Theme, Origin::Schedule)
            .await
        {
            tracing::warn!("[THEME-004] startup render failed: {e}");
        }
        if crashed_mid_apply {
            let _ = self.events.send(Event::MonitorsChanged);
        }
        self.sync_all_files();
    }

    pub async fn set_theme(
        &mut self,
        id: &str,
        v: Variant,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        if self.themes.get(id, v).is_none() {
            return Err(DesktopError::UnknownTheme(id.to_string()));
        }
        let mut next = self.state.clone();
        next.theme = id.to_string();
        next.variant = v;
        self.commit(next, Change::Theme, origin).await
    }

    pub async fn set_layout(
        &mut self,
        ws: &str,
        layout: &str,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        if ws.trim().is_empty() {
            return Err(DesktopError::EmptyWorkspace);
        }
        let layout: Layout = layout
            .parse()
            .map_err(|_| DesktopError::InvalidLayout(layout.to_string()))?;
        let mut next = self.state.clone();
        // The `layouts` section's limits, so what SetLayout stores SetSection accepts back.
        let ws = set_rule(&mut next.layouts, ws, layout)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        self.commit(next, Change::Layout(ws, layout), origin).await
    }

    /// Re-render every file from current state and re-theme the session.
    pub async fn reload(&mut self) -> Result<(), DesktopError> {
        self.commit(self.state.clone(), Change::Theme, Origin::Unknown)
            .await
    }

    /// Run `domain`'s normal render path again on the current state. Nothing changes, so
    /// nothing is journalled. Never refused for a known domain.
    pub async fn reapply(&mut self, domain: &str) -> Result<(), DesktopError> {
        if !SECTION_DOMAINS.contains(&domain) {
            return Err(DesktopError::InvalidArgument(format!(
                "unknown domain {domain:?}"
            )));
        }
        let res = self
            .commit(self.state.clone(), change_for(domain), Origin::Settings)
            .await;
        self.note_section_commit(domain, &res);
        res?;
        let json = self.get_section(domain)?;
        let _ = self.events.send(Event::SectionChanged {
            domain: domain.to_string(),
            json,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
