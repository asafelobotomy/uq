//! Watches who owns `org.freedesktop.Notifications` on the session bus (spec §2.3 `NotifOwner`).

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use zbus::Connection;
use zbus::fdo::DBusProxy;

const NOTIF_NAME: &str = "org.freedesktop.Notifications";

/// Name of a process from `/proc/<pid>/comm`: at most 32 printable ASCII chars, else None.
fn clean_comm(comm: &str) -> Option<String> {
    let c = comm.trim();
    if c.is_empty() || !c.chars().all(|ch| ch.is_ascii_graphic() || ch == ' ') {
        return None;
    }
    Some(c.chars().take(32).collect())
}

/// The `NotifOwner` payload. `parent` is this bridge's parent pid (the shell).
fn describe(owner: Option<String>, pid: Option<u32>, comm: Option<&str>, parent: u32) -> Value {
    json!({
        "owner": owner,
        "pid": pid,
        "name": comm.and_then(clean_comm),
        "self": pid == Some(parent),
    })
}

async fn emit(
    tx: &mpsc::Sender<Outbound>,
    proxy: &DBusProxy<'_>,
    owner: Option<String>,
    stop: &CancellationToken,
) -> bool {
    let pid = match &owner {
        Some(o) => match o.as_str().try_into() {
            Ok(name) => proxy.get_connection_unix_process_id(name).await.ok(),
            Err(_) => None,
        },
        None => None,
    };
    let comm = pid.and_then(|p| std::fs::read_to_string(format!("/proc/{p}/comm")).ok());
    let out = Outbound::Event {
        event: "NotifOwner".into(),
        data: describe(
            owner,
            pid,
            comm.as_deref(),
            std::os::unix::process::parent_id(),
        ),
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
    if !emit(&tx, &proxy, current, &stop).await {
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
        if !emit(&tx, &proxy, owner, &stop).await {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use uq_core::test_bus::{connect, private_bus};

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

    #[test]
    fn describe_owner_self_and_name() {
        let o = Some(":1.5".to_string());
        let v = describe(o.clone(), Some(7), Some("quickshell\n"), 7);
        assert_eq!(
            v,
            json!({"owner": ":1.5", "pid": 7, "name": "quickshell", "self": true})
        );
        assert_eq!(describe(o.clone(), Some(7), Some("mako"), 8)["self"], false);
        assert_eq!(describe(o.clone(), None, None, 8)["self"], false);
        assert_eq!(
            describe(o.clone(), Some(7), Some("a\x1bb"), 8)["name"],
            Value::Null
        );
        assert_eq!(
            describe(o.clone(), Some(7), Some("caf\u{e9}"), 8)["name"],
            Value::Null
        );
        let long = "x".repeat(40);
        assert_eq!(
            describe(o, Some(7), Some(&long), 8)["name"],
            json!("x".repeat(32))
        );
    }

    #[tokio::test]
    async fn notif_owner_event() {
        let bus = private_bus();
        let watcher = connect(&bus).await;
        let (tx, mut rx) = mpsc::channel(8);
        let stop = CancellationToken::new();
        let task = tokio::spawn(watch(watcher, tx, stop.clone()));

        let first = next(&mut rx).await;
        assert_eq!(
            first,
            json!({"owner": null, "pid": null, "name": null, "self": false})
        );

        let owner = connect(&bus).await;
        owner.request_name(NOTIF_NAME).await.unwrap();
        let unique = owner.unique_name().unwrap().to_string();
        let got = next(&mut rx).await;
        assert_eq!(got["owner"], json!(unique));
        assert_eq!(got["pid"], json!(std::process::id()));
        assert!(got.get("name").is_some() && got.get("self").is_some());
        // The test process is not its own parent.
        assert_eq!(got["self"], json!(false));

        owner.release_name(NOTIF_NAME).await.unwrap();
        assert_eq!(
            next(&mut rx).await,
            json!({"owner": null, "pid": null, "name": null, "self": false})
        );

        stop.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("watch stops on cancel")
            .unwrap();
    }
}
