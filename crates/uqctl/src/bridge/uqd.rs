//! Forwarding between the bridge protocol and `org.uq.Desktop1` on the session bus.

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::Settings;
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::{Connection, MatchRule, MessageStream};
mod request;
use request::*;

pub const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
const IFACE: &str = "org.uq.Desktop1";
pub const DOWN: &str = "uqd is not running";
/// `Reload` can take about 12 s inside uqd (verify + reload); never wait forever.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_secs(5);
/// `ShellReady` returns at once in uqd; a watch loop never waits longer than this on it. The
/// `GetWelcome` asked before `Hello` is bounded by it too, so a stuck uqd can't hold up `Hello`.
pub const READY_WAIT: Duration = Duration::from_secs(2);
/// Connecting to the bus, subscribing and asking who owns the name: a bus that accepts the
/// connection but never answers must not stall `open` (and so `Hello`) or the watch loop, which
/// retries on its next poll.
const BUS_WAIT: Duration = Duration::from_secs(2);

/// The bridge's view of uqd: a (lazily established) bus connection plus whether the name is owned.
pub struct Link {
    addr: Option<String>,
    conn: Mutex<Option<Connection>>,
    up: AtomicBool,
    /// Subscriptions made in `open`, before the first state check, so nothing is missed.
    first: Mutex<Option<Subs>>,
    /// The latest settings seen (from `GetSettings` and `SettingsChanged`); None until known.
    settings: watch::Sender<Option<Settings>>,
    /// The shell has sent its first request, so each uqd that appears is told it is up (R5).
    shell_up: AtomicBool,
}

struct Subs {
    owner: OwnerStream,
    signals: MessageStream,
}

type OwnerStream = zbus::fdo::NameOwnerChangedStream;

/// Connect and subscribe to owner changes and uqd's signals, giving up after `BUS_WAIT`.
async fn subscribe(addr: Option<&str>) -> Option<(Connection, Subs)> {
    tokio::time::timeout(BUS_WAIT, subscribe_unbounded(addr))
        .await
        .ok()
        .flatten()
}

async fn subscribe_unbounded(addr: Option<&str>) -> Option<(Connection, Subs)> {
    let conn = connect(addr).await.ok()?;
    let owner = DBusProxy::new(&conn)
        .await
        .ok()?
        // Only uqd's own name: unfiltered, every client joining or leaving the bus would wake
        // this, and a full queue of them stalls the whole connection (zbus stops reading).
        .receive_name_owner_changed_with_args(&[(0, NAME)])
        .await
        .ok()?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(NAME)
        .path(PATH)
        .interface(IFACE)
        .build()
        .ok()?;
    let signals = MessageStream::for_match_rule(rule, &conn, std::num::NonZeroUsize::new(64))
        .await
        .ok()?;
    Some((conn, Subs { owner, signals }))
}

async fn connect(addr: Option<&str>) -> zbus::Result<Connection> {
    match addr {
        Some(a) => zbus::connection::Builder::address(a).build().await,
        None => Connection::session().await,
    }
}

/// Whether uqd owns its name; false when the bus doesn't answer within `BUS_WAIT` (the watch
/// loop's poll asks again while uqd looks down).
async fn has_owner(conn: &Connection) -> bool {
    let ask = async {
        match DBusProxy::new(conn).await {
            Ok(p) => p
                .name_has_owner(NAME.try_into().expect("valid bus name"))
                .await
                .unwrap_or(false),
            Err(_) => false,
        }
    };
    tokio::time::timeout(BUS_WAIT, ask).await.unwrap_or(false)
}

impl Link {
    /// Connect to the bus (if it is reachable) and learn whether uqd owns its name right now.
    /// Call before sending `Hello` so `uqd_up` is truthful.
    pub async fn open(addr: Option<&str>) -> Link {
        let sub = subscribe(addr).await;
        if sub.is_none() {
            eprintln!("uqctl bridge: no session bus yet");
        }
        let (conn, first) = match sub {
            Some((c, s)) => (Some(c), Some(s)),
            None => (None, None),
        };
        let up = match &conn {
            Some(c) => has_owner(c).await,
            None => false,
        };
        Link {
            addr: addr.map(str::to_owned),
            conn: Mutex::new(conn),
            up: AtomicBool::new(up),
            first: Mutex::new(first),
            settings: watch::channel(None).0,
            shell_up: AtomicBool::new(false),
        }
    }

    pub fn settings_rx(&self) -> watch::Receiver<Option<Settings>> {
        self.settings.subscribe()
    }

    /// Fetch the settings from uqd and publish them (a failure leaves the last value).
    async fn refresh_settings(&self) {
        if let Ok(v) = self.call("GetSettings", &[]).await
            && let Ok(s) = serde_json::from_value::<Settings>(v)
        {
            self.settings.send_replace(Some(s));
        }
    }

    /// Record a state change, tell the shell, and learn the settings when uqd appears. A uqd
    /// that appears after the shell's first request is told the shell is up (R5).
    async fn state_changed(&self, tx: &mpsc::Sender<Outbound>, up: bool) -> bool {
        if up {
            self.refresh_settings().await;
        }
        let sent = emit_state(tx, up).await;
        if up && self.shell_up.load(Ordering::SeqCst) {
            // uqd answers at once; never hold up the watch loop for long.
            let _ = tokio::time::timeout(READY_WAIT, self.call("ShellReady", &[])).await;
        }
        sent
    }

    /// The shell's first request has been read: tell uqd the shell is up, once (later calls
    /// do nothing). Errors are ignored; a uqd that appears later is told on its `UqdUp`.
    pub async fn shell_ready(&self) {
        if !self.shell_up.swap(true, Ordering::SeqCst) {
            let _ = self.call("ShellReady", &[]).await;
        }
    }

    pub fn is_up(&self) -> bool {
        self.up.load(Ordering::SeqCst)
    }

    /// Record the current state; returns true when it changed.
    fn set_up(&self, up: bool) -> bool {
        self.up.swap(up, Ordering::SeqCst) != up
    }

    pub async fn connection(&self) -> Option<Connection> {
        self.conn.lock().await.clone()
    }

    /// Forward one bridge call to uqd.
    pub async fn call(&self, call: &str, args: &[Value]) -> Result<Value, String> {
        // Validate before touching the bus so argument errors do not depend on uqd's state.
        let req = Request::parse(call, args)?;
        let conn = match self.connection().await {
            Some(c) if self.is_up() => c,
            _ => return Err(DOWN.into()),
        };
        match tokio::time::timeout(CALL_TIMEOUT, req.send(&conn)).await {
            Ok(r) => r,
            Err(_) => Err("uqd did not answer within 20 s".into()),
        }
    }

    /// Watch the name owner and forward uqd's signals until `stop` is cancelled. Events go to
    /// `tx`. Reconnects to the bus if it goes away.
    pub async fn watch(&self, tx: mpsc::Sender<Outbound>, stop: CancellationToken) {
        let mut listener: Option<tokio::task::JoinHandle<()>> = None;
        let mut owner: Option<OwnerStream> = None;
        let mut poll = tokio::time::interval(POLL);
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut first = self.first.lock().await.take();
        if self.is_up() {
            self.refresh_settings().await;
        }
        'outer: loop {
            if owner.is_none() {
                let subs = match first.take() {
                    Some(s) => Some(s),
                    None => match subscribe(self.addr.as_deref()).await {
                        Some((conn, s)) => {
                            let up = has_owner(&conn).await;
                            *self.conn.lock().await = Some(conn);
                            if self.set_up(up) && !self.state_changed(&tx, up).await {
                                break;
                            }
                            Some(s)
                        }
                        None => None,
                    },
                };
                if let Some(Subs { owner: o, signals }) = subs {
                    owner = Some(o);
                    listener = Some(tokio::spawn(forward(
                        signals,
                        tx.clone(),
                        stop.clone(),
                        self.settings.clone(),
                    )));
                }
            }
            tokio::select! {
                _ = stop.cancelled() => break 'outer,
                // Only while there is something to retry: subscribing again (no owner stream)
                // or asking whether uqd is up. While up and subscribed, nothing wakes this loop
                // but the bus, so an idle bridge stays asleep.
                _ = poll.tick(), if owner.is_none() || !self.is_up() => {
                    // Belt and braces while down; NameOwnerChanged covers the normal case.
                    if !self.is_up()
                        && let Some(conn) = self.connection().await
                    {
                        let up = has_owner(&conn).await;
                        if self.set_up(up) && !self.state_changed(&tx, up).await {
                            break 'outer;
                        }
                    }
                }
                ev = next_owner_change(&mut owner), if owner.is_some() => {
                    match ev {
                        Some(up) => {
                            if self.set_up(up) && !self.state_changed(&tx, up).await {
                                break 'outer;
                            }
                        }
                        // The bus connection ended: uqd is unreachable until we reconnect.
                        None => {
                            owner = None;
                            if let Some(l) = listener.take() {
                                l.abort();
                            }
                            *self.conn.lock().await = None;
                            if self.set_up(false) && !self.state_changed(&tx, false).await {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
        if let Some(l) = listener {
            l.abort();
        }
    }
}

async fn next_owner_change(s: &mut Option<OwnerStream>) -> Option<bool> {
    loop {
        let ev = s.as_mut()?.next().await?;
        let Ok(args) = ev.args() else { continue };
        if args.name().as_str() == NAME {
            return Some(args.new_owner().is_some());
        }
    }
}

async fn emit_state(tx: &mpsc::Sender<Outbound>, up: bool) -> bool {
    let event = if up { "UqdUp" } else { "UqdDown" };
    tx.send(Outbound::Event {
        event: event.into(),
        data: Value::Null,
    })
    .await
    .is_ok()
}

/// Turn uqd's signals into bridge events. Only messages from the owner of the well-known name
/// arrive (the daemon resolves the name in the match rule).
async fn forward(
    mut stream: MessageStream,
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
    settings: watch::Sender<Option<Settings>>,
) {
    // Own the cancellation check: this task holds a sender clone, and aborting the watcher does
    // not abort it.
    while let Some(m) = tokio::select! { _ = stop.cancelled() => None, m = stream.next() => m } {
        let Ok(msg) = m else { break };
        let header = msg.header();
        let Some(member) = header.member() else {
            continue;
        };
        let body = msg.body();
        let parsed = match member.as_str() {
            "ThemeChanged" => body
                .deserialize::<(String, String, String)>()
                .map(|(n, v, p)| json!({"name": n, "variant": v, "tokens_path": p})),
            "LayoutChanged" => body
                .deserialize::<(String, String)>()
                .map(|(w, l)| json!({"workspace": w, "layout": l})),
            "ConfigError" => body.deserialize::<String>().map(|m| json!({"message": m})),
            "SettingsChanged" => body.deserialize::<String>().map(|j| parse_json(&j)),
            "SectionChanged" => body
                .deserialize::<(String, String)>()
                .map(|(d, j)| json!({"domain": d, "value": parse_json(&j)})),
            "LaunchCountsChanged" => body.deserialize::<HashMap<String, u32>>().map(|c| json!(c)),
            "ProfileChanged" => body.deserialize::<String>().map(|id| json!({"id": id})),
            // No arguments: the shell asks `ListOutputs` again.
            "MonitorsChanged" => Ok(json!({})),
            "PickRequested" => body
                .deserialize::<String>()
                .map(|token| json!({"token": token})),
            _ => continue,
        };
        match parsed {
            Ok(data) => {
                // Read from the borrowed value: no deep copy of the settings per change.
                if member.as_str() == "SettingsChanged"
                    && let Ok(s) = <Settings as serde::Deserialize>::deserialize(&data)
                {
                    settings.send_replace(Some(s));
                }
                let out = Outbound::Event {
                    event: member.to_string(),
                    data,
                };
                let sent = tokio::select! { _ = stop.cancelled() => return, r = tx.send(out) => r };
                if sent.is_err() {
                    return;
                }
            }
            Err(e) => eprintln!("uqctl bridge: bad {member} signal: {e}"),
        }
    }
}

fn parse_json(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}

/// The origin uqd's history records for the shell's writes.
const ORIGIN: &str = "shell";

#[cfg(test)]
mod tests;
