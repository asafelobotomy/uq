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
        Ok(self.desktop.lock().await.restore(id).await?)
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

    #[zbus(signal)]
    async fn history_changed(e: &SignalEmitter<'_>) -> zbus::Result<()>;

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
