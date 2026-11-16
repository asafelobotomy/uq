//! uqd: owns uQ's desktop state and serves `org.uq.Desktop1` on the session bus.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::EnvFilter;
use uq_core::integration::watch::{DEBOUNCE, watch_dirs, watch_dirs_flat};
use uq_core::paths::Paths;
use uqd::autostart::AutostartConfig;
use uqd::compositor::{HyprSocket, Hyprland};
use uqd::dbus::serve_with;
use uqd::hypr_events;
use uqd::notify::SessionNotifier;
use uqd::power::SystemPowerProfiles;
use uqd::service::Event;
use uqd::service::{Desktop, desktop_file_installed, xdg_data_dirs};
use uqd::system::GSettings;
use uqd::walltimer::{Wake, WallTimer};
use uqd::xrandr::SystemXrandr;

/// How long a failed file sync waits before `refresh_files` tries again.
const FILES_RETRY: Duration = Duration::from_secs(60);
/// The check when a watch couldn't be set up (what every check was before).
const POLL: Duration = Duration::from_secs(60);
/// The check with every watch up: only for changes inotify can't see.
const SAFETY: Duration = Duration::from_secs(3600);

/// The next burst from a folder watch; never, once there is none. A watch that ends is dropped,
/// so the loop falls back to `POLL`.
async fn changed(rx: &mut Option<mpsc::Receiver<()>>) {
    match rx {
        Some(r) => {
            if r.recv().await.is_none() {
                tracing::warn!("a folder watch ended; checking every minute");
                *rx = None;
            }
        }
        None => std::future::pending().await,
    }
}

/// The wall clock reaching `at` (or being set); never without a timer.
async fn wait_wall(wall: &mut Option<WallTimer>, at: Option<SystemTime>) -> std::io::Result<Wake> {
    match wall {
        Some(t) => t.wait(at).await,
        None => std::future::pending().await,
    }
}

/// The next desktop event of any kind (missed ones count too).
async fn desktop_event(rx: &mut Option<broadcast::Receiver<Event>>) {
    match rx {
        Some(r) => {
            if let Err(broadcast::error::RecvError::Closed) = r.recv().await {
                *rx = None;
            }
        }
        None => std::future::pending().await,
    }
}

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
    // Nothing here runs on a fixed tick: each part sleeps until its own next deadline or is
    // woken by what it depends on.
    // - Startup files and the user's themes: inotify on the folders they are made from
    //   (`watched_dirs`), plus a retry a minute after a failed write.
    // - The dark/light schedule: a wall-clock timer at the next boundary (it survives suspend
    //   and reports clock changes), any desktop event (a new schedule, a profile that held the
    //   variant ending), and `/etc` for a new time zone (`/etc/localtime` is replaced).
    // - Launch counts: only while there are unsaved ones, when the 30 s throttle allows.
    // A watch that can't be set up falls back to the old once-a-minute check; with all of them
    // up, an hourly check stays as a net for what inotify can't see (a network home folder).
    let (nested, mut flat) = shared.lock().await.watched_dirs();
    if std::env::var_os("TZ").is_none() {
        flat.push("/etc".into());
    }
    let watch = |r: std::io::Result<mpsc::Receiver<()>>| {
        r.map_err(|e| tracing::warn!("watching folders: {e}; checking every minute"))
            .ok()
    };
    let mut nested_rx = watch(watch_dirs(nested, DEBOUNCE));
    let mut flat_rx = watch(watch_dirs_flat(flat, DEBOUNCE));
    let mut wall = WallTimer::new()
        .map_err(|e| tracing::warn!("wall-clock timer: {e}; checking every minute"))
        .ok();
    let mut events = Some(shared.lock().await.subscribe());
    let mut last_poll = Instant::now();
    let mut last_files = Instant::now();
    loop {
        let now = Instant::now();
        let (launch_due, failing, next_boundary) = {
            let d = shared.lock().await;
            (
                d.launch_flush_due(now),
                d.files_failing(),
                d.next_schedule_event(chrono::Local::now()),
            )
        };
        let watching = nested_rx.is_some() && flat_rx.is_some() && wall.is_some();
        let poll_at = last_poll + if watching { SAFETY } else { POLL };
        let retry_at = failing.then(|| last_files + FILES_RETRY);
        let wake_at = [Some(poll_at), launch_due, retry_at]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(poll_at);
        let (mut woke, mut schedule) = (false, false);
        tokio::select! {
            () = tokio::time::sleep_until(wake_at.into()) => {}
            w = wait_wall(&mut wall, next_boundary.map(SystemTime::from)) => {
                if let Err(e) = w {
                    tracing::warn!("wall-clock timer: {e}; checking every minute");
                    wall = None;
                }
                schedule = true;
            }
            () = changed(&mut nested_rx) => woke = true,
            () = changed(&mut flat_rx) => woke = true,
            () = desktop_event(&mut events) => schedule = true,
            _ = tokio::signal::ctrl_c() => break,
            _ = term.recv() => break,
        }
        let now = Instant::now();
        let poll_due = now >= poll_at;
        if poll_due {
            last_poll = now;
            schedule = true;
        }
        let files = files_due(woke, failing, retry_at.is_some_and(|t| now >= t), poll_due);
        let mut d = shared.lock().await;
        // Writes only when due.
        d.flush_launch_counts_at(now);
        if files {
            last_files = now;
            d.refresh_files();
            d.refresh_themes();
        }
        if (files || schedule)
            && let Err(e) = d.tick_schedule_at(chrono::Local::now()).await
        {
            tracing::warn!("scheduled theme change failed: {e}");
        }
    }
    shared
        .lock()
        .await
        .flush_launch_counts_at(std::time::Instant::now() + Duration::from_secs(60));
    tracing::info!("shutting down");
    Ok(())
}

/// Whether to sync the files now. While a write is failing, a watched folder's change waits for
/// the retry: a failed write creates and removes its temporary file in that same folder, which
/// would otherwise wake this again at once and retry about every second (disk full, say).
fn files_due(watch_woke: bool, failing: bool, retry_due: bool, poll_due: bool) -> bool {
    poll_due || retry_due || (watch_woke && !failing)
}

#[cfg(test)]
mod main_tests {
    use super::files_due;

    #[test]
    fn a_failing_write_waits_for_its_retry() {
        assert!(
            files_due(true, false, false, false),
            "a change is synced at once"
        );
        assert!(
            !files_due(true, true, false, false),
            "not while failing: its own temp files"
        );
        assert!(files_due(true, true, true, false), "the retry");
        assert!(files_due(false, true, false, true), "the poll");
        assert!(!files_due(false, false, false, false));
    }
}
