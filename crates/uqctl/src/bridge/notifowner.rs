//! Watches who owns `org.freedesktop.Notifications` on the session bus (spec §2.3 `NotifOwner`).

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use zbus::Connection;
use zbus::fdo::DBusProxy;

const NOTIF_NAME: &str = "org.freedesktop.Notifications";

async fn emit(
    tx: &mpsc::Sender<Outbound>,
    owner: Option<String>,
    stop: &CancellationToken,
) -> bool {
    let out = Outbound::Event {
        event: "NotifOwner".into(),
        data: json!({ "owner": owner }),
    };
    tokio::select! { _ = stop.cancelled() => false, r = tx.send(out) => r.is_ok() }
}

/// Emit the current owner (or null), then every change, until `stop` is cancelled.
pub async fn watch(conn: Connection, tx: mpsc::Sender<Outbound>, stop: CancellationToken) {
    let Ok(proxy) = DBusProxy::new(&conn).await else {
        return;
    };
    // Subscribe before reading the current owner so a change in between is not lost.
    let Ok(mut changes) = proxy.receive_name_owner_changed().await else {
        return;
    };
    let current = match proxy
        .get_name_owner(NOTIF_NAME.try_into().expect("valid bus name"))
        .await
    {
        Ok(o) => Some(o.to_string()),
        Err(_) => None,
    };
    if !emit(&tx, current, &stop).await {
        return;
    }
    loop {
        let ev = tokio::select! { _ = stop.cancelled() => return, e = changes.next() => e };
        let Some(ev) = ev else { return };
        let Ok(args) = ev.args() else { continue };
        if args.name().as_str() != NOTIF_NAME {
            continue;
        }
        let owner = args.new_owner().as_ref().map(ToString::to_string);
        if !emit(&tx, owner, &stop).await {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    struct Bus(Child, String, #[allow(dead_code)] tempfile::TempDir);
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Private bus under a short /tmp path (socket paths must stay under 108 bytes).
    fn private_bus() -> Bus {
        let dir = tempfile::Builder::new()
            .prefix("uqn")
            .tempdir_in("/tmp")
            .unwrap();
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address"])
            .arg(format!(
                "--address=unix:path={}",
                dir.path().join("b").display()
            ))
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let addr = line.trim().to_string();
        assert!(addr.len() < 108);
        Bus(child, addr, dir)
    }

    async fn conn(addr: &str) -> Connection {
        zbus::connection::Builder::address(addr)
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    async fn next(rx: &mut mpsc::Receiver<Outbound>) -> serde_json::Value {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("event in time")
            .expect("channel open")
        {
            Outbound::Event { event, data } => {
                assert_eq!(event, "NotifOwner");
                data
            }
            _ => panic!("expected event"),
        }
    }

    #[tokio::test]
    async fn notif_owner_event() {
        let bus = private_bus();
        let watcher = conn(&bus.1).await;
        let (tx, mut rx) = mpsc::channel(8);
        let stop = CancellationToken::new();
        let task = tokio::spawn(watch(watcher, tx, stop.clone()));

        assert_eq!(next(&mut rx).await, json!({"owner": null}));

        let owner = conn(&bus.1).await;
        owner.request_name(NOTIF_NAME).await.unwrap();
        let unique = owner.unique_name().unwrap().to_string();
        assert_eq!(next(&mut rx).await, json!({"owner": unique}));

        owner.release_name(NOTIF_NAME).await.unwrap();
        assert_eq!(next(&mut rx).await, json!({"owner": null}));

        stop.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("watch stops on cancel")
            .unwrap();
    }
}
