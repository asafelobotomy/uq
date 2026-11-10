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
use uq_core::lua::{Layout, emit_generated, emit_workspace_rule, lua_string};
use uq_core::paths::Paths;
use uq_core::profile::{ActiveProfile, ProfileStore, holds, load_active};
use uq_core::render::{RenderedFiles, kvantum_base, render};
use uq_core::schedule::{Schedule, last_event, variant_at};
use uq_core::settings::{
    SECTION_DOMAINS, Settings, provenance, renders_config, renders_section, set_rule,
    valid_pinned_id,
};
use uq_core::state::State;
use uq_core::theme::{ThemeRegistry, Variant};

const USER_LUA_STUB: &str =
    "-- Your own Hyprland settings. uQ reads this file last and never rewrites it.\n";

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
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DesktopError {
    #[error("unknown theme {0:?}")]
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
    journal: Journal,
    pub(crate) restore: RestoreStore,
    /// Ids of the journal entries the latest `persist` appended; empty when it journalled nothing.
    last_entry_ids: Vec<String>,
    /// `(domain, Hyprland's message)` of the last section write that failed verification.
    last_rejected: Option<(String, String)>,
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
}

/// `explain`'s dotted key as a profile path.
fn json_path(key: &str) -> Vec<String> {
    key.split('.').map(str::to_string).collect()
}

/// How much of `user.lua` `explain` scans.
const USER_LUA_SCAN_LIMIT: u64 = 1 << 20;

/// How a section's write is applied live: `appearance` re-themes, `look` and `layouts` reload
/// Hyprland, and the shell reads the rest itself.
pub(crate) fn change_for(domain: &str) -> Change {
    match domain {
        "appearance" => Change::Theme,
        d if renders_section(d) => Change::Reload,
        _ => Change::Quiet,
    }
}

/// How a conflict message names a journal domain.
fn domain_label(domain: &str) -> &'static str {
    match domain {
        "theme" => "The theme",
        "schedule" => "The theme schedule",
        "appearance" => "Appearance",
        "look" => "Look",
        "layouts" => "Layouts",
        "wallpaper" => "Wallpaper",
        "shell" => "Shell",
        "settings" => "A setting",
        _ => "This setting",
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
}

/// Launch counts stop here per app.
const MAX_LAUNCH_COUNT: u32 = 10_000;
/// Distinct apps tracked; further new ids are ignored.
const MAX_LAUNCH_IDS: usize = 2048;
/// Launch counts hit the disk at most this often.
const LAUNCH_SAVE_INTERVAL: Duration = Duration::from_secs(30);

/// Whether `<id>` (or `<id>.desktop`) exists under an `applications/` folder of `data_dirs`.
pub fn desktop_file_installed(id: &str, data_dirs: &[PathBuf]) -> bool {
    if !valid_pinned_id(id) {
        return false;
    }
    let file = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };
    data_dirs
        .iter()
        .any(|d| d.join("applications").join(&file).is_file())
}

/// `$XDG_DATA_HOME` (default `~/.local/share`) followed by `$XDG_DATA_DIRS`
/// (default `/usr/local/share:/usr/share`).
pub fn xdg_data_dirs() -> Vec<PathBuf> {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let mut dirs = Vec::new();
    if let Some(h) = var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(h));
    } else if let Some(h) = var("HOME") {
        dirs.push(PathBuf::from(h).join(".local/share"));
    }
    let rest = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(rest.split(':').filter(|p| !p.is_empty()).map(PathBuf::from));
    dirs
}

impl Desktop {
    pub fn new(
        paths: Paths,
        comp: Arc<dyn Compositor>,
        sys: Arc<dyn SystemSettings>,
    ) -> Result<Self, DesktopError> {
        let themes = ThemeRegistry::load_dir(&paths.themes_dir)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        let mut state = State::load(&paths.state_file);
        if themes.get(&state.theme, state.variant).is_none() {
            tracing::warn!(
                "theme {:?} not installed; using {}",
                state.theme,
                State::default().theme
            );
            state.theme = State::default().theme;
        }
        let (events, _) = broadcast::channel(64);
        let journal = Journal::open(&paths.history_dir);
        let restore = RestoreStore::open(&paths.restore_dir);
        let profiles = ProfileStore::open(paths.profiles_dir.clone());
        let active = load_active(&paths.profile_active);
        Ok(Desktop {
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
        })
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

    /// Ids of the entries the latest `persist` journalled (empty if none).
    pub(crate) fn last_entry_ids(&self) -> &[String] {
        &self.last_entry_ids
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

    pub fn get_launch_counts(&self) -> BTreeMap<String, u32> {
        self.state.launch_counts.clone()
    }

    /// How many times launch counts have been written (for throttle tests).
    pub fn launch_saves(&self) -> u32 {
        self.launch_saves
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

    pub fn record_launch(&mut self, id: &str) {
        self.record_launch_at(id, Instant::now());
    }

    /// Count a launch. Memory and subscribers update at once; the disk write is throttled.
    pub fn record_launch_at(&mut self, id: &str, now: Instant) {
        if !self.state.settings.privacy.record_launches || !valid_pinned_id(id) {
            return;
        }
        let counts = &mut self.state.launch_counts;
        if !counts.contains_key(id) && counts.len() >= MAX_LAUNCH_IDS {
            return;
        }
        let n = counts.entry(id.to_string()).or_insert(0);
        *n = (*n + 1).min(MAX_LAUNCH_COUNT);
        self.launch_dirty = true;
        self.flush_launch_counts_at(now);
        let _ = self
            .events
            .send(Event::LaunchCountsChanged(self.state.launch_counts.clone()));
    }

    /// Write dirty launch counts if the last write is at least 30 s old. Called on the tick.
    pub fn flush_launch_counts_at(&mut self, now: Instant) {
        if !self.launch_dirty
            || self
                .last_launch_save
                .is_some_and(|t| now.saturating_duration_since(t) < LAUNCH_SAVE_INTERVAL)
        {
            return;
        }
        self.save_launch_counts(now);
    }

    pub fn flush_launch_counts(&mut self) {
        self.flush_launch_counts_at(Instant::now());
    }

    /// Writes `state.toml` outside `persist` on purpose: launch counts are never journalled.
    fn save_launch_counts(&mut self, now: Instant) {
        match self.state.save(&self.paths.state_file) {
            Ok(()) => {
                self.launch_dirty = false;
                self.last_launch_save = Some(now);
                self.launch_saves += 1;
            }
            Err(e) => {
                // Stay dirty; the next tick retries.
                tracing::warn!("saving launch counts: {e}");
            }
        }
    }

    pub fn clear_launch_counts(&mut self) {
        self.state.launch_counts.clear();
        self.save_launch_counts(Instant::now());
        let _ = self
            .events
            .send(Event::LaunchCountsChanged(BTreeMap::new()));
    }

    /// Drop counts for apps that are no longer installed. Run at startup.
    pub fn prune_launch_counts(&mut self, installed: &dyn Fn(&str) -> bool) {
        let before = self.state.launch_counts.len();
        self.state.launch_counts.retain(|id, _| installed(id));
        if self.state.launch_counts.len() != before {
            self.save_launch_counts(Instant::now());
            let _ = self
                .events
                .send(Event::LaunchCountsChanged(self.state.launch_counts.clone()));
        }
    }

    /// Ensure `user.lua` exists (never overwriting it), then render everything from state.
    pub async fn startup(&mut self) {
        if let Err(e) = self.ensure_user_lua() {
            tracing::warn!("{e}");
        }
        self.save_new_version_point();
        // A display change left unconfirmed by a crash: the stored config never changed, so
        // the reload below puts the old layout back.
        let crashed_mid_apply = self.clear_stale_monitor_marker();
        if let Err(e) = self
            .commit(self.state.clone(), Change::Theme, Origin::Schedule)
            .await
        {
            tracing::warn!("startup render failed: {e}");
        }
        if crashed_mid_apply {
            let _ = self.events.send(Event::MonitorsChanged);
        }
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

    pub async fn set_schedule(&mut self, s: Schedule, origin: Origin) -> Result<(), DesktopError> {
        self.set_schedule_at(s, Local::now(), origin).await
    }

    /// Store a schedule and switch to the variant it wants at `now`.
    pub async fn set_schedule_at<Tz: TimeZone>(
        &mut self,
        s: Schedule,
        now: DateTime<Tz>,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        s.validate().map_err(DesktopError::InvalidSchedule)?;
        let mut next = self.state.clone();
        // While the active profile sets the variant, the schedule waits: no mark, so the first
        // tick after Back to normal applies it.
        let held = self.profile_holds_variant();
        let wanted = variant_at(&s, &now).filter(|_| !held);
        next.schedule_mark = last_event(&s, &now)
            .filter(|_| !held)
            .map(|(at, _)| at.with_timezone(&Utc));
        next.schedule = s;
        let change = match wanted {
            Some(v) if v != next.variant => {
                next.variant = v;
                Change::Theme
            }
            _ => Change::Quiet,
        };
        self.commit(next, change, origin).await
    }

    /// Apply a scheduled transition that is newer than the last one applied. Returns whether the
    /// variant switched. A manual variant change therefore holds until the next boundary.
    /// Called every minute, so suspend/resume and clock changes are caught on the next tick.
    /// While the active profile sets the variant nothing happens and the mark stays, so the first
    /// tick after Back to normal switches as usual.
    pub async fn tick_schedule_at<Tz: TimeZone>(
        &mut self,
        now: DateTime<Tz>,
    ) -> Result<bool, DesktopError> {
        if self.profile_holds_variant() {
            return Ok(false);
        }
        let Some((at, v)) = last_event(&self.state.schedule, &now) else {
            return Ok(false);
        };
        let at = at.with_timezone(&Utc);
        if self.state.schedule_mark.is_some_and(|mark| mark >= at) {
            return Ok(false);
        }
        let mut next = self.state.clone();
        next.schedule_mark = Some(at);
        if v == next.variant {
            self.commit(next, Change::Quiet, Origin::Schedule)
                .await
                .map(|()| false)
        } else {
            next.variant = v;
            self.commit(next, Change::Theme, Origin::Schedule)
                .await
                .map(|()| true)
        }
    }

    /// Re-render every file from current state and re-theme the session.
    pub async fn reload(&mut self) -> Result<(), DesktopError> {
        self.commit(self.state.clone(), Change::Theme, Origin::Unknown)
            .await
    }

    /// The journal, newest first, at most `limit` (capped at [`MAX_ENTRIES`]) entries.
    pub fn history(&self, limit: usize) -> Vec<Entry> {
        self.journal.list(limit.min(MAX_ENTRIES))
    }

    /// Write entry `id`'s `before` back through the normal write path, recorded with origin
    /// `restore`. Only the keys the entry changed are written back. Refused with `Conflict` when a newer entry changed the same domain, unless
    /// `force`. [`Self::last_entry_ids`] then holds what the undo itself journalled.
    pub async fn undo(&mut self, id: &str, force: bool) -> Result<(), DesktopError> {
        let entries = self.journal.list(usize::MAX);
        let Some(pos) = entries.iter().position(|e| e.id == id) else {
            return Err(DesktopError::InvalidArgument(format!(
                "no history entry {id:?}"
            )));
        };
        let entry = &entries[pos];
        if !force && entries[..pos].iter().any(|e| e.domain == entry.domain) {
            return Err(DesktopError::Conflict(format!(
                "{} was changed again after this. Undo anyway?",
                domain_label(&entry.domain)
            )));
        }
        let damaged = |e: serde_json::Error| {
            DesktopError::InvalidArgument(format!("history entry {id:?} is damaged: {e}"))
        };
        let before = entry.before.clone();
        match entry.domain.as_str() {
            "theme" => {
                let name: String =
                    serde_json::from_value(before["theme"].clone()).map_err(damaged)?;
                let variant: Variant =
                    serde_json::from_value(before["variant"].clone()).map_err(damaged)?;
                self.set_theme(&name, variant, Origin::Restore).await?;
            }
            "schedule" => {
                let schedule: Schedule = serde_json::from_value(before).map_err(damaged)?;
                self.set_schedule(schedule, Origin::Restore).await?;
            }
            "settings" => {
                let after = &entry.after;
                let values: Vec<(String, serde_json::Value)> = before
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(k, v)| after.get(k.as_str()) != Some(*v))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.set_settings(values, Origin::Restore).await?;
            }
            domain => {
                // Only the keys this entry changed go back; later changes to other keys of the
                // section stay.
                let domain = domain.to_string();
                let current = self
                    .state
                    .section(&domain)
                    .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
                let value = revert_changed(&current, &before, &entry.after);
                if domain == "monitors" {
                    // Displays never take SetSection; an undo goes back to a layout the user
                    // already kept, so it is saved and reloaded without a revert window.
                    let mut next = self.state.clone();
                    next.set_section(&domain, value)
                        .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
                    self.write_monitors(next, Origin::Restore).await?;
                } else {
                    self.set_section(&domain, &value.to_string(), Origin::Restore)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Where each key of `domain` gets its live value: `{key: {source, live, stored, detail}}`,
    /// keyed by the dotted path within the section. `source` is `theme`, `default`, `uq`,
    /// `profile` (a `uq` value the active profile set), `user_lua`, `runtime` or `error`.
    /// Without a live Hyprland every key reports from state.
    pub async fn explain(&self, domain: &str) -> Result<serde_json::Value, DesktopError> {
        use serde_json::{Value, json};
        if !SECTION_DOMAINS.contains(&domain) {
            return Err(DesktopError::InvalidArgument(format!(
                "unknown domain {domain:?}"
            )));
        }
        let section = self
            .state
            .section(domain)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        let theme = self.themes.get(&self.state.theme, self.state.variant);
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == domain)
            .map(|(_, m)| m.clone());
        let mut up = self.comp.config_errors().await.is_ok();
        let mut user_lua: Option<Option<String>> = None;
        let mut out = serde_json::Map::new();
        for row in provenance::options_for(domain) {
            let stored = row.stored(&section);
            let from_state = if stored.is_null() {
                row.null_source.as_str()
            } else {
                "uq"
            };
            let mut live = Value::Null;
            if up && let Some(path) = row.hypr {
                match self.comp.get_option(path).await {
                    Ok(v) => live = v,
                    Err(CompositorError::Rejected(e)) => tracing::warn!("getoption {path}: {e}"),
                    Err(e) => {
                        tracing::warn!("getoption {path}: {e}");
                        up = false;
                    }
                }
            } else if up && let Some((schema, key)) = row.gsettings {
                live = self.sys.read(schema, key).map_or(Value::Null, Value::from);
            }
            let expected = theme.and_then(|t| provenance::expected(row, &self.state, t));
            let (source, detail) = if let Some(message) = &rejected {
                ("error", json!({ "message": message }))
            } else if live.is_null()
                || expected
                    .as_ref()
                    .is_some_and(|e| provenance::live_matches(e, &live))
            {
                (from_state, Value::Null)
            } else {
                // A mismatch, or a value uQ leaves to base.lua: look for it in user.lua.
                let line = row.hypr.and_then(|path| {
                    let text = user_lua.get_or_insert_with(|| self.read_user_lua());
                    provenance::find_in_user_lua(text.as_deref()?, path)
                });
                match line {
                    Some(line) => (
                        "user_lua",
                        json!({ "line": line, "path": self.paths.user_lua.to_string_lossy() }),
                    ),
                    None if expected.is_none() => (from_state, Value::Null),
                    None => ("runtime", Value::Null),
                }
            };
            let (source, detail) = match &self.active {
                Some(a)
                    if source == "uq" && holds(a, domain, &json_path(row.json_key()), &stored) =>
                {
                    ("profile", json!({ "name": a.name }))
                }
                _ => (source, detail),
            };
            out.insert(
                row.json_key().to_string(),
                json!({ "source": source, "live": live, "stored": stored, "detail": detail }),
            );
        }
        Ok(Value::Object(out))
    }

    /// The first [`USER_LUA_SCAN_LIMIT`] bytes of `user.lua`, decoded lossily; `None` if unreadable.
    fn read_user_lua(&self) -> Option<String> {
        use std::io::Read;
        let mut buf = Vec::new();
        std::fs::File::open(&self.paths.user_lua)
            .and_then(|f| f.take(USER_LUA_SCAN_LIMIT).read_to_end(&mut buf))
            .ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
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

    /// Remember a section write Hyprland rejected, so `explain` reports it. Any later commit
    /// that installs a verified config clears it (see `commit`).
    pub(crate) fn note_section_commit(&mut self, domain: &str, res: &Result<(), DesktopError>) {
        if let Err(DesktopError::ConfigInvalid(msg)) = res {
            self.last_rejected = Some((domain.to_string(), msg.clone()));
        }
    }

    /// Restore points, newest first.
    pub fn restore_points(&self) -> Vec<RestorePoint> {
        self.restore.list()
    }

    /// Save the current state as a manual restore point; returns its id.
    pub fn save_restore_point(&self, name: &str) -> Result<String, DesktopError> {
        self.restore
            .save(&self.state, name, PointKind::Manual, Utc::now().timestamp())
            .map(|p| p.id)
            .map_err(io_err("saving restore point"))
    }

    pub fn delete_restore_point(&self, id: &str) -> Result<(), DesktopError> {
        self.restore.delete(id).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidInput => DesktopError::InvalidArgument(e.to_string()),
            _ => DesktopError::Io(format!("deleting restore point: {e}")),
        })
    }

    /// Bring back every domain of restore point `id` as one write, keeping today's launch
    /// counts. A damaged point or one whose theme is gone changes nothing; otherwise an
    /// automatic point of the current state is saved first. Returns plain notices (the power
    /// mode of a profile the restore ended).
    pub async fn restore(&mut self, id: &str) -> Result<Vec<String>, DesktopError> {
        let mut next = self.restore.load(id).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidData => {
                DesktopError::InvalidArgument("restore point is damaged".into())
            }
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::NotFound => {
                DesktopError::InvalidArgument(e.to_string())
            }
            _ => DesktopError::Io(format!("reading restore point: {e}")),
        })?;
        if self.themes.get(&next.theme, next.variant).is_none() {
            return Err(DesktopError::UnknownTheme(next.theme));
        }
        if let Err(e) = self.restore.save(
            &self.state,
            "Before restoring a restore point",
            PointKind::Auto(AutoReason::BeforeRestore),
            Utc::now().timestamp(),
        ) {
            tracing::warn!("saving before-restore point: {e}");
        }
        next.launch_counts = self.state.launch_counts.clone();
        self.commit(next, Change::Theme, Origin::Restore).await?;
        self.announce_all();
        // The restored settings aren't the profile's any more, and neither is its power mode,
        // which a restore point doesn't hold: put it back as "Back to normal" does (K2: a
        // failure is a notice, never an error).
        let mut notices = Vec::new();
        if let Some(a) = self.active.take() {
            if let Some(pm) = &a.power {
                self.power_back(pm, &mut Vec::new(), &mut notices).await;
            }
            self.forget_active();
        }
        Ok(notices)
    }

    /// After a `Change::Theme` commit, which announces only the theme: every section and the
    /// settings may have changed too.
    pub(crate) fn announce_all(&self) {
        for domain in SECTION_DOMAINS {
            if let Ok(json) = self.get_section(domain) {
                let _ = self.events.send(Event::SectionChanged {
                    domain: domain.to_string(),
                    json,
                });
            }
        }
        let _ = self.events.send(Event::SettingsChanged(Box::new(
            self.state.settings.clone(),
        )));
    }

    fn ensure_user_lua(&self) -> Result<(), DesktopError> {
        let p = &self.paths.user_lua;
        if p.exists() {
            return Ok(());
        }
        atomic_write(p, USER_LUA_STUB).map_err(io_err("creating user.lua"))
    }

    /// The one save point: write `state.toml`, adopt `next`, and record what changed. Journal and
    /// restore-point failures are logged and never fail the write.
    pub(crate) fn persist(&mut self, next: State, origin: Origin) -> Result<(), DesktopError> {
        next.save(&self.paths.state_file)
            .map_err(io_err("saving state"))?;
        let old = std::mem::replace(&mut self.state, next);
        // A schedule tick keeps the last entry ids, so it never moves the undo target.
        if origin == Origin::Schedule {
            return Ok(());
        }
        self.last_entry_ids.clear();
        let changes = diff(&old, &self.state, &uq_core::history::label);
        if changes.is_empty() {
            return Ok(());
        }
        let now = Utc::now().timestamp();
        let today = Local
            .timestamp_opt(now, 0)
            .earliest()
            .map(|t| t.date_naive());
        let have_today = self.restore.last_auto(AutoReason::Daily).is_some_and(|p| {
            Local
                .timestamp_opt(p.time, 0)
                .earliest()
                .map(|t| t.date_naive())
                == today
        });
        if !have_today
            && let Err(e) = self.restore.save(
                &old,
                "Before today's changes",
                PointKind::Auto(AutoReason::Daily),
                now,
            )
        {
            tracing::warn!("saving daily restore point: {e}");
        }
        let origin = origin.as_str();
        let entries: Vec<Entry> = changes
            .into_iter()
            .map(|c| Entry::new(now, origin.clone(), c.domain, c.summary, c.before, c.after))
            .collect();
        match self.journal.append(&entries) {
            Ok(()) => {
                self.last_entry_ids = entries.into_iter().map(|e| e.id).collect();
                let _ = self.events.send(Event::HistoryChanged);
            }
            Err(e) => tracing::warn!("writing history: {e}"),
        }
        self.journal.prune(now);
        self.restore.prune();
        Ok(())
    }

    /// Keep a restore point of the state a new uQ version found, once per version.
    fn save_new_version_point(&self) {
        let version = env!("CARGO_PKG_VERSION");
        if self
            .restore
            .last_auto(AutoReason::NewVersion)
            .is_some_and(|p| p.name == version)
        {
            return;
        }
        if let Err(e) = self.restore.save(
            &self.state,
            version,
            PointKind::Auto(AutoReason::NewVersion),
            Utc::now().timestamp(),
        ) {
            tracing::warn!("saving new-version restore point: {e}");
        }
    }

    /// Validate, then write theme dir + generated.lua + state; then apply live and notify.
    pub(crate) async fn commit(
        &mut self,
        next: State,
        change: Change,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let c = self.render_candidate(&next).await?;
        self.install(c, next, change, origin).await
    }

    /// Render `next`, write the candidate `generated.lua` and have Hyprland verify it. The
    /// candidate stays on disk for `install`; a rejected one is removed.
    pub(crate) async fn render_candidate(&self, next: &State) -> Result<Candidate, DesktopError> {
        let theme = self
            .themes
            .get(&next.theme, next.variant)
            .ok_or_else(|| DesktopError::UnknownTheme(next.theme.clone()))?
            .clone();
        // The user's appearance overrides win over the theme's fonts, cursor and icons.
        let theme = theme.with_appearance(&next.settings.appearance);
        let base = if next.settings.appearance.kvantum {
            kvantum_base(Path::new(&self.kvantum_dir))
        } else {
            None
        };
        let files = render(&theme, &self.paths.templates_dir, base)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        let generated = emit_generated(&theme, &next.layouts, &next.settings);

        self.ensure_user_lua()?;
        let path = self
            .paths
            .generated_lua
            .with_file_name("generated.lua.candidate");
        // The verify config sits beside the real hyprland.lua and mirrors the stub, so relative
        // requires in user.lua resolve exactly as they do in the session.
        let verify_main = self
            .paths
            .hypr_dir
            .join(format!(".uq-verify-{}.lua", std::process::id()));
        atomic_write(&path, &generated).map_err(io_err("writing candidate config"))?;
        let main_src = format!(
            "require({})\nrequire(\"./uq/generated.lua.candidate\")\nrequire(\"./uq/user.lua\")\n",
            lua_string(&self.paths.base_lua.to_string_lossy())
        );
        let verdict = match atomic_write(&verify_main, &main_src) {
            Ok(()) => self.comp.verify(&verify_main).await,
            Err(e) => Err(CompositorError::Io(e)),
        };
        let _ = std::fs::remove_file(&verify_main);
        match verdict {
            Ok(()) => {}
            Err(CompositorError::Rejected(msg)) => {
                let _ = std::fs::remove_file(&path);
                let _ = self.events.send(Event::ConfigError(msg.clone()));
                return Err(DesktopError::ConfigInvalid(msg));
            }
            Err(e) => tracing::warn!("could not validate config ({e}); applying unvalidated"),
        }
        Ok(Candidate {
            files,
            generated,
            path,
            theme,
        })
    }

    /// Write a verified candidate (theme dir, `generated.lua`, state), then apply `change` live.
    pub(crate) async fn install(
        &mut self,
        c: Candidate,
        next: State,
        change: Change,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let theme = c.theme.clone();
        let committed = atomic_replace_dir(&self.paths.theme_dir, &c.files)
            .map_err(io_err("writing theme files"))
            .and_then(|()| {
                std::fs::rename(&c.path, &self.paths.generated_lua)
                    .map_err(io_err("installing generated.lua"))
            })
            .and_then(|()| self.persist(next, origin));
        if let Err(e) = committed {
            let _ = std::fs::remove_file(&c.path);
            return Err(e);
        }
        // Every section is rendered into the config just installed, so no rejection is pending.
        self.last_rejected = None;

        match change {
            Change::Quiet => {}
            Change::Layout(ws, layout) => {
                let _ = self.live(self.comp.eval(&emit_workspace_rule(&ws, layout)).await);
                let _ = self.events.send(Event::LayoutChanged {
                    workspace: ws,
                    layout,
                });
                // The Layouts page follows keybind changes through the section.
                if let Ok(v) = self.state.section("layouts") {
                    let _ = self.events.send(Event::SectionChanged {
                        domain: "layouts".to_string(),
                        json: v.to_string(),
                    });
                }
            }
            Change::Reload => {
                let _ = self.live(self.comp.reload().await);
                self.reapply_pending().await;
            }
            Change::Input => {
                let _ = self.live(self.comp.reload().await);
                self.reapply_pending().await;
                let _ = self.events.send(Event::SettingsChanged(Box::new(
                    self.state.settings.clone(),
                )));
            }
            Change::Theme => {
                // gsettings are desktop-wide: only touch them while a Hyprland session is live,
                // so a uqd left running never re-themes another desktop.
                let live = self.live(self.comp.reload().await);
                self.reapply_pending().await;
                if live {
                    if let Err(e) = self.sys.apply(&theme) {
                        tracing::warn!("system settings: {e}");
                    }
                    let cursor = self
                        .comp
                        .set_cursor(&theme.icons.cursor_theme, theme.icons.cursor_size)
                        .await;
                    self.live(cursor);
                }
                let _ = self.events.send(Event::ThemeChanged {
                    name: theme.id.clone(),
                    variant: theme.variant,
                    tokens_path: self.paths.theme_dir.join("tokens.json"),
                });
            }
        }
        Ok(())
    }

    /// Report the outcome of a live-apply call; the change is already persisted either way.
    /// Returns whether a live Hyprland answered.
    fn live(&self, res: Result<(), CompositorError>) -> bool {
        match res {
            Ok(()) => true,
            Err(CompositorError::NotRunning) => {
                tracing::warn!("Hyprland not running; change saved for next start");
                false
            }
            Err(CompositorError::Rejected(msg)) => {
                tracing::warn!("Hyprland rejected live change: {msg}");
                let _ = self.events.send(Event::ConfigError(msg));
                true
            }
            Err(e) => {
                tracing::warn!("Hyprland: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::fake::FakeCompositor;
    use crate::system::FakeSystem;
    use std::fs;
    use std::path::PathBuf;
    use uq_core::history::Origin;
    use uq_core::lua::Layout;
    use uq_core::schedule::Schedule;

    struct Rig {
        _dir: tempfile::TempDir,
        desktop: Desktop,
        comp: Arc<FakeCompositor>,
        sys: Arc<FakeSystem>,
        events: broadcast::Receiver<Event>,
    }

    fn rig_with(running: bool, prepare: impl FnOnce(&Paths)) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths = Paths::under(dir.path());
        paths.themes_dir = repo.join("themes");
        paths.templates_dir = repo.join("templates");
        prepare(&paths);
        let comp = Arc::new(FakeCompositor::default());
        *comp.running.lock().unwrap() = running;
        let sys = Arc::new(FakeSystem::default());
        let desktop = Desktop::new(paths, comp.clone(), sys.clone()).unwrap();
        let events = desktop.subscribe();
        Rig {
            _dir: dir,
            desktop,
            comp,
            sys,
            events,
        }
    }

    fn rig() -> Rig {
        rig_with(true, |_| {})
    }

    /// Every event so far.
    fn drain_all(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    /// Events so far, leaving out `HistoryChanged`, which every journalled write adds.
    fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
        let mut evs = drain_all(rx);
        evs.retain(|e| *e != Event::HistoryChanged);
        evs
    }

    #[tokio::test]
    async fn set_theme_writes_files_and_emits() {
        let mut r = rig();
        r.desktop
            .set_theme("dracula", Variant::Light, Origin::Settings)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        let tokens = fs::read_to_string(p.theme_dir.join("tokens.json")).unwrap();
        assert!(tokens.contains(r#""id": "dracula""#), "{tokens}");
        let generated = fs::read_to_string(&p.generated_lua).unwrap();
        assert!(generated.contains("Dracula (light)"), "{generated}");
        assert_eq!(State::load(&p.state_file).theme, "dracula");
        assert_eq!(
            r.desktop.get_theme(),
            ("dracula".to_string(), Variant::Light)
        );
        assert!(r.comp.calls().contains(&"reload".to_string()));
        assert_eq!(
            *r.sys.applied.lock().unwrap(),
            vec!["dracula light".to_string()]
        );
        assert_eq!(
            drain(&mut r.events),
            vec![Event::ThemeChanged {
                name: "dracula".into(),
                variant: Variant::Light,
                tokens_path: p.theme_dir.join("tokens.json")
            }]
        );
    }

    #[tokio::test]
    async fn named_workspace_layout_uses_name_selector() {
        let mut r = rig();
        r.desktop
            .set_layout("web", "scrolling", Origin::Settings)
            .await
            .unwrap();
        assert_eq!(
            r.desktop.get_layouts().get("name:web"),
            Some(&Layout::Scrolling)
        );
        assert!(
            r.comp.calls().contains(
                &r#"eval hl.workspace_rule({ workspace = "name:web", layout = "scrolling" })"#
                    .to_string()
            ),
            "{:?}",
            r.comp.calls()
        );
    }

    #[tokio::test]
    async fn verify_main_lives_in_hypr_dir_with_relative_requires() {
        let mut r = rig();
        r.desktop
            .set_layout("1", "master", Origin::Settings)
            .await
            .unwrap();
        let (path, src) = r.comp.verified.lock().unwrap()[0].clone();
        assert_eq!(path.parent(), Some(r.desktop.paths().hypr_dir.as_path()));
        assert!(
            src.contains(r#"require("./uq/generated.lua.candidate")"#),
            "{src}"
        );
        assert!(src.contains(r#"require("./uq/user.lua")"#), "{src}");
    }

    #[tokio::test]
    async fn no_system_settings_without_hyprland() {
        let mut r = rig_with(false, |_| {});
        r.desktop
            .set_theme("matcha", Variant::Light, Origin::Settings)
            .await
            .unwrap();
        assert!(
            r.sys.applied.lock().unwrap().is_empty(),
            "gsettings touched outside Hyprland"
        );
    }

    #[tokio::test]
    async fn manual_override_holds_until_next_boundary() {
        let mut r = rig();
        let day = chrono::Local::now().date_naive();
        let at = |d: chrono::NaiveDate, h, m| {
            d.and_hms_opt(h, m, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .earliest()
                .unwrap()
        };
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        r.desktop
            .set_schedule_at(s, at(day, 20, 0), Origin::Settings)
            .await
            .unwrap();
        assert_eq!(r.desktop.get_theme().1, Variant::Dark);
        r.desktop
            .set_theme("graphite", Variant::Light, Origin::Settings)
            .await
            .unwrap();
        assert!(
            !r.desktop.tick_schedule_at(at(day, 22, 1)).await.unwrap(),
            "override reverted"
        );
        assert_eq!(r.desktop.get_theme().1, Variant::Light);
        let tomorrow = day.succ_opt().unwrap();
        r.desktop
            .tick_schedule_at(at(tomorrow, 7, 1))
            .await
            .unwrap();
        assert_eq!(r.desktop.get_theme().1, Variant::Light);
        assert!(
            r.desktop
                .tick_schedule_at(at(tomorrow, 19, 1))
                .await
                .unwrap()
        );
        assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    }

    #[tokio::test]
    async fn set_layout_persists_and_evals() {
        let mut r = rig();
        r.desktop
            .set_layout("2", "scrolling", Origin::Settings)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        assert!(
            r.comp.calls().contains(
                &r#"eval hl.workspace_rule({ workspace = "2", layout = "scrolling" })"#.to_string()
            ),
            "{:?}",
            r.comp.calls()
        );
        assert_eq!(
            State::load(&p.state_file).layouts.get("2"),
            Some(&Layout::Scrolling)
        );
        assert!(
            fs::read_to_string(&p.generated_lua)
                .unwrap()
                .contains(r#"workspace = "2", layout = "scrolling""#)
        );
        assert_eq!(r.desktop.get_layouts().get("2"), Some(&Layout::Scrolling));
        let evs = drain(&mut r.events);
        assert_eq!(evs.len(), 2, "{evs:?}");
        assert_eq!(
            evs[0],
            Event::LayoutChanged {
                workspace: "2".into(),
                layout: Layout::Scrolling
            }
        );
        assert!(matches!(&evs[1], Event::SectionChanged { domain, .. } if domain == "layouts"));
        // A layout change doesn't re-theme the session.
        assert!(r.sys.applied.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_verify_keeps_last_good_and_reports() {
        let mut r = rig();
        r.desktop
            .set_theme("forest", Variant::Dark, Origin::Settings)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        let before = fs::read(&p.generated_lua).unwrap();
        let tokens_before = fs::read(p.theme_dir.join("tokens.json")).unwrap();
        drain(&mut r.events);
        *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: syntax error".into());

        let err = r
            .desktop
            .set_theme("velvet", Variant::Light, Origin::Settings)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, DesktopError::ConfigInvalid(m) if m == "user.lua:3: syntax error"),
            "{err:?}"
        );
        assert_eq!(fs::read(&p.generated_lua).unwrap(), before);
        assert_eq!(
            fs::read(p.theme_dir.join("tokens.json")).unwrap(),
            tokens_before
        );
        assert_eq!(State::load(&p.state_file).theme, "forest");
        assert_eq!(r.desktop.get_theme().0, "forest");
        assert_eq!(
            drain(&mut r.events),
            vec![Event::ConfigError("user.lua:3: syntax error".into())]
        );
        let leftovers: Vec<_> = fs::read_dir(p.generated_lua.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let mut leftovers = leftovers;
        leftovers.sort();
        assert_eq!(leftovers, vec!["generated.lua", "user.lua"]);
    }

    #[tokio::test]
    async fn verify_sees_candidate_and_user_files() {
        let mut r = rig();
        r.desktop
            .set_layout("1", "master", Origin::Settings)
            .await
            .unwrap();
        let verify = r
            .comp
            .calls()
            .into_iter()
            .find(|c| c.starts_with("verify "))
            .unwrap();
        let main = PathBuf::from(verify.trim_start_matches("verify "));
        assert!(!main.exists(), "temporary main config left behind");
    }

    #[tokio::test]
    async fn compositor_down_still_persists() {
        let mut r = rig_with(false, |_| {});
        r.desktop
            .set_layout("web", "dwindle", Origin::Settings)
            .await
            .unwrap();
        r.desktop
            .set_theme("matcha", Variant::Light, Origin::Settings)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        let s = State::load(&p.state_file);
        assert_eq!(
            (s.theme.as_str(), s.layouts.get("name:web")),
            ("matcha", Some(&Layout::Dwindle))
        );
        assert!(p.theme_dir.join("tokens.json").exists());
    }

    #[tokio::test]
    async fn unknown_theme_rejected() {
        let mut r = rig();
        let err = r
            .desktop
            .set_theme("solarized", Variant::Dark, Origin::Settings)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::UnknownTheme(ref id) if id == "solarized"));
    }

    #[tokio::test]
    async fn invalid_layout_rejected() {
        let mut r = rig();
        let err = r
            .desktop
            .set_layout("1", "monocle", Origin::Settings)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidLayout(ref l) if l == "monocle"));
    }

    #[tokio::test]
    async fn set_layout_enforces_section_limits() {
        let mut r = rig();
        let long = "a".repeat(65);
        assert!(matches!(
            r.desktop
                .set_layout(&long, "master", Origin::Settings)
                .await,
            Err(DesktopError::InvalidSetting(_))
        ));
        assert!(matches!(
            r.desktop
                .set_layout("a\u{1}b", "master", Origin::Settings)
                .await,
            Err(DesktopError::InvalidSetting(_))
        ));
        for i in 1..=64 {
            r.desktop
                .set_layout(&i.to_string(), "master", Origin::Settings)
                .await
                .unwrap();
        }
        let err = r
            .desktop
            .set_layout("65", "master", Origin::Settings)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("64"), "{err}");
        // Changing an existing rule at the limit is fine.
        r.desktop
            .set_layout("64", "scrolling", Origin::Settings)
            .await
            .unwrap();
        assert_eq!(r.desktop.state.layouts.len(), 64);
        let v = r.desktop.state.section("layouts").unwrap();
        r.desktop.state.clone().set_section("layouts", v).unwrap();
    }

    #[tokio::test]
    async fn empty_workspace_rejected() {
        let mut r = rig();
        assert!(matches!(
            r.desktop.set_layout("  ", "master", Origin::Settings).await,
            Err(DesktopError::EmptyWorkspace)
        ));
    }

    #[tokio::test]
    async fn set_schedule_applies_current_variant() {
        let mut r = rig();
        r.desktop
            .set_theme("graphite", Variant::Dark, Origin::Settings)
            .await
            .unwrap();
        let noon = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let now = noon.and_local_timezone(chrono::Local).earliest().unwrap();
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        r.desktop
            .set_schedule_at(s.clone(), now, Origin::Settings)
            .await
            .unwrap();
        assert_eq!(
            r.desktop.get_theme(),
            ("graphite".to_string(), Variant::Light)
        );
        assert_eq!(State::load(&r.desktop.paths().state_file).schedule, s);
    }

    #[tokio::test]
    async fn schedule_tick_switches_variant_when_due() {
        let mut r = rig();
        let at = |h| {
            chrono::Local::now()
                .date_naive()
                .and_hms_opt(h, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .earliest()
                .unwrap()
        };
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        r.desktop
            .set_schedule_at(s, at(12), Origin::Settings)
            .await
            .unwrap();
        assert_eq!(r.desktop.get_theme().1, Variant::Light);
        drain(&mut r.events);
        assert!(
            !r.desktop.tick_schedule_at(at(13)).await.unwrap(),
            "nothing due at 13:00"
        );
        assert!(drain(&mut r.events).is_empty());
        assert!(
            r.desktop.tick_schedule_at(at(20)).await.unwrap(),
            "dark due at 20:00"
        );
        assert_eq!(r.desktop.get_theme().1, Variant::Dark);
    }

    #[tokio::test]
    async fn schedule_tick_off_does_nothing() {
        let mut r = rig();
        assert!(
            !r.desktop
                .tick_schedule_at(chrono::Utc::now())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn invalid_schedule_rejected() {
        let mut r = rig();
        let t = chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap();
        let err = r
            .desktop
            .set_schedule(
                Schedule::Fixed {
                    light_at: t,
                    dark_at: t,
                },
                Origin::Settings,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidSchedule(_)));
    }

    #[tokio::test]
    async fn startup_creates_user_lua_and_renders() {
        let mut r = rig();
        r.desktop.startup().await;
        let p = r.desktop.paths().clone();
        assert!(p.user_lua.exists());
        assert!(p.generated_lua.exists());
        assert!(
            fs::read_to_string(&p.generated_lua)
                .unwrap()
                .contains("Graphite (dark)")
        );
    }

    #[tokio::test]
    async fn startup_never_overwrites_user_lua() {
        let mut r = rig_with(true, |p| {
            fs::create_dir_all(p.user_lua.parent().unwrap()).unwrap();
            fs::write(&p.user_lua, "hl.config({ general = { gaps_in = 0 } })\n").unwrap();
        });
        r.desktop.startup().await;
        assert_eq!(
            fs::read_to_string(&r.desktop.paths().user_lua).unwrap(),
            "hl.config({ general = { gaps_in = 0 } })\n"
        );
    }

    #[tokio::test]
    async fn stale_theme_in_state_falls_back_to_default() {
        let r = rig_with(true, |p| {
            let s = State {
                theme: "removed".into(),
                ..State::default()
            };
            s.save(&p.state_file).unwrap();
        });
        assert_eq!(
            r.desktop.get_theme(),
            ("graphite".to_string(), Variant::Dark)
        );
    }

    fn counts(r: &Rig) -> BTreeMap<String, u32> {
        r.desktop.state().launch_counts.clone()
    }

    #[tokio::test]
    async fn set_setting_persists_and_emits() {
        let mut r = rig();
        // Defaults are 240/300/360/1200, so lock = 400 needs screen_off moved up first.
        r.desktop
            .set_setting("idle.screen_off", serde_json::json!(500), Origin::Settings)
            .await
            .unwrap();
        drain(&mut r.events);
        r.desktop
            .set_setting("idle.lock", serde_json::json!(400), Origin::Settings)
            .await
            .unwrap();
        assert_eq!(
            State::load(&r.desktop.paths().state_file)
                .settings
                .idle
                .lock,
            Some(400)
        );
        let evs = drain(&mut r.events);
        assert_eq!(evs.len(), 1, "{evs:?}");
        let Event::SettingsChanged(s) = &evs[0] else {
            panic!("{evs:?}")
        };
        assert_eq!(s.idle.lock, Some(400));
        let json = serde_json::to_string(s).unwrap();
        assert!(!json.contains("launch_counts"), "{json}");
        // The light path never touches generated.lua or the compositor.
        assert!(!r.desktop.paths().generated_lua.exists());
        assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
    }

    #[tokio::test]
    async fn set_setting_invalid_rejected() {
        let mut r = rig();
        let before = r.desktop.state().clone();
        let err = r
            .desktop
            .set_setting("idle.lock", serde_json::json!(1), Origin::Settings)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
        let err = r
            .desktop
            .set_setting("nope", serde_json::json!(1), Origin::Settings)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
        assert_eq!(r.desktop.state(), &before);
        assert!(drain(&mut r.events).is_empty());
        assert!(!r.desktop.paths().state_file.exists());
    }

    #[tokio::test]
    async fn mouse_setting_renders_verifies_and_reloads() {
        let mut r = rig();
        r.desktop
            .set_setting(
                "mouse.sensitivity",
                serde_json::json!(-0.5),
                Origin::Settings,
            )
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        let generated = fs::read_to_string(&p.generated_lua).unwrap();
        assert!(generated.contains("sensitivity = -0.5,"), "{generated}");
        assert_eq!(State::load(&p.state_file).settings.mouse.sensitivity, -0.5);
        let calls = r.comp.calls();
        assert!(calls.iter().any(|c| c.starts_with("verify ")), "{calls:?}");
        assert!(calls.contains(&"reload".to_string()), "{calls:?}");
        let evs = drain(&mut r.events);
        assert_eq!(evs.len(), 1, "{evs:?}");
        let Event::SettingsChanged(s) = &evs[0] else {
            panic!("{evs:?}")
        };
        assert_eq!(s.mouse.sensitivity, -0.5);
        // Input changes never re-theme the desktop.
        assert!(r.sys.applied.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn mouse_setting_rejected_by_verify_keeps_old_value() {
        let mut r = rig();
        *r.comp.verify_error.lock().unwrap() = Some("input: bad value".into());
        let err = r
            .desktop
            .set_setting(
                "mouse.left_handed",
                serde_json::json!(true),
                Origin::Settings,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
        assert!(!r.desktop.settings().mouse.left_handed);
        assert!(!r.desktop.paths().state_file.exists());
        assert_eq!(
            drain(&mut r.events),
            vec![Event::ConfigError("input: bad value".into())]
        );
    }

    #[tokio::test]
    async fn invalid_mouse_value_never_renders() {
        let mut r = rig();
        let err = r
            .desktop
            .set_setting(
                "mouse.scroll_factor",
                serde_json::json!(99),
                Origin::Settings,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
        assert!(r.comp.calls().is_empty(), "{:?}", r.comp.calls());
        assert!(!r.desktop.paths().generated_lua.exists());
    }

    #[tokio::test]
    async fn record_launch_throttled() {
        let mut r = rig();
        let t0 = Instant::now();
        r.desktop.record_launch_at("a.desktop", t0);
        r.desktop
            .record_launch_at("a.desktop", t0 + Duration::from_secs(1));
        assert_eq!(counts(&r).get("a.desktop"), Some(&2));
        assert_eq!(r.desktop.launch_saves(), 1);
        assert_eq!(
            State::load(&r.desktop.paths().state_file)
                .launch_counts
                .get("a.desktop"),
            Some(&1),
            "second launch not yet on disk"
        );
        let evs = drain(&mut r.events);
        assert_eq!(evs.len(), 2);
        assert!(matches!(&evs[1], Event::LaunchCountsChanged(c) if c.get("a.desktop") == Some(&2)));
        // Too early: the tick flush does nothing. After 30 s it writes the dirty count.
        r.desktop
            .flush_launch_counts_at(t0 + Duration::from_secs(10));
        assert_eq!(r.desktop.launch_saves(), 1);
        r.desktop
            .flush_launch_counts_at(t0 + Duration::from_secs(31));
        assert_eq!(r.desktop.launch_saves(), 2);
        assert_eq!(
            State::load(&r.desktop.paths().state_file)
                .launch_counts
                .get("a.desktop"),
            Some(&2)
        );
        r.desktop
            .flush_launch_counts_at(t0 + Duration::from_secs(90));
        assert_eq!(r.desktop.launch_saves(), 2, "clean state is not rewritten");
    }

    #[tokio::test]
    async fn record_launch_opt_out() {
        let mut r = rig();
        r.desktop
            .set_setting(
                "privacy.record_launches",
                serde_json::json!(false),
                Origin::Settings,
            )
            .await
            .unwrap();
        drain(&mut r.events);
        r.desktop.record_launch("a.desktop");
        assert!(counts(&r).is_empty());
        assert!(drain(&mut r.events).is_empty());
    }

    #[tokio::test]
    async fn launch_count_capped() {
        let mut r = rig();
        r.desktop
            .state
            .launch_counts
            .insert("a.desktop".into(), 10_000);
        r.desktop.record_launch("a.desktop");
        assert_eq!(counts(&r).get("a.desktop"), Some(&10_000));
    }

    #[tokio::test]
    async fn record_launch_rejects_bad_ids() {
        let mut r = rig();
        r.desktop.record_launch("../evil");
        r.desktop.record_launch("");
        assert!(counts(&r).is_empty());
    }

    #[tokio::test]
    async fn clear_launch_counts_saves_and_emits() {
        let mut r = rig();
        r.desktop.record_launch("a.desktop");
        drain(&mut r.events);
        r.desktop.clear_launch_counts();
        assert!(counts(&r).is_empty());
        assert!(
            State::load(&r.desktop.paths().state_file)
                .launch_counts
                .is_empty()
        );
        assert_eq!(
            drain(&mut r.events),
            vec![Event::LaunchCountsChanged(BTreeMap::new())]
        );
    }

    #[tokio::test]
    async fn prune_drops_uninstalled() {
        let mut r = rig();
        r.desktop.record_launch("keep.desktop");
        r.desktop.record_launch("gone.desktop");
        r.desktop.prune_launch_counts(&|id| id == "keep.desktop");
        assert_eq!(counts(&r).keys().collect::<Vec<_>>(), vec!["keep.desktop"]);
        assert_eq!(
            State::load(&r.desktop.paths().state_file)
                .launch_counts
                .keys()
                .collect::<Vec<_>>(),
            vec!["keep.desktop"]
        );
    }

    #[test]
    fn desktop_file_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("applications");
        fs::create_dir_all(&apps).unwrap();
        fs::write(apps.join("foo.desktop"), "").unwrap();
        let dirs = [dir.path().to_path_buf()];
        assert!(desktop_file_installed("foo.desktop", &dirs));
        assert!(desktop_file_installed("foo", &dirs));
        assert!(!desktop_file_installed("bar.desktop", &dirs));
        assert!(!desktop_file_installed("../foo", &dirs));
    }

    fn journal_of(r: &Rig) -> Vec<uq_core::history::Entry> {
        Journal::open(&r.desktop.paths().history_dir).list(100)
    }

    #[tokio::test]
    async fn section_write_appends_one_history_entry() {
        let mut r = rig();
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap();
        let entries = journal_of(&r);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].origin, "settings");
        assert_eq!(entries[0].domain, "look");
        assert_eq!(r.desktop.last_entry_ids(), [entries[0].id.clone()]);
    }

    #[tokio::test]
    async fn light_setting_write_is_journalled() {
        let mut r = rig();
        r.desktop
            .set_setting("idle.dim", serde_json::json!(200), Origin::Uqctl)
            .await
            .unwrap();
        let entries = journal_of(&r);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].origin, "uqctl");
        assert_eq!(entries[0].domain, "settings");
    }

    #[tokio::test]
    async fn several_settings_apply_in_an_order_that_validates() {
        let mut r = rig();
        // Defaults are dim 240 < lock 300 < screen_off 360: dim 310 alone would break the order.
        let values = vec![
            ("idle.dim".to_string(), serde_json::json!(310)),
            ("idle.lock".to_string(), serde_json::json!(320)),
        ];
        r.desktop
            .set_settings(values, Origin::Restore)
            .await
            .unwrap();
        let idle = &r.desktop.settings().idle;
        assert_eq!((idle.dim, idle.lock), (Some(310), Some(320)));
        assert_eq!(journal_of(&r).len(), 1);

        let bad = vec![("idle.dim".to_string(), serde_json::json!(999))];
        let res = r.desktop.set_settings(bad, Origin::Restore).await;
        assert!(
            matches!(res, Err(DesktopError::InvalidSetting(_))),
            "{res:?}"
        );
        assert_eq!(r.desktop.settings().idle.dim, Some(310));
    }

    #[tokio::test]
    async fn schedule_tick_is_not_journalled() {
        let mut r = rig();
        let day = chrono::Local::now().date_naive();
        let at = |d: chrono::NaiveDate, h| {
            d.and_hms_opt(h, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .earliest()
                .unwrap()
        };
        let s = Schedule::Fixed {
            light_at: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            dark_at: chrono::NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
        };
        // At 20:00 the schedule wants dark, the default, so setting it journals one entry.
        r.desktop
            .set_schedule_at(s, at(day, 20), Origin::Settings)
            .await
            .unwrap();
        let entries = journal_of(&r);
        assert_eq!(entries.len(), 1, "{entries:?}");
        let points = r.desktop.restore.list().len();
        let next_morning = at(day.succ_opt().unwrap(), 8);
        assert!(r.desktop.tick_schedule_at(next_morning).await.unwrap());
        assert_eq!(journal_of(&r), entries);
        // The tick leaves the undo target alone.
        assert_eq!(r.desktop.last_entry_ids(), [entries[0].id.clone()]);
        assert_eq!(r.desktop.restore.list().len(), points);
    }

    #[tokio::test]
    async fn rejected_verify_writes_no_entry() {
        let mut r = rig();
        *r.comp.verify_error.lock().unwrap() = Some("bad".into());
        let res = r.desktop.set_layout("1", "master", Origin::Settings).await;
        assert!(matches!(res, Err(DesktopError::ConfigInvalid(_))));
        assert!(journal_of(&r).is_empty());
        assert!(r.desktop.restore.list().is_empty());
    }

    #[tokio::test]
    async fn first_change_of_the_day_saves_a_daily_point() {
        let mut r = rig();
        r.desktop
            .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
            .await
            .unwrap();
        let points = r.desktop.restore.list();
        assert_eq!(points.len(), 1, "{points:?}");
        assert_eq!(points[0].kind, PointKind::Auto(AutoReason::Daily));
        // The point holds the state from before the change.
        let saved = r.desktop.restore.load(&points[0].id).unwrap();
        assert_ne!(saved.settings.idle.dim, Some(200));
        r.desktop
            .set_setting("idle.dim", serde_json::json!(100), Origin::Settings)
            .await
            .unwrap();
        assert_eq!(r.desktop.restore.list().len(), 1);
    }

    #[tokio::test]
    async fn journal_failure_does_not_fail_the_write() {
        let mut r = rig_with(true, |p| {
            fs::create_dir_all(p.history_dir.parent().unwrap()).unwrap();
            fs::write(&p.history_dir, "not a directory").unwrap();
        });
        r.desktop
            .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
            .await
            .unwrap();
        let p = r.desktop.paths().clone();
        assert_eq!(State::load(&p.state_file).settings.idle.dim, Some(200));
        assert!(r.desktop.last_entry_ids().is_empty());
    }

    #[tokio::test]
    async fn startup_saves_new_version_point_once() {
        let mut r = rig();
        r.desktop.startup().await;
        r.desktop.startup().await;
        let points: Vec<_> = r
            .desktop
            .restore
            .list()
            .into_iter()
            .filter(|p| p.kind == PointKind::Auto(AutoReason::NewVersion))
            .collect();
        assert_eq!(points.len(), 1, "{points:?}");
        assert_eq!(points[0].name, env!("CARGO_PKG_VERSION"));
        assert!(journal_of(&r).is_empty());
    }

    #[tokio::test]
    async fn history_changed_event_sent() {
        let mut r = rig();
        r.desktop
            .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
            .await
            .unwrap();
        assert!(drain_all(&mut r.events).contains(&Event::HistoryChanged));
        // A no-op write journals nothing and sends nothing.
        r.desktop
            .set_setting("idle.dim", serde_json::json!(200), Origin::Settings)
            .await
            .unwrap();
        assert!(!drain_all(&mut r.events).contains(&Event::HistoryChanged));
        assert!(r.desktop.last_entry_ids().is_empty());
    }

    fn user_lua(r: &Rig, text: &str) {
        let p = &r.desktop.paths().user_lua;
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    #[tokio::test]
    async fn explain_reports_user_lua_override_with_line() {
        let mut r = rig();
        r.desktop
            .set_section("look", r#"{"gaps_in":5}"#, Origin::Settings)
            .await
            .unwrap();
        user_lua(
            &r,
            "-- mine\nhl.config({ general = {\n    gaps_in = 2,\n} })\n",
        );
        r.comp
            .set_option("general:gaps_in", serde_json::json!("2 2 2 2"));
        let v = r.desktop.explain("look").await.unwrap();
        let row = &v["gaps_in"];
        assert_eq!(row["source"], "user_lua", "{v}");
        assert_eq!(row["stored"], 5);
        assert_eq!(row["live"], "2 2 2 2");
        assert_eq!(row["detail"]["line"], 3);
        assert_eq!(
            row["detail"]["path"],
            r.desktop.paths().user_lua.to_string_lossy().as_ref()
        );
    }

    #[tokio::test]
    async fn explain_reports_runtime_when_user_lua_silent() {
        let mut r = rig();
        r.desktop
            .set_section("look", r#"{"gaps_in":5,"rounding":4}"#, Origin::Settings)
            .await
            .unwrap();
        r.comp
            .set_option("general:gaps_in", serde_json::json!("8 8 8 8"));
        r.comp
            .set_option("decoration:rounding", serde_json::json!(4));
        r.sys
            .set_value("org.gnome.desktop.interface", "cursor-size", "48");
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_in"]["source"], "runtime", "{v}");
        assert_eq!(v["gaps_in"]["detail"], serde_json::Value::Null);
        assert_eq!(v["rounding"]["source"], "uq", "{v}");
        assert_eq!(v["rounding"]["live"], 4);
        // A gsettings key changed by another app is runtime too.
        let v = r.desktop.explain("appearance").await.unwrap();
        assert_eq!(v["cursor_size"]["source"], "runtime", "{v}");
        assert_eq!(v["cursor_size"]["live"], "48");
    }

    #[tokio::test]
    async fn explain_null_key_reports_theme_or_default() {
        let r = rig();
        // graphite's gap_out token is 12; blur is left to base.lua.
        r.comp
            .set_option("general:gaps_out", serde_json::json!("12 12 12 12"));
        r.comp
            .set_option("decoration:blur:enabled", serde_json::json!(true));
        r.sys
            .set_value("org.gnome.desktop.interface", "font-name", "Inter 11");
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_out"]["source"], "theme", "{v}");
        assert_eq!(v["gaps_out"]["stored"], serde_json::Value::Null);
        assert_eq!(v["blur"]["source"], "default", "{v}");
        assert_eq!(v["blur"]["live"], true);
        assert_eq!(v["animation_speed"]["source"], "default", "{v}");
        let v = r.desktop.explain("layouts").await.unwrap();
        assert_eq!(v["dwindle.preserve_split"]["source"], "default", "{v}");
        let v = r.desktop.explain("appearance").await.unwrap();
        assert_eq!(v["ui_font"]["source"], "theme", "{v}");
        assert_eq!(v["font_size"]["source"], "theme", "{v}");
        // A base.lua default that user.lua sets is still found.
        user_lua(
            &r,
            "hl.config({ decoration = { blur = { enabled = false } } })\n",
        );
        r.comp
            .set_option("decoration:blur:enabled", serde_json::json!(false));
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["blur"]["source"], "user_lua", "{v}");
        assert_eq!(v["blur"]["detail"]["line"], 1);
        assert!(matches!(
            r.desktop.explain("bogus").await,
            Err(DesktopError::InvalidArgument(_))
        ));
        assert_eq!(
            r.desktop.explain("wallpaper").await.unwrap(),
            serde_json::json!({})
        );
    }

    #[tokio::test]
    async fn explain_after_rejected_write_reports_error_then_clears() {
        let mut r = rig();
        r.comp
            .set_option("general:gaps_in", serde_json::json!("5 5 5 5"));
        *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
        let err = r
            .desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap_err();
        assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_in"]["source"], "error", "{v}");
        assert_eq!(v["gaps_in"]["detail"]["message"], "user.lua:3: bad");
        assert_eq!(v["rounding"]["source"], "error", "{v}");
        let other = r.desktop.explain("layouts").await.unwrap();
        assert_ne!(other["default"]["source"], "error", "{other}");
        *r.comp.verify_error.lock().unwrap() = None;
        r.desktop.reapply("look").await.unwrap();
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_in"]["source"], "theme", "{v}");
    }

    #[tokio::test]
    async fn restore_after_rejected_write_clears_error() {
        let mut r = rig();
        let id = r.desktop.save_restore_point("before").unwrap();
        *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap_err();
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_in"]["source"], "error", "{v}");
        *r.comp.verify_error.lock().unwrap() = None;
        r.desktop.restore(&id).await.unwrap();
        let v = r.desktop.explain("look").await.unwrap();
        assert!(
            v.as_object()
                .unwrap()
                .values()
                .all(|k| k["source"] != "error"),
            "{v}"
        );
        // A theme change re-renders and verifies every section too.
        *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
        r.desktop
            .set_section("layouts", r#"{"default":"master"}"#, Origin::Settings)
            .await
            .unwrap_err();
        *r.comp.verify_error.lock().unwrap() = None;
        r.desktop
            .set_theme("forest", Variant::Dark, Origin::Settings)
            .await
            .unwrap();
        let v = r.desktop.explain("layouts").await.unwrap();
        assert_ne!(v["default"]["source"], "error", "{v}");
    }

    #[tokio::test]
    async fn explain_without_compositor_uses_state() {
        let mut r = rig_with(false, |_| {});
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap();
        user_lua(&r, "general.gaps_in = 2\n");
        r.comp
            .set_option("general:gaps_in", serde_json::json!("2 2 2 2"));
        r.sys
            .set_value("org.gnome.desktop.interface", "cursor-size", "48");
        let v = r.desktop.explain("look").await.unwrap();
        assert_eq!(v["gaps_in"]["source"], "uq", "{v}");
        assert_eq!(v["gaps_in"]["live"], serde_json::Value::Null);
        assert_eq!(v["gaps_out"]["source"], "theme", "{v}");
        assert_eq!(v["blur"]["source"], "default", "{v}");
        let v = r.desktop.explain("appearance").await.unwrap();
        assert_eq!(v["cursor_size"]["source"], "theme", "{v}");
    }

    #[tokio::test]
    async fn reapply_writes_no_history() {
        let mut r = rig();
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap();
        assert_eq!(journal_of(&r).len(), 1);
        r.comp.calls.lock().unwrap().clear();
        for domain in ["look", "layouts", "appearance", "wallpaper", "shell"] {
            r.desktop.reapply(domain).await.unwrap();
        }
        assert_eq!(journal_of(&r).len(), 1, "{:?}", journal_of(&r));
        assert!(r.desktop.last_entry_ids().is_empty());
        assert!(r.comp.calls().contains(&"reload".to_string()));
        assert_eq!(r.desktop.state().settings.look.gaps_in, Some(9));
        assert!(matches!(
            r.desktop.reapply("bogus").await,
            Err(DesktopError::InvalidArgument(_))
        ));
    }

    #[tokio::test]
    async fn history_summary_names_settings_by_label() {
        let mut r = rig();
        r.desktop
            .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
            .await
            .unwrap();
        let summary = &journal_of(&r)[0].summary;
        assert!(summary.starts_with("Gaps between windows:"), "{summary}");
    }
}
