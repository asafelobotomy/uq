//! `uq-settings-helper`: D-Bus-activated on the system bus by `uq-settings-helper.service`;
//! exits after 30 s with no calls.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use uq_settings_helper::idle::IDLE_EXIT;
use uq_settings_helper::passwd::SystemPasswd;
use uq_settings_helper::{Config, Helper, NAME, Polkit, serve};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .without_time()
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let conn = zbus::connection::Builder::system()?.build().await?;
    let helper = Arc::new(Helper::new(
        Config::system(),
        Arc::new(Polkit::new(conn.clone())),
        Arc::new(SystemPasswd),
    ));
    serve(&conn, helper.clone()).await?;
    tracing::info!("serving {NAME}");

    let mut term = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = helper.activity().wait_idle(IDLE_EXIT, Duration::from_secs(1)) => {
            tracing::info!("idle for {}s; exiting", IDLE_EXIT.as_secs());
        }
        _ = term.recv() => tracing::info!("stopping"),
    }
    // Give the name up first, so a call that arrives now activates a fresh helper instead of
    // reaching one that is leaving.
    let _ = conn.release_name(NAME).await;
    Ok(())
}
