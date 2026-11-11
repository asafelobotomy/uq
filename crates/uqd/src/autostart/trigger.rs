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
    let (plan, env, events) = {
        let d = desktop.lock().await;
        let (plan, env) = d.autostart_plan();
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
