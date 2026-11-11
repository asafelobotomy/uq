//! Service events out as D-Bus signals.

use super::DesktopIface;
use crate::service::Event;
use tokio::sync::broadcast;
use zbus::object_server::InterfaceRef;

/// The event's name, for logs: never its content (F34).
fn kind(ev: &Event) -> &'static str {
    match ev {
        Event::ThemeChanged { .. } => "ThemeChanged",
        Event::LayoutChanged { .. } => "LayoutChanged",
        Event::ConfigError(_) => "ConfigError",
        Event::SettingsChanged(_) => "SettingsChanged",
        Event::LaunchCountsChanged(_) => "LaunchCountsChanged",
        Event::SectionChanged { .. } => "SectionChanged",
        Event::HistoryChanged => "HistoryChanged",
        Event::MonitorsChanged => "MonitorsChanged",
        Event::ProfileChanged(_) => "ProfileChanged",
        Event::PickRequested(_) => "PickRequested",
        Event::AutostartChanged => "AutostartChanged",
        Event::ThemesChanged => "ThemesChanged",
    }
}

/// Send each desktop event as its signal until the desktop goes away.
pub(super) async fn forward(
    iface: InterfaceRef<DesktopIface>,
    mut events: broadcast::Receiver<Event>,
) {
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
            Event::AutostartChanged => DesktopIface::autostart_changed(e).await,
            Event::ThemesChanged => Ok(()),
            Event::LaunchCountsChanged(counts) => {
                DesktopIface::launch_counts_changed(
                    e,
                    counts.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                )
                .await
            }
        };
        if let Err(err) = res {
            tracing::warn!("emitting {}: {err}", kind(&ev));
        }
    }
}
