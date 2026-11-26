//! Putting the interface on a bus, and the timer that reverts an unconfirmed change.

use super::*;

/// uqd's own revert timer: sleeps without the lock, then reverts if `token` is still pending.
/// A confirmed or replaced token makes it a no-op.
pub(crate) async fn revert_timer(
    shared: SharedDesktop,
    token: String,
    deadline: std::time::Instant,
) {
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    shared
        .lock()
        .await
        .expire_monitors(&token, std::time::Instant::now())
        .await;
}

/// Claim the bus name, export the interface and forward service events as signals. The
/// autostart runner is disabled: it never starts a process (F1).
pub async fn serve(
    builder: zbus::connection::Builder<'_>,
    desktop: Desktop,
) -> zbus::Result<(zbus::Connection, SharedDesktop)> {
    serve_with(builder, desktop, AutostartConfig::disabled()).await
}

/// [`serve`] with an autostart runner: with a spawner, it starts the startup apps on
/// `ShellReady`, or [`SHELL_WAIT`](crate::autostart::SHELL_WAIT) after this call.
pub async fn serve_with(
    builder: zbus::connection::Builder<'_>,
    desktop: Desktop,
    cfg: AutostartConfig,
) -> zbus::Result<(zbus::Connection, SharedDesktop)> {
    let events = desktop.subscribe();
    let shared: SharedDesktop = Arc::new(Mutex::new(desktop));
    let enabled = cfg.has_spawner();
    let runner = Runner::shared(cfg);
    let conn = builder
        .name(NAME)
        .serve_at(
            PATH,
            DesktopIface {
                desktop: shared.clone(),
                picks: crate::pick::PickBroker::default(),
                captures: crate::capture::CaptureBroker::default(),
                runner: runner.clone(),
            },
        )
        .build()
        .await?;
    if enabled {
        crate::autostart::spawn_fallback(shared.clone(), runner);
    }
    let iface = conn
        .object_server()
        .interface::<_, DesktopIface>(PATH)
        .await?;
    tokio::spawn(signals::forward(iface, events));
    Ok((conn, shared))
}
