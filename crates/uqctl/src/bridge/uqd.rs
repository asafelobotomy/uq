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

pub const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
const IFACE: &str = "org.uq.Desktop1";
pub const DOWN: &str = "uqd is not running";
/// `Reload` can take about 12 s inside uqd (verify + reload); never wait forever.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_secs(5);
/// `ShellReady` returns at once in uqd; a watch loop never waits longer than this on it.
const READY_WAIT: Duration = Duration::from_secs(2);

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

/// Connect and subscribe to owner changes and uqd's signals.
async fn subscribe(addr: Option<&str>) -> Option<(Connection, Subs)> {
    let conn = connect(addr).await.ok()?;
    let owner = DBusProxy::new(&conn)
        .await
        .ok()?
        .receive_name_owner_changed()
        .await
        .ok()?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(NAME)
        .and_then(|b| b.path(PATH))
        .and_then(|b| b.interface(IFACE))
        .map(|b| b.build())
        .ok()?;
    let signals = MessageStream::for_match_rule(rule, &conn, Some(64))
        .await
        .ok()?;
    Some((conn, Subs { owner, signals }))
}

async fn connect(addr: Option<&str>) -> zbus::Result<Connection> {
    match addr {
        Some(a) => zbus::connection::Builder::address(a)?.build().await,
        None => Connection::session().await,
    }
}

async fn has_owner(conn: &Connection) -> bool {
    match DBusProxy::new(conn).await {
        Ok(p) => p
            .name_has_owner(NAME.try_into().expect("valid bus name"))
            .await
            .unwrap_or(false),
        Err(_) => false,
    }
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
                _ = poll.tick() => {
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
            "PickRequested" => body
                .deserialize::<String>()
                .map(|token| json!({"token": token})),
            _ => continue,
        };
        match parsed {
            Ok(data) => {
                if member.as_str() == "SettingsChanged"
                    && let Ok(s) = serde_json::from_value::<Settings>(data.clone())
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

/// A validated call, ready to send.
enum Request {
    GetTheme,
    SetTheme(String, String),
    ListThemes,
    ListThemeInfo,
    GetLayouts,
    SetLayout(String, String),
    GetSchedule,
    SetSchedule(String),
    Reload,
    GetSettings,
    SetSetting(String, String),
    GetLaunchCounts,
    RecordLaunch(String),
    ListProfiles,
    ApplyProfile(String),
    ProfileOff,
    PickDone(String, String),
    /// The bridge's own calls; the shell can't make them (`handle()` doesn't pass them on;
    /// `GetWelcome` reaches the shell only inside `Hello` and `WelcomeStatus`).
    GetSection(String),
    ShellReady,
    GetWelcome,
}

fn string_arg(args: &[Value], i: usize, what: &str) -> Result<String, String> {
    args.get(i)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("argument {} ({what}) must be a string", i + 1))
}

fn arity(call: &str, args: &[Value], n: usize) -> Result<(), String> {
    if args.len() == n {
        Ok(())
    } else {
        Err(format!("{call} takes {n} argument(s), got {}", args.len()))
    }
}

impl Request {
    fn parse(call: &str, args: &[Value]) -> Result<Request, String> {
        let r = match call {
            "GetTheme" => (0, Request::GetTheme),
            "ListThemes" => (0, Request::ListThemes),
            "ListThemeInfo" => (0, Request::ListThemeInfo),
            "GetLayouts" => (0, Request::GetLayouts),
            "GetSchedule" => (0, Request::GetSchedule),
            "Reload" => (0, Request::Reload),
            "GetSettings" => (0, Request::GetSettings),
            "GetLaunchCounts" => (0, Request::GetLaunchCounts),
            "ListProfiles" => (0, Request::ListProfiles),
            "ProfileOff" => (0, Request::ProfileOff),
            "ShellReady" => (0, Request::ShellReady),
            "GetWelcome" => (0, Request::GetWelcome),
            "GetSection" => {
                arity(call, args, 1)?;
                (1, Request::GetSection(string_arg(args, 0, "domain")?))
            }
            "SetTheme" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetTheme(
                        string_arg(args, 0, "name")?,
                        string_arg(args, 1, "variant")?,
                    ),
                )
            }
            "SetLayout" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetLayout(
                        string_arg(args, 0, "workspace")?,
                        string_arg(args, 1, "layout")?,
                    ),
                )
            }
            "SetSchedule" => {
                arity(call, args, 1)?;
                // The schedule is a JSON object (a pre-encoded JSON string is accepted too).
                let json = match &args[0] {
                    Value::String(s) => s.clone(),
                    v @ Value::Object(_) => v.to_string(),
                    _ => return Err("argument 1 (schedule) must be an object".into()),
                };
                (1, Request::SetSchedule(json))
            }
            "SetSetting" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetSetting(string_arg(args, 0, "key")?, args[1].to_string()),
                )
            }
            "RecordLaunch" => {
                arity(call, args, 1)?;
                (1, Request::RecordLaunch(string_arg(args, 0, "desktop id")?))
            }
            "PickDone" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::PickDone(
                        string_arg(args, 0, "token")?,
                        string_arg(args, 1, "outcome")?,
                    ),
                )
            }
            "ApplyProfile" => {
                arity(call, args, 1)?;
                (1, Request::ApplyProfile(string_arg(args, 0, "profile id")?))
            }
            other => return Err(format!("unknown call {other}")),
        };
        if r.0 == 0 {
            arity(call, args, 0)?;
        }
        Ok(r.1)
    }

    async fn send(self, conn: &Connection) -> Result<Value, String> {
        let proxy = zbus::Proxy::new(conn, NAME, PATH, IFACE)
            .await
            .map_err(describe)?;
        match self {
            Request::GetTheme => {
                let (n, v): (String, String) =
                    proxy.call("GetTheme", &()).await.map_err(describe)?;
                Ok(json!([n, v]))
            }
            Request::ListThemes => {
                let ids: Vec<String> = proxy.call("ListThemes", &()).await.map_err(describe)?;
                Ok(json!(ids))
            }
            Request::ListThemeInfo => {
                let s: String = proxy.call("ListThemeInfo", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad theme list: {e}"))
            }
            Request::GetLayouts => {
                let m: HashMap<String, String> =
                    proxy.call("GetLayouts", &()).await.map_err(describe)?;
                Ok(json!(m))
            }
            Request::GetSchedule => {
                let s: String = proxy.call("GetSchedule", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad schedule: {e}"))
            }
            Request::GetSettings => {
                let s: String = proxy.call("GetSettings", &()).await.map_err(describe)?;
                let v: Value =
                    serde_json::from_str(&s).map_err(|e| format!("bad settings: {e}"))?;
                Ok(if v.is_object() {
                    v
                } else {
                    Value::Object(Map::new())
                })
            }
            Request::GetLaunchCounts => {
                let m: HashMap<String, u32> =
                    proxy.call("GetLaunchCounts", &()).await.map_err(describe)?;
                Ok(json!(m))
            }
            Request::Reload => unit(proxy.call("Reload", &()).await),
            Request::SetTheme(a, b) => unit(proxy.call("SetTheme", &(a, b, ORIGIN)).await),
            Request::SetLayout(a, b) => unit(proxy.call("SetLayout", &(a, b, ORIGIN)).await),
            Request::SetSchedule(j) => unit(proxy.call("SetSchedule", &(j, ORIGIN)).await),
            Request::SetSetting(k, j) => unit(proxy.call("SetSetting", &(k, j, ORIGIN)).await),
            Request::PickDone(t, o) => unit(proxy.call("PickDone", &(t, o)).await),
            Request::RecordLaunch(id) => unit(proxy.call("RecordLaunch", &(id,)).await),
            // Profile writes take no origin: uqd journals them as `profile:<id>`.
            Request::ListProfiles => {
                let s: String = proxy.call("ListProfiles", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad profile list: {e}"))
            }
            Request::ApplyProfile(id) => {
                report(proxy.call("ApplyProfile", &(id,)).await.map_err(describe)?)
            }
            Request::ProfileOff => report(proxy.call("ProfileOff", &()).await.map_err(describe)?),
            Request::GetSection(d) => {
                let s: String = proxy.call("GetSection", &(d,)).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad section: {e}"))
            }
            Request::ShellReady => unit(proxy.call("ShellReady", &()).await),
            Request::GetWelcome => {
                let s: String = proxy.call("GetWelcome", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad welcome status: {e}"))
            }
        }
    }
}

/// uqd's profile report (`{active, kept, notices}`), parsed.
fn report(s: String) -> Result<Value, String> {
    serde_json::from_str(&s).map_err(|e| format!("bad profile report: {e}"))
}

fn unit(r: zbus::Result<()>) -> Result<Value, String> {
    r.map(|()| Value::Null).map_err(describe)
}

/// D-Bus error to the `err` text: uqd's own message when it gave one, "not running" when the
/// name has no owner.
fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, msg, _) => {
            if matches!(
                name.as_str(),
                "org.freedesktop.DBus.Error.ServiceUnknown"
                    | "org.freedesktop.DBus.Error.NameHasNoOwner"
            ) {
                DOWN.into()
            } else {
                msg.unwrap_or_else(|| name.to_string())
            }
        }
        zbus::Error::FDO(e) => match *e {
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
                DOWN.into()
            }
            other => other.to_string(),
        },
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_requests_check_arity() {
        assert!(Request::parse("ListProfiles", &[]).is_ok());
        assert!(Request::parse("ListProfiles", &[json!(1)]).is_err());
        assert!(Request::parse("ApplyProfile", &[json!("gaming")]).is_ok());
        assert!(Request::parse("ApplyProfile", &[]).is_err());
        assert!(Request::parse("ApplyProfile", &[json!(1)]).is_err());
        assert!(Request::parse("ApplyProfile", &[json!("a"), json!("b")]).is_err());
        assert!(Request::parse("ProfileOff", &[]).is_ok());
        assert!(Request::parse("ProfileOff", &[json!("x")]).is_err());
    }

    #[test]
    fn request_parse_accepts_get_section_and_shell_ready() {
        assert!(matches!(
            Request::parse("GetSection", &[json!("compat")]),
            Ok(Request::GetSection(d)) if d == "compat"
        ));
        assert!(Request::parse("GetSection", &[]).is_err());
        assert!(Request::parse("GetSection", &[json!(1)]).is_err());
        assert!(Request::parse("GetSection", &[json!("a"), json!("b")]).is_err());
        assert!(matches!(
            Request::parse("ShellReady", &[]),
            Ok(Request::ShellReady)
        ));
        assert!(Request::parse("ShellReady", &[json!("x")]).is_err());
    }

    #[test]
    fn pick_done_checks_arity() {
        assert!(Request::parse("PickDone", &[json!("t"), json!("click")]).is_ok());
        assert!(Request::parse("PickDone", &[]).is_err());
        assert!(Request::parse("PickDone", &[json!("t")]).is_err());
        assert!(Request::parse("PickDone", &[json!("t"), json!(1)]).is_err());
        assert!(Request::parse("PickDone", &[json!("t"), json!("x"), json!("y")]).is_err());
    }
}
