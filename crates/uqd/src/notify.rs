//! Desktop notifications for the few things uqd says on its own.

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;
use zbus::zvariant::Value;

/// A notification must never wedge uqd (it holds the desktop lock while it waits).
const TIMEOUT: Duration = Duration::from_secs(3);

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, summary: &str, body: &str);
}

/// Says nothing: tests, a dev session, and the daemon before it has a bus connection.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoNotifier;

#[async_trait]
impl Notifier for NoNotifier {
    async fn notify(&self, _summary: &str, _body: &str) {}
}

/// `org.freedesktop.Notifications.Notify` on the session bus. Made only in `main.rs`.
#[derive(Debug, Clone)]
pub struct SessionNotifier {
    conn: zbus::Connection,
}

impl SessionNotifier {
    pub fn new(conn: zbus::Connection) -> Self {
        SessionNotifier { conn }
    }
}

#[async_trait]
impl Notifier for SessionNotifier {
    async fn notify(&self, summary: &str, body: &str) {
        let args = (
            "uQ",
            0u32,
            "preferences-desktop-display",
            summary,
            body,
            Vec::<&str>::new(),
            HashMap::<&str, Value>::new(),
            8000i32,
        );
        let call = self.conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &args,
        );
        match tokio::time::timeout(TIMEOUT, call).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::warn!("sending a notification: {e}"),
            Err(_) => tracing::warn!("sending a notification: no answer"),
        }
    }
}

/// Remembers what it was asked to say.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct RecordingNotifier {
    pub sent: std::sync::Mutex<Vec<(String, String)>>,
}

#[cfg(test)]
#[async_trait]
impl Notifier for RecordingNotifier {
    async fn notify(&self, summary: &str, body: &str) {
        self.sent
            .lock()
            .unwrap()
            .push((summary.to_string(), body.to_string()));
    }
}
