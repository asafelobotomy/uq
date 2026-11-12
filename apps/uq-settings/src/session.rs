//! The session-bus side, run on a tokio runtime off the Qt thread: claim the single-instance name,
//! serve `org.uq.Settings1`, and follow whether uqd (`org.uq.Desktop1`) is running.

use std::sync::{Arc, OnceLock};

use futures_util::StreamExt;
use uq_core::client::{DESKTOP_NAME, DesktopProxy};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use crate::app::{self, Event};
use crate::instance::{self, Claim};

/// The runtime and bus connection that page models use for their D-Bus calls.
static BUS: OnceLock<(tokio::runtime::Handle, zbus::Connection)> = OnceLock::new();

/// The runtime handle and session-bus connection, once [`start`] has made them; `None` without a
/// session bus (the offscreen screenshot run), when pages show uqd as down.
pub fn bus() -> Option<(tokio::runtime::Handle, zbus::Connection)> {
    BUS.get().cloned()
}

/// Keeps the bus connection (and with it the name) for the app's lifetime.
pub struct Session {
    _conn: zbus::Connection,
}

/// Why [`start`] failed.
#[derive(Debug)]
pub enum StartError {
    /// No session bus: run alone, with uqd counted as down.
    NoBus(zbus::Error),
    /// The bus works but the name couldn't be claimed or the call forwarded: running anyway
    /// would make a second window that no later launch can reach.
    Claim(zbus::Error),
}

/// Connect, serve and claim. `Ok(None)`: another instance owns the name and was sent
/// `ShowPage(page, setting)`, so this process should exit.
pub async fn start(page: &str, setting: &str) -> Result<Option<Session>, StartError> {
    let conn = zbus::Connection::session()
        .await
        .map_err(StartError::NoBus)?;
    let owner = claim_on(&conn, page, setting)
        .await
        .map_err(StartError::Claim)?;
    if owner {
        let _ = BUS.set((tokio::runtime::Handle::current(), conn.clone()));
    }
    Ok(owner.then_some(Session { _conn: conn }))
}

/// Follow uqd without claiming the single-instance name, for the screenshot run
/// (`UQ_SETTINGS_SHOTS_BUS=1`, with `DBUS_SESSION_BUS_ADDRESS` pointing at a private bus).
pub async fn follow_only() -> zbus::Result<Session> {
    let conn = zbus::Connection::session().await?;
    watch_owner(&conn, DESKTOP_NAME, |up| app::post(Event::UqdUp(up))).await?;
    watch_theme(&conn, || app::post(Event::ThemeChanged)).await?;
    let _ = BUS.set((tokio::runtime::Handle::current(), conn.clone()));
    Ok(Session { _conn: conn })
}

/// Serve, claim, and when this process is the owner, start following uqd. `true` = owner.
async fn claim_on(conn: &zbus::Connection, page: &str, setting: &str) -> zbus::Result<bool> {
    let on_show: instance::ShowHandler =
        Arc::new(|page, setting| app::post(Event::Show { page, setting }));
    instance::serve(conn, on_show).await?;
    if instance::claim(conn, page, setting).await? == Claim::Forwarded {
        return Ok(false);
    }
    if let Err(e) = watch_owner(conn, DESKTOP_NAME, |up| app::post(Event::UqdUp(up))).await {
        eprintln!("uq-settings: can't follow {DESKTOP_NAME}: {e}");
    }
    if let Err(e) = watch_theme(conn, || app::post(Event::ThemeChanged)).await {
        eprintln!("uq-settings: can't follow theme changes: {e}");
    }
    Ok(true)
}

/// Call `report` on every uqd `ThemeChanged`, from a task on the current tokio runtime. The
/// match rule doesn't need uqd to be running, so a uqd started later is followed too.
pub async fn watch_theme(
    conn: &zbus::Connection,
    report: impl Fn() + Send + 'static,
) -> zbus::Result<()> {
    let proxy = DesktopProxy::new(conn).await?;
    let mut changes = proxy.receive_theme_changed().await?;
    tokio::spawn(async move {
        while changes.next().await.is_some() {
            report();
        }
    });
    Ok(())
}

/// Report whether `name` has an owner now, then on every `NameOwnerChanged` for it, from a task
/// on the current tokio runtime.
pub async fn watch_owner(
    conn: &zbus::Connection,
    name: &'static str,
    report: impl Fn(bool) + Send + 'static,
) -> zbus::Result<()> {
    let dbus = DBusProxy::new(conn).await?;
    // Subscribe before asking, so a change in between isn't lost.
    let mut changes = dbus
        .receive_name_owner_changed_with_args(&[(0, name)])
        .await?;
    let bus_name = BusName::try_from(name)?;
    report(dbus.name_has_owner(bus_name).await?);
    tokio::spawn(async move {
        while let Some(signal) = changes.next().await {
            if let Ok(args) = signal.args() {
                report(args.new_owner().is_some());
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use uq_core::test_bus::{connect, private_bus};

    async fn next(rx: &mut mpsc::UnboundedReceiver<bool>) -> bool {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no owner report in 5 s")
            .unwrap()
    }

    #[tokio::test]
    async fn uqd_up_follows_the_name_owner() {
        let bus = private_bus();
        let app = connect(&bus).await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        watch_owner(&app, DESKTOP_NAME, move |up| {
            let _ = tx.send(up);
        })
        .await
        .unwrap();
        assert!(!next(&mut rx).await, "nobody owns it at first");

        let uqd = connect(&bus).await;
        uqd.request_name(DESKTOP_NAME).await.unwrap();
        assert!(next(&mut rx).await, "uqd started");

        drop(uqd);
        assert!(!next(&mut rx).await, "uqd went away");
    }

    #[tokio::test]
    async fn uqd_already_running_reports_up() {
        let bus = private_bus();
        let uqd = connect(&bus).await;
        uqd.request_name(DESKTOP_NAME).await.unwrap();
        let app = connect(&bus).await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        watch_owner(&app, DESKTOP_NAME, move |up| {
            let _ = tx.send(up);
        })
        .await
        .unwrap();
        assert!(next(&mut rx).await);
    }
}
