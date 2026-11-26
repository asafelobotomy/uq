//! End-to-end: the real `uqctl bridge` binary against a private dbus-daemon serving a real
//! `Desktop` with fakes. Never touches the user's session bus.

#[path = "../common/mod.rs"]
mod common;

use common::{Bridge, private_bus, serve_uqd};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uqd::dbus::NAME;

/// How many times the served uqd has been told the shell is up.
async fn ready_calls(uqd: &zbus::Connection) -> u32 {
    let iface = uqd
        .object_server()
        .interface::<_, uqd::dbus::DesktopIface>("/org/uq/Desktop1")
        .await
        .unwrap();
    let runner = iface.get().await.runner().clone();
    runner.lock().unwrap().shell_ready_calls()
}

async fn wait_ready_calls(uqd: &zbus::Connection, want: u32) {
    for _ in 0..100 {
        if ready_calls(uqd).await == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(ready_calls(uqd).await, want);
}

/// A uqd that owns its name but never answers `GetWelcome` (or anything else of its own).
struct StuckUqd;

#[zbus::interface(name = "org.uq.Desktop1")]
impl StuckUqd {
    async fn get_welcome(&self) -> String {
        std::future::pending::<()>().await;
        String::new()
    }
}

mod calls;
mod events;
