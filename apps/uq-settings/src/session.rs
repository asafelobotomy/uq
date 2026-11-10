//! The session-bus side, run on a tokio runtime off the Qt thread: claim the single-instance name,
//! serve `org.uq.Settings1`, and follow whether uqd (`org.uq.Desktop1`) is running.

use std::sync::Arc;

use futures_util::StreamExt;
use uq_core::client::DESKTOP_NAME;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use crate::app::{self, Event};
use crate::instance::{self, Claim};

/// Keeps the bus connection (and with it the name) for the app's lifetime.
pub struct Session {
    _conn: zbus::Connection,
}

/// Connect, serve and claim. `Ok(None)`: another instance owns the name and was sent
/// `ShowPage(page, setting)`, so this process should exit.
pub async fn start(page: &str, setting: &str) -> zbus::Result<Option<Session>> {
    let conn = zbus::Connection::session().await?;
    let on_show: instance::ShowHandler =
        Arc::new(|page, setting| app::post(Event::Show { page, setting }));
    instance::serve(&conn, on_show).await?;
    if instance::claim(&conn, page, setting).await? == Claim::Forwarded {
        return Ok(None);
    }
    if let Err(e) = watch_owner(&conn, DESKTOP_NAME, |up| app::post(Event::UqdUp(up))).await {
        eprintln!("uq-settings: can't follow {DESKTOP_NAME}: {e}");
    }
    Ok(Some(Session { _conn: conn }))
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
    use crate::test_bus::{connect, private_bus};
    use std::time::Duration;
    use tokio::sync::mpsc;

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
