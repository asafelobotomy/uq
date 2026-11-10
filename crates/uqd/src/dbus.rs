//! `org.uq.Desktop1` on the session bus (spec §2.2).

use crate::service::{Desktop, DesktopError, Event};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uq_core::history::Origin;
use uq_core::schedule::Schedule;
use uq_core::theme::Variant;
use zbus::object_server::SignalEmitter;

pub const NAME: &str = "org.uq.Desktop1";
pub const PATH: &str = "/org/uq/Desktop1";
pub const IFACE: &str = "org.uq.Desktop1";

pub type SharedDesktop = Arc<Mutex<Desktop>>;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.Desktop1.Error")]
pub enum UqError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UnknownTheme(String),
    InvalidLayout(String),
    EmptyWorkspace(String),
    InvalidSchedule(String),
    InvalidArgument(String),
    InvalidSetting(String),
    ConfigInvalid(String),
    Io(String),
    Conflict(String),
}

impl From<DesktopError> for UqError {
    fn from(e: DesktopError) -> Self {
        let msg = e.to_string();
        match e {
            DesktopError::UnknownTheme(_) => UqError::UnknownTheme(msg),
            DesktopError::InvalidLayout(_) => UqError::InvalidLayout(msg),
            DesktopError::EmptyWorkspace => UqError::EmptyWorkspace(msg),
            DesktopError::InvalidSchedule(_) => UqError::InvalidSchedule(msg),
            DesktopError::InvalidSetting(_) => UqError::InvalidSetting(msg),
            DesktopError::ConfigInvalid(_) => UqError::ConfigInvalid(msg),
            DesktopError::Io(_) => UqError::Io(msg),
            DesktopError::InvalidArgument(_) => UqError::InvalidArgument(msg),
            DesktopError::Conflict(_) => UqError::Conflict(msg),
        }
    }
}

pub struct DesktopIface {
    desktop: SharedDesktop,
    /// Outside the desktop lock: a pick waits for the user, and writes must still go through.
    picks: crate::pick::PickBroker,
}

/// Every write takes a last `origin` argument (`settings`, `uqctl`, `shell`, …; "" is unknown),
/// which the history journal records.
#[zbus::interface(name = "org.uq.Desktop1")]
impl DesktopIface {
    async fn set_theme(&self, name: &str, variant: &str, origin: &str) -> Result<(), UqError> {
        let v: Variant = variant
            .parse()
            .map_err(|e: uq_core::theme::ThemeError| UqError::InvalidArgument(e.to_string()))?;
        Ok(self
            .desktop
            .lock()
            .await
            .set_theme(name, v, Origin::parse(origin))
            .await?)
    }

    async fn get_theme(&self) -> (String, String) {
        let (name, v) = self.desktop.lock().await.get_theme();
        (name, v.to_string())
    }

    /// `json` is a Schedule: `{"mode":"off"}`, `{"mode":"fixed","light_at":"07:00:00","dark_at":"19:00:00"}`
    /// or `{"mode":"sun","lat":51.5,"lon":-0.13}`.
    async fn set_schedule(&self, json: &str, origin: &str) -> Result<(), UqError> {
        let s: Schedule =
            serde_json::from_str(json).map_err(|e| UqError::InvalidSchedule(e.to_string()))?;
        Ok(self
            .desktop
            .lock()
            .await
            .set_schedule(s, Origin::parse(origin))
            .await?)
    }

    /// The stored schedule as JSON, in the format `SetSchedule` accepts.
    async fn get_schedule(&self) -> Result<String, UqError> {
        let d = self.desktop.lock().await;
        serde_json::to_string(d.schedule()).map_err(|e| UqError::InvalidSchedule(e.to_string()))
    }

    async fn set_layout(&self, workspace: &str, layout: &str, origin: &str) -> Result<(), UqError> {
        Ok(self
            .desktop
            .lock()
            .await
            .set_layout(workspace, layout, Origin::parse(origin))
            .await?)
    }

    async fn get_layouts(&self) -> HashMap<String, String> {
        let layouts = self.desktop.lock().await.get_layouts();
        layouts
            .into_iter()
            .map(|(ws, l)| (ws, l.as_str().to_string()))
            .collect()
    }

    async fn reload(&self) -> Result<(), UqError> {
        Ok(self.desktop.lock().await.reload().await?)
    }

    async fn list_themes(&self) -> Vec<String> {
        self.desktop.lock().await.theme_ids()
    }

    /// The settings as JSON (never includes launch counts).
    async fn get_settings(&self) -> Result<String, UqError> {
        serde_json::to_string(self.desktop.lock().await.settings())
            .map_err(|e| UqError::Io(e.to_string()))
    }

    async fn set_setting(&self, key: &str, json: &str, origin: &str) -> Result<(), UqError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| UqError::InvalidSetting(format!("value is not JSON: {e}")))?;
        Ok(self
            .desktop
            .lock()
            .await
            .set_setting(key, value, Origin::parse(origin))
            .await?)
    }

    /// One whole settings section (`appearance`, `wallpaper`) as JSON.
    async fn get_section(&self, domain: &str) -> Result<String, UqError> {
        Ok(self.desktop.lock().await.get_section(domain)?)
    }

    /// Replace a whole section with `json`.
    async fn set_section(&self, domain: &str, json: &str, origin: &str) -> Result<(), UqError> {
        Ok(self
            .desktop
            .lock()
            .await
            .set_section(domain, json, Origin::parse(origin))
            .await?)
    }

    /// Input devices Hyprland reports: `{keyboards, pointers, touch}` as JSON.
    async fn list_input_devices(&self) -> Result<String, UqError> {
        Ok(self.desktop.lock().await.list_input_devices().await?)
    }

    async fn record_launch(&self, id: &str) {
        self.desktop.lock().await.record_launch(id);
    }

    async fn get_launch_counts(&self) -> HashMap<String, u32> {
        self.desktop
            .lock()
            .await
            .get_launch_counts()
            .into_iter()
            .collect()
    }

    async fn clear_launch_counts(&self) {
        self.desktop.lock().await.clear_launch_counts();
    }

    /// The history journal as a JSON array of entries, newest first, at most `limit` (≤ 500).
    async fn list_history(&self, limit: u32) -> Result<String, UqError> {
        let entries = self.desktop.lock().await.history(limit as usize);
        serde_json::to_string(&entries).map_err(|e| UqError::Io(e.to_string()))
    }

    /// Write entry `entry_id`'s `before` back. `Conflict` when its domain changed again since,
    /// unless `force`. Returns the id of the entry the undo recorded ("" if nothing changed).
    async fn undo(&self, entry_id: &str, force: bool) -> Result<String, UqError> {
        let mut d = self.desktop.lock().await;
        d.undo(entry_id, force).await?;
        // Read under the same lock, right after the write, so no other write can replace it.
        Ok(d.last_entry_ids().last().cloned().unwrap_or_default())
    }

    /// Restore points as a JSON array, newest first.
    async fn list_restore_points(&self) -> Result<String, UqError> {
        let points = self.desktop.lock().await.restore_points();
        serde_json::to_string(&points).map_err(|e| UqError::Io(e.to_string()))
    }

    async fn save_restore_point(&self, name: &str) -> Result<String, UqError> {
        Ok(self.desktop.lock().await.save_restore_point(name)?)
    }

    async fn delete_restore_point(&self, id: &str) -> Result<(), UqError> {
        Ok(self.desktop.lock().await.delete_restore_point(id)?)
    }

    async fn restore(&self, id: &str) -> Result<(), UqError> {
        // `Restore` has no report to carry a notice, so it's logged.
        for notice in self.desktop.lock().await.restore(id).await? {
            tracing::warn!("restore: {notice}");
        }
        Ok(())
    }

    /// Where each key of `domain` gets its live value, as JSON:
    /// `{key: {source, live, stored, detail}}`.
    async fn explain(&self, domain: &str) -> Result<String, UqError> {
        let v = self.desktop.lock().await.explain(domain).await?;
        Ok(v.to_string())
    }

    /// Run `domain`'s render path again; changes nothing in state or history. Emits
    /// `SectionChanged` for `domain` afterwards, so open pages explain it again.
    async fn reapply(&self, domain: &str) -> Result<(), UqError> {
        Ok(self.desktop.lock().await.reapply(domain).await?)
    }

    /// Live outputs, the stored layout and any pending change, as JSON:
    /// `{live: [...], stored: {outputs}, pending: null | {token, seconds_left}}`.
    async fn list_outputs(&self) -> String {
        self.desktop
            .lock()
            .await
            .list_outputs(std::time::Instant::now())
            .await
    }

    /// The open windows as a JSON array of `{class, title, initial_class, initial_title,
    /// xwayland, floating, fullscreen, pinned, workspace_id, workspace_name, tags}`; `[]` when
    /// Hyprland is down. No addresses or pids.
    async fn list_windows(&self) -> Result<String, UqError> {
        let comp = self.desktop.lock().await.comp.clone();
        let windows = crate::service::Desktop::windows_of(comp.as_ref()).await;
        serde_json::to_string(&windows).map_err(|e| UqError::Io(e.to_string()))
    }

    /// Let the user click a window (spec §5). Returns JSON `{status, window?}`: `picked` (with
    /// only that window's fields), `none`, `cancelled`, `timeout`, `no_shell` or `unavailable`.
    /// `Conflict` when a pick is already pending. Waits up to 20 s without the desktop lock.
    async fn pick_window(&self) -> Result<String, UqError> {
        let (comp, events) = {
            let d = self.desktop.lock().await;
            (d.comp.clone(), d.events.clone())
        };
        let emit = move |t: String| {
            let _ = events.send(Event::PickRequested(t));
        };
        Ok(crate::pick::pick(&self.picks, comp, emit).await?)
    }

    /// The shell's answer for the pick `token`: `shown`, `click` or `cancel`. An unknown token
    /// does nothing.
    async fn pick_done(&self, token: &str, outcome: &str) -> Result<(), UqError> {
        self.picks.done(token, outcome.parse()?);
        Ok(())
    }

    /// Apply a whole `monitors` section live for 15 s. Returns the token `ConfirmMonitors`
    /// needs; without a confirm, uqd puts the old layout back on its own.
    async fn apply_monitors(&self, json: &str) -> Result<String, UqError> {
        let (token, deadline) = self
            .desktop
            .lock()
            .await
            .apply_monitors(json, std::time::Instant::now())
            .await?;
        tokio::spawn(revert_timer(self.desktop.clone(), token.clone(), deadline));
        Ok(token)
    }

    /// Keep the pending display change: it is saved and recorded with `origin`.
    async fn confirm_monitors(&self, token: &str, origin: &str) -> Result<(), UqError> {
        Ok(self
            .desktop
            .lock()
            .await
            .confirm_monitors(token, Origin::parse(origin))
            .await?)
    }

    /// Put the old layout back now.
    async fn revert_monitors(&self, token: &str) -> Result<(), UqError> {
        Ok(self.desktop.lock().await.revert_monitors(token).await?)
    }

    /// Every profile as a JSON array of `{id, name, icon, keys, power, active}`, by name.
    async fn list_profiles(&self) -> Result<String, UqError> {
        let list = self.desktop.lock().await.list_profiles();
        serde_json::to_string(&list).map_err(|e| UqError::Io(e.to_string()))
    }

    /// The profile's file text (the format export and import use).
    async fn get_profile(&self, id: &str) -> Result<String, UqError> {
        Ok(self.desktop.lock().await.get_profile(id)?)
    }

    /// Save a profile from its file text and return its id. `Conflict` when the id exists and
    /// `replace` is false. Changes no setting.
    async fn set_profile(&self, toml: &str, replace: bool) -> Result<String, UqError> {
        Ok(self.desktop.lock().await.set_profile(toml, replace)?)
    }

    async fn delete_profile(&self, id: &str) -> Result<(), UqError> {
        Ok(self.desktop.lock().await.delete_profile(id)?)
    }

    /// Turn a profile on. Returns the report as JSON: `{active, kept, notices}`.
    async fn apply_profile(&self, id: &str) -> Result<String, UqError> {
        let report = self.desktop.lock().await.apply_profile(id).await?;
        serde_json::to_string(&report).map_err(|e| UqError::Io(e.to_string()))
    }

    /// "Back to normal". Returns the report as JSON, with `active` empty.
    async fn profile_off(&self) -> Result<String, UqError> {
        let report = self.desktop.lock().await.profile_off().await?;
        serde_json::to_string(&report).map_err(|e| UqError::Io(e.to_string()))
    }

    /// A pick began; the shell shows its overlay and answers `PickDone(token, …)`.
    #[zbus(signal)]
    async fn pick_requested(e: &SignalEmitter<'_>, token: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn monitors_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn history_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// The active profile or the list of profiles changed; `id` is the active profile, or `""`.
    #[zbus(signal)]
    async fn profile_changed(e: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn settings_changed(e: &SignalEmitter<'_>, json: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn section_changed(e: &SignalEmitter<'_>, domain: &str, json: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn launch_counts_changed(
        e: &SignalEmitter<'_>,
        counts: HashMap<String, u32>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn theme_changed(
        e: &SignalEmitter<'_>,
        name: &str,
        variant: &str,
        tokens_path: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn layout_changed(
        e: &SignalEmitter<'_>,
        workspace: &str,
        layout: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn config_error(e: &SignalEmitter<'_>, message: &str) -> zbus::Result<()>;
}

/// uqd's own revert timer: sleeps without the lock, then reverts if `token` is still pending.
/// A confirmed or replaced token makes it a no-op.
async fn revert_timer(shared: SharedDesktop, token: String, deadline: std::time::Instant) {
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    shared
        .lock()
        .await
        .expire_monitors(&token, std::time::Instant::now())
        .await;
}

/// Claim the bus name, export the interface and forward service events as signals.
pub async fn serve(
    builder: zbus::connection::Builder<'_>,
    desktop: Desktop,
) -> zbus::Result<(zbus::Connection, SharedDesktop)> {
    let mut events = desktop.subscribe();
    let shared: SharedDesktop = Arc::new(Mutex::new(desktop));
    let conn = builder
        .name(NAME)?
        .serve_at(
            PATH,
            DesktopIface {
                desktop: shared.clone(),
                picks: crate::pick::PickBroker::default(),
            },
        )?
        .build()
        .await?;
    let iface = conn
        .object_server()
        .interface::<_, DesktopIface>(PATH)
        .await?;
    tokio::spawn(async move {
        loop {
            let ev = match events.recv().await {
                Ok(ev) => ev,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("dropped {n} desktop events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let e = iface.signal_emitter();
            let res = match &ev {
                Event::ThemeChanged {
                    name,
                    variant,
                    tokens_path,
                } => {
                    DesktopIface::theme_changed(
                        e,
                        name,
                        variant.as_str(),
                        &tokens_path.to_string_lossy(),
                    )
                    .await
                }
                Event::LayoutChanged { workspace, layout } => {
                    DesktopIface::layout_changed(e, workspace, layout.as_str()).await
                }
                Event::ConfigError(msg) => DesktopIface::config_error(e, msg).await,
                Event::SettingsChanged(settings) => match serde_json::to_string(settings) {
                    Ok(json) => DesktopIface::settings_changed(e, &json).await,
                    Err(err) => Err(zbus::Error::Failure(err.to_string())),
                },
                Event::SectionChanged { domain, json } => {
                    DesktopIface::section_changed(e, domain, json).await
                }
                Event::HistoryChanged => DesktopIface::history_changed(e).await,
                Event::MonitorsChanged => DesktopIface::monitors_changed(e).await,
                Event::ProfileChanged(id) => DesktopIface::profile_changed(e, id).await,
                Event::PickRequested(token) => DesktopIface::pick_requested(e, token).await,
                Event::LaunchCountsChanged(counts) => {
                    DesktopIface::launch_counts_changed(
                        e,
                        counts.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                    )
                    .await
                }
            };
            if let Err(err) = res {
                tracing::warn!("emitting {ev:?}: {err}");
            }
        }
    });
    Ok((conn, shared))
}
