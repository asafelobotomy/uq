//! uqd: owns uQ's desktop state and serves `org.uq.Desktop1` on the session bus.

use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use uq_core::paths::Paths;
use uqd::autostart::AutostartConfig;
use uqd::compositor::{HyprSocket, Hyprland};
use uqd::dbus::serve_with;
use uqd::hypr_events;
use uqd::notify::SessionNotifier;
use uqd::power::SystemPowerProfiles;
use uqd::service::{Desktop, desktop_file_installed, xdg_data_dirs};
use uqd::system::GSettings;
use uqd::xrandr::SystemXrandr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let dev = std::env::var_os("UQ_DEV_SESSION").is_some();
    let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok();
    let mut desktop = Desktop::new(
        Paths::from_env(),
        Arc::new(Hyprland::from_env()),
        Arc::new(GSettings::default()),
    )?
    .with_power(Arc::new(SystemPowerProfiles::default()))
    .with_dev_session(dev);
    desktop.startup().await;
    let data_dirs = xdg_data_dirs();
    desktop.prune_launch_counts(&|id| desktop_file_installed(id, &data_dirs));
    // The only place the real spawner is made (F1).
    let autostart = AutostartConfig::system(dev, session);
    let (conn, shared) =
        serve_with(zbus::connection::Builder::session()?, desktop, autostart).await?;
    tracing::info!("serving org.uq.Desktop1");
    // The only place the real notifier, `xrandr` and the event listener are made (F15): they
    // need the session connection and the shared desktop, which exist only after serving.
    {
        let mut d = shared.lock().await;
        if !dev {
            d.set_notifier(Arc::new(SessionNotifier::new(conn.clone())));
            if let Some(x) = SystemXrandr::find() {
                d.set_xrandr(Arc::new(x));
            }
        }
    }
    // Screens may have changed while uqd was stopped: match once, then follow the events.
    shared.lock().await.outputs_changed(false).await;
    if let Some(path) = HyprSocket::from_env().event_path() {
        tokio::spawn(hypr_events::run(shared.clone(), path));
    }

    let mut term = signal(SignalKind::terminate())?;
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                shared.lock().await.flush_launch_counts();
                shared.lock().await.refresh_files();
                shared.lock().await.refresh_themes();
                if let Err(e) = shared.lock().await.tick_schedule_at(chrono::Local::now()).await {
                    tracing::warn!("scheduled theme change failed: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => break,
            _ = term.recv() => break,
        }
    }
    shared
        .lock()
        .await
        .flush_launch_counts_at(std::time::Instant::now() + Duration::from_secs(60));
    tracing::info!("shutting down");
    Ok(())
}
