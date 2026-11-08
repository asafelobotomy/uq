//! `org.uq.Desktop1` on the session bus (spec §2.2).

use crate::service::{Desktop, DesktopError, Event};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
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
        }
    }
}

pub struct DesktopIface {
    desktop: SharedDesktop,
}

#[zbus::interface(name = "org.uq.Desktop1")]
impl DesktopIface {
    async fn set_theme(&self, name: &str, variant: &str) -> Result<(), UqError> {
        let v: Variant = variant
            .parse()
            .map_err(|e: uq_core::theme::ThemeError| UqError::InvalidArgument(e.to_string()))?;
        Ok(self.desktop.lock().await.set_theme(name, v).await?)
    }

    async fn get_theme(&self) -> (String, String) {
        let (name, v) = self.desktop.lock().await.get_theme();
        (name, v.to_string())
    }

    /// `json` is a Schedule: `{"mode":"off"}`, `{"mode":"fixed","light_at":"07:00:00","dark_at":"19:00:00"}`
    /// or `{"mode":"sun","lat":51.5,"lon":-0.13}`.
    async fn set_schedule(&self, json: &str) -> Result<(), UqError> {
        let s: Schedule =
            serde_json::from_str(json).map_err(|e| UqError::InvalidSchedule(e.to_string()))?;
        Ok(self.desktop.lock().await.set_schedule(s).await?)
    }

    async fn set_layout(&self, workspace: &str, layout: &str) -> Result<(), UqError> {
        Ok(self
            .desktop
            .lock()
            .await
            .set_layout(workspace, layout)
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

    async fn set_setting(&self, key: &str, json: &str) -> Result<(), UqError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| UqError::InvalidSetting(format!("value is not JSON: {e}")))?;
        Ok(self.desktop.lock().await.set_setting(key, value)?)
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

    #[zbus(signal)]
    async fn settings_changed(e: &SignalEmitter<'_>, json: &str) -> zbus::Result<()>;

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
