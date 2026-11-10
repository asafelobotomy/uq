//! Single instance through the session bus name `org.uq.Settings1` (spec §2.1).
//!
//! The first process serves [`Settings1`] at `/org/uq/Settings1` and then takes the name. A later
//! one finds the name taken, forwards its page with `ShowPage`, and exits. If the owner vanished
//! in between, the later one tries the name again and becomes the instance.

use std::sync::Arc;

use uq_core::client::{SETTINGS_NAME, SETTINGS_PATH, SettingsAppProxy, no_owner};
use zbus::fdo::{RequestNameFlags, RequestNameReply};

/// Where `ShowPage` calls go: in the app, onto the Qt thread.
pub type ShowHandler = Arc<dyn Fn(String, String) + Send + Sync>;

/// The `org.uq.Settings1` interface. An empty `page_id` means "just raise".
pub struct Settings1 {
    pub on_show: ShowHandler,
}

#[zbus::interface(name = "org.uq.Settings1")]
impl Settings1 {
    fn show_page(&self, page_id: &str, setting_id: &str) {
        (self.on_show)(page_id.to_owned(), setting_id.to_owned());
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    /// This process owns `org.uq.Settings1`.
    Owner,
    /// Another process owns it and has been sent `ShowPage(page, setting)`.
    Forwarded,
}

/// Serve the interface on `conn`; do this before [`claim`], so no forwarded call is lost.
pub async fn serve(conn: &zbus::Connection, on_show: ShowHandler) -> zbus::Result<()> {
    conn.object_server()
        .at(SETTINGS_PATH, Settings1 { on_show })
        .await
        .map(drop)
}

/// Take `org.uq.Settings1`, or hand `page`/`setting` to whoever holds it.
pub async fn claim(conn: &zbus::Connection, page: &str, setting: &str) -> zbus::Result<Claim> {
    // Each pass either wins the name, forwards, or finds the owner gone and tries again.
    for _ in 0..3 {
        // zbus reports `Exists` (someone else holds the name) as `Error::NameTaken`.
        match conn
            .request_name_with_flags(SETTINGS_NAME, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {
                return Ok(Claim::Owner);
            }
            Ok(RequestNameReply::Exists | RequestNameReply::InQueue)
            | Err(zbus::Error::NameTaken) => {}
            Err(e) => return Err(e),
        }
        let proxy = SettingsAppProxy::new(conn).await?;
        match proxy.show_page(page, setting).await {
            Ok(()) => return Ok(Claim::Forwarded),
            Err(e) if no_owner(&e) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(zbus::Error::Failure(format!(
        "{SETTINGS_NAME} kept changing owner"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_bus::{Bus, connect, private_bus};
    use std::sync::Mutex;
    use std::time::Duration;

    type Calls = Arc<Mutex<Vec<(String, String)>>>;

    /// A connection that serves the interface, recording its calls, and claims the name.
    async fn instance(bus: &Bus, page: &str) -> (zbus::Connection, Calls, Claim) {
        let calls: Calls = Arc::default();
        let conn = connect(bus).await;
        let sink = calls.clone();
        serve(
            &conn,
            Arc::new(move |p, s| sink.lock().unwrap().push((p, s))),
        )
        .await
        .unwrap();
        let claim = claim(&conn, page, "").await.unwrap();
        (conn, calls, claim)
    }

    #[tokio::test]
    async fn second_instance_forwards_and_exits() {
        let bus = private_bus();
        let (_first, first_calls, c1) = instance(&bus, "").await;
        assert_eq!(c1, Claim::Owner);

        let second = connect(&bus).await;
        let c2 = claim(&second, "wallpaper", "wallpaper-fit").await.unwrap();
        assert_eq!(c2, Claim::Forwarded);
        assert_eq!(
            *first_calls.lock().unwrap(),
            [("wallpaper".to_string(), "wallpaper-fit".to_string())]
        );
    }

    #[tokio::test]
    async fn stale_owner_gone_means_owner() {
        let bus = private_bus();
        let (first, _, c1) = instance(&bus, "").await;
        assert_eq!(c1, Claim::Owner);
        let unique = first.unique_name().unwrap().to_owned();
        drop(first);

        // Wait until the daemon has noticed the first connection close.
        let watcher = connect(&bus).await;
        let dbus = zbus::fdo::DBusProxy::new(&watcher).await.unwrap();
        for _ in 0..100 {
            if !dbus.name_has_owner(unique.as_ref().into()).await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let (_second, _, c2) = instance(&bus, "appearance").await;
        assert_eq!(c2, Claim::Owner);
    }
}
