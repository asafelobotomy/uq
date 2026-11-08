//! uqd: owns uQ's desktop state and serves `org.uq.Desktop1` on the session bus.

use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use uq_core::paths::Paths;
use uqd::compositor::Hyprland;
use uqd::dbus::serve;
use uqd::service::Desktop;
use uqd::system::GSettings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let mut desktop = Desktop::new(
        Paths::from_env(),
        Arc::new(Hyprland::from_env()),
        Arc::new(GSettings::default()),
    )?;
    desktop.startup().await;
    let (_conn, shared) = serve(zbus::connection::Builder::session()?, desktop).await?;
    tracing::info!("serving org.uq.Desktop1");

    let mut term = signal(SignalKind::terminate())?;
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(e) = shared.lock().await.tick_schedule_at(chrono::Local::now()).await {
                    tracing::warn!("scheduled theme change failed: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => break,
            _ = term.recv() => break,
        }
    }
    tracing::info!("shutting down");
    Ok(())
}
