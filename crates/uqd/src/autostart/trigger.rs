//! When the run begins (R5): on `ShellReady`, or [`SHELL_WAIT`] after uqd starts.

use super::{SHELL_WAIT, SharedRunner, lock, start_once};
use crate::dbus::SharedDesktop;
use crate::service::Event;
use std::sync::Arc;
use uq_core::autostart::scan::Plan;

/// `ShellReady()`: count it, and start the run in a task unless it has started already.
pub fn shell_ready(desktop: &SharedDesktop, runner: &SharedRunner) {
    let go = {
        let mut r = lock(runner);
        r.shell_ready_calls += 1;
        !r.started
    };
    if go {
        tokio::spawn(run(desktop.clone(), runner.clone()));
    }
}

/// The fallback (R5): start [`SHELL_WAIT`] after uqd starts if the shell never said so.
pub fn spawn_fallback(desktop: SharedDesktop, runner: SharedRunner) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(SHELL_WAIT).await;
        run(desktop, runner).await;
    })
}

async fn run(desktop: SharedDesktop, runner: SharedRunner) {
    {
        let mut r = lock(&runner);
        // A disabled runner (`serve()`'s) only counts `ShellReady`; it doesn't even scan.
        if r.started || !r.enabled() {
            return;
        }
        let d = desktop.clone();
        r.set_recheck(Arc::new(move |id: String| {
            let d = d.clone();
            Box::pin(async move { still_on(&d, &id).await })
        }));
    }
    // uQ's own apps for the kinds nobody chose, before anything starts (Default apps).
    let defaults = {
        let d = desktop.lock().await;
        let state_dir = d
            .paths
            .state_file
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let desktops = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Hyprland".into());
        crate::native_defaults::Where::new(&d.paths.startup, state_dir, &desktops)
    };
    let _ = tokio::task::spawn_blocking(move || crate::native_defaults::run(&defaults)).await;
    let (plan, env, events) = {
        let d = desktop.lock().await;
        lock(&runner).set_when(d.state.autostart.when.clone());
        let (plan, env) = d.autostart_plan();
        // Last session's apps, read before anything notes this session's.
        if d.state.autostart.restore {
            let running: Vec<&str> = plan
                .iter()
                .filter(|e| matches!(e.plan, Plan::Run { .. }))
                .map(|e| e.id.as_str())
                .collect();
            let added_apps = d.state.autostart.added.iter().filter_map(|a| match a {
                uq_core::autostart::Added::App { id, app, .. }
                    if running.contains(&id.as_str()) =>
                {
                    Some(app.clone())
                }
                _ => None,
            });
            let taken = running
                .iter()
                .filter(|id| id.ends_with(".desktop"))
                .map(|id| id.to_string())
                .chain(added_apps)
                .collect();
            let apps = crate::session::snapshot::load(&d.session_file());
            lock(&runner).set_restore(d.restore_plan(&apps, &taken));
        }
        tokio::spawn(crate::session::record(desktop.clone()));
        (plan, env, d.events.clone())
    };
    start_once(&runner, plan, env, move || {
        let _ = events.send(Event::AutostartChanged);
    })
    .await;
}

/// Whether `id` would still run now, from the folders and the state as they are.
async fn still_on(desktop: &SharedDesktop, id: &str) -> bool {
    let (plan, _) = desktop.lock().await.autostart_plan();
    plan.iter()
        .any(|e| e.id == id && matches!(e.plan, Plan::Run { .. }))
}
