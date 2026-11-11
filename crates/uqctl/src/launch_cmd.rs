//! `uqctl launch <desktop-id>`: start an app the way the shell does (R21). The app gets its fix
//! from Apps and scaling, runs in its own scope as Hyprland's child, and the launch is counted.

use crate::bridge::desktop_entry::{Dirs, normalise_id};
use crate::bridge::launch::{FIX_WAIT, dispatch, fix_in, plan, random_suffix};
use crate::bridge::run::RunState;
use serde_json::{Value, json};
use std::process::ExitCode;
use std::time::Duration;
use uq_core::client::DesktopProxy;
use uq_core::compat::AppFix;
use uq_core::settings::Settings;
use uqd::hypr::HyprSocket;

/// Counting is best effort, so it gets a short bound of its own.
const RECORD_WAIT: Duration = Duration::from_secs(2);

/// What uqd knows that a launch uses: the app's fix and the chosen terminal. Anything missing
/// (no bus, uqd down, or no answer within [`FIX_WAIT`] in all) is `None`; the launch goes ahead
/// without it.
async fn from_uqd(
    id: &str,
) -> (
    Option<DesktopProxy<'static>>,
    Option<AppFix>,
    Option<String>,
) {
    let deadline = tokio::time::Instant::now() + FIX_WAIT;
    let proxy = async {
        let conn = zbus::Connection::session().await.ok()?;
        DesktopProxy::builder(&conn)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .ok()
    };
    let Ok(Some(proxy)) = tokio::time::timeout_at(deadline, proxy).await else {
        return (None, None, None);
    };
    let ask = async { tokio::join!(proxy.get_section("compat"), proxy.get_settings()) };
    let Ok((compat, settings)) = tokio::time::timeout_at(deadline, ask).await else {
        return (Some(proxy), None, None);
    };
    let fix = compat
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| fix_in(v, id));
    let terminal = settings
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .and_then(|s| s.shell.terminal);
    (Some(proxy), fix, terminal)
}

async fn launch(id: &str) -> Result<(), String> {
    let (proxy, fix, terminal) = from_uqd(id).await;
    let plan = plan(
        Some(&json!({ "desktop_id": id })),
        &Dirs::from_env(),
        random_suffix(),
        &RunState::default(),
        terminal.as_deref(),
        fix.as_ref(),
    )?;
    dispatch(&HyprSocket::from_env(), &plan).await?;
    if let (Some(proxy), Some(rec)) = (&proxy, &plan.record) {
        // Counted under the full id, as the shell's launches are; best effort.
        let rec = normalise_id(rec);
        let body = (rec,);
        let record = proxy.inner().call::<_, _, ()>("RecordLaunch", &body);
        let _ = tokio::time::timeout(RECORD_WAIT, record).await;
    }
    Ok(())
}

pub async fn run(id: &str) -> ExitCode {
    match launch(id).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("uqctl: {msg}");
            ExitCode::FAILURE
        }
    }
}
