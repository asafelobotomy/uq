//! logind on the system bus (spec §2.7): the sleep-delay inhibitor, `PrepareForSleep`, the
//! session's `Lock` signal, `SetLockedHint` and the `Lock` call.
//!
//! The bridge holds a delay inhibitor at all times. On `PrepareForSleep(true)` it reports the
//! event and keeps the inhibitor until the shell says the lock is secure (`SleepLockReady`), or
//! until the fallback passes: logind's own `InhibitDelayMaxUSec` less `FALLBACK_MARGIN_MS`
//! (`fallback_ms`), so a slow lock still beats the sleep. On `PrepareForSleep(false)` it takes a
//! fresh one. A failed
//! `Inhibit` is retried every `INHIBIT_RETRY` while no inhibitor is held and no sleep is pending.
//!
//! `hint_loop` forwards the latest lock state from the relay in `mod.rs` to `SetLockedHint`,
//! off the relay's path, so a logind that stops answering never delays a `LockState` event.
//! The shell's idle state (`IdleHint`) goes to `SetIdleHint` the same way, so schedulers such as
//! scdr, which read the session's hints, know when nobody is at the desktop.

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use zbus::message::Type as MsgType;
use zbus::wire::{OwnedFd, OwnedObjectPath};
use zbus::{Connection, MatchRule, MessageStream};

const DEST: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
const USER: &str = "org.freedesktop.login1.User";

/// The shortest wait for `SleepLockReady` before the inhibitor is dropped anyway.
pub const MIN_FALLBACK_MS: u64 = 2000;
/// The wait when logind's `InhibitDelayMaxUSec` can't be read (its default is 5 s).
pub const DEFAULT_FALLBACK_MS: u64 = 4500;
/// Released this long before logind would stop waiting on its own.
pub const FALLBACK_MARGIN_MS: u64 = 500;

/// How long a pending sleep waits for `SleepLockReady`, from logind's `InhibitDelayMaxUSec`
/// (`None`: unreadable): that limit less `FALLBACK_MARGIN_MS`, never below `MIN_FALLBACK_MS`.
pub fn fallback_ms(inhibit_delay_max_usec: Option<u64>) -> u64 {
    match inhibit_delay_max_usec {
        Some(us) => (us / 1000)
            .saturating_sub(FALLBACK_MARGIN_MS)
            .max(MIN_FALLBACK_MS),
        None => DEFAULT_FALLBACK_MS,
    }
}
/// How often a failed `Inhibit` is tried again while awake.
pub const INHIBIT_RETRY: Duration = Duration::from_secs(30);

/// The `Lock` call's command: a fixed argv, run directly, never through a shell. `--no-block`
/// queues the start and returns, so a stuck job never holds the caller (or `Session.starting`).
pub fn lock_argv() -> [&'static str; 5] {
    [
        "systemctl",
        "--user",
        "start",
        "--no-block",
        "uq-lock.service",
    ]
}

/// The bridge's `Lock` call: queue a start of the lock unit.
pub async fn lock(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Lock takes no arguments".into());
    }
    let [prog, rest @ ..] = lock_argv();
    let status = tokio::process::Command::new(prog)
        .args(rest)
        .stdin(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("{prog}: {e}"))?;
    if status.success() {
        Ok(Value::Null)
    } else {
        Err(format!("{prog} failed: {status}"))
    }
}

#[derive(Default)]
struct State {
    /// The held delay inhibitor; dropping it releases the inhibitor.
    fd: Option<OwnedFd>,
    /// Between `PrepareForSleep(true)` and `PrepareForSleep(false)`.
    sleeping: bool,
    /// Bumped on each new sleep, so a stale fallback timer does nothing.
    epoch: u64,
}

/// A run of failures of one kind: logged when it starts, not again until a success ends it.
#[derive(Default)]
struct Streak(AtomicBool);

impl Streak {
    /// Record a failure; log `msg` if it starts a streak. Returns whether it logged.
    fn fail(&self, msg: impl std::fmt::Display) -> bool {
        let first = !self.0.swap(true, Ordering::Relaxed);
        if first {
            eprintln!("uqctl bridge: {msg}");
        }
        first
    }

    /// Record a success. Returns whether it ended a streak.
    fn ok(&self) -> bool {
        self.0.swap(false, Ordering::Relaxed)
    }
}

/// The bridge's handle on logind. Calls on it answer whether or not logind is reachable.
pub struct Logind {
    state: Mutex<State>,
    tx: mpsc::Sender<Outbound>,
    warned: AtomicBool,
    inhibit_fail: Streak,
    hint_fail: Streak,
    retry: Duration,
    /// The sleep fallback in ms (`fallback_ms`), read from logind when the watch starts.
    fallback: std::sync::atomic::AtomicU64,
    /// Our session object, once found.
    session: watch::Sender<Option<(Connection, OwnedObjectPath)>>,
}

impl Logind {
    pub fn new(tx: mpsc::Sender<Outbound>) -> Arc<Logind> {
        Logind::with_retry(tx, INHIBIT_RETRY)
    }

    /// As `new`, retrying a failed `Inhibit` every `retry` (tests use a short one).
    fn with_retry(tx: mpsc::Sender<Outbound>, retry: Duration) -> Arc<Logind> {
        Arc::new(Logind {
            state: Mutex::default(),
            tx,
            warned: AtomicBool::new(false),
            inhibit_fail: Streak::default(),
            hint_fail: Streak::default(),
            retry,
            fallback: std::sync::atomic::AtomicU64::new(DEFAULT_FALLBACK_MS),
            session: watch::Sender::new(None),
        })
    }

    /// Log a logind problem to stderr, once per bridge.
    fn warn(&self, what: impl std::fmt::Display) {
        if !self.warned.swap(true, Ordering::Relaxed) {
            eprintln!("uqctl bridge: logind unavailable ({what}); sleep and lock hooks are off");
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `SleepLockReady`: release the inhibitor if a sleep is pending; otherwise do nothing.
    pub fn sleep_lock_ready(&self) {
        let mut s = self.state();
        if s.sleeping {
            s.fd = None;
        }
    }

    /// One direct `SetLockedHint` call (tests). Without a session this does nothing. The bridge
    /// goes through `hint_loop` instead, so a stuck call never holds up a caller.
    #[cfg(test)]
    pub async fn set_locked_hint(&self, locked: bool) {
        let session = self.session.borrow().clone();
        if let Some((conn, path)) = session {
            self.send_bool(&conn, &path, "SetLockedHint", locked).await;
        }
    }

    /// Send the latest state in `hints` to `SetLockedHint`: once the session is found, then on
    /// every change. A slow call only delays this loop; states that arrive meanwhile collapse
    /// into the latest. Ends when the sender is dropped.
    pub async fn hint_loop(self: Arc<Self>, hints: watch::Receiver<Option<bool>>) {
        self.bool_hint_loop("SetLockedHint", hints).await;
    }

    /// The same for the shell's idle state and `SetIdleHint`.
    pub async fn idle_loop(self: Arc<Self>, hints: watch::Receiver<Option<bool>>) {
        self.bool_hint_loop("SetIdleHint", hints).await;
    }

    async fn bool_hint_loop(&self, method: &'static str, mut hints: watch::Receiver<Option<bool>>) {
        let mut session = self.session.subscribe();
        let Ok(s) = session.wait_for(Option::is_some).await.map(|s| s.clone()) else {
            return;
        };
        let Some((conn, path)) = s else { return };
        loop {
            let Ok(locked) = hints.wait_for(Option::is_some).await.map(|h| *h) else {
                return;
            };
            if let Some(locked) = locked {
                self.send_bool(&conn, &path, method, locked).await;
            }
            if hints.changed().await.is_err() {
                return;
            }
        }
    }

    async fn send_bool(&self, conn: &Connection, path: &OwnedObjectPath, method: &str, on: bool) {
        let r = conn
            .call_method(Some(DEST), path, Some(SESSION), method, &(on,))
            .await;
        match r {
            Ok(_) => {
                self.hint_fail.ok();
            }
            Err(e) => {
                self.hint_fail.fail(format_args!(
                    "{method} failed ({e}); will try on the next change"
                ));
            }
        }
    }

    /// Connect to the system bus (`addr`, or the default system bus) and run the watch.
    pub async fn connect_and_run(self: Arc<Self>, addr: Option<String>, stop: CancellationToken) {
        let conn = match addr {
            Some(a) => zbus::connection::Builder::address(a.as_str()).build().await,
            None => Connection::system().await,
        };
        match conn {
            Ok(c) => self.run(c, stop).await,
            Err(e) => self.warn(format!("system bus: {e}")),
        }
    }

    /// Subscribe to `PrepareForSleep` and the session's `Lock`, take the inhibitor, then relay
    /// signals until `stop` is cancelled.
    pub async fn run(self: Arc<Self>, conn: Connection, stop: CancellationToken) {
        let rule = signal_rule(PATH, MANAGER, "PrepareForSleep");
        let mut sleeps = match rule {
            Ok(r) => match MessageStream::for_match_rule(r, &conn, std::num::NonZeroUsize::new(16))
                .await
            {
                Ok(s) => s,
                Err(e) => return self.warn(e),
            },
            Err(e) => return self.warn(e),
        };
        let mut locks = None;
        match find_session(&conn, own_uid(), std::env::var("XDG_SESSION_ID").ok()).await {
            Some(path) => {
                if let Ok(r) = signal_rule(path.as_str(), SESSION, "Lock") {
                    locks =
                        MessageStream::for_match_rule(r, &conn, std::num::NonZeroUsize::new(16))
                            .await
                            .ok();
                }
                self.session.send_replace(Some((conn.clone(), path)));
            }
            None => self.warn("no session"),
        }
        let delay = inhibit_delay_max(&conn).await;
        self.fallback.store(fallback_ms(delay), Ordering::Relaxed);
        // Take the inhibitor last, so the subscriptions above are live once logind sees it.
        self.inhibit(&conn, "at start").await;

        let mut retry =
            tokio::time::interval_at(tokio::time::Instant::now() + self.retry, self.retry);
        retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let lock_next = async {
                match locks.as_mut() {
                    Some(s) => s.next().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = stop.cancelled() => return,
                m = sleeps.next() => {
                    let Some(m) = m else { return };
                    let Ok(m) = m else { continue };
                    let Ok(start) = m.body().deserialize::<bool>() else { continue };
                    if !self.prepare_for_sleep(&conn, start, &stop).await {
                        return;
                    }
                }
                m = lock_next => {
                    match m {
                        None => { locks = None; continue }
                        Some(Err(_)) => continue,
                        Some(Ok(_)) => {}
                    }
                    if !self.emit("LockRequested", json!({}), &stop).await {
                        return;
                    }
                }
                _ = retry.tick() => {
                    let need = {
                        let s = self.state();
                        s.fd.is_none() && !s.sleeping
                    };
                    if need {
                        self.inhibit(&conn, "on retry").await;
                    }
                }
            }
        }
    }

    async fn prepare_for_sleep(
        self: &Arc<Self>,
        conn: &Connection,
        start: bool,
        stop: &CancellationToken,
    ) -> bool {
        if start {
            let epoch = {
                let mut s = self.state();
                if s.sleeping {
                    None
                } else {
                    s.sleeping = true;
                    s.epoch += 1;
                    Some(s.epoch)
                }
            };
            // One fallback per sleep: a repeated PrepareForSleep(true) does not stack timers.
            if let Some(epoch) = epoch {
                // Weak: a pending timer must not keep the event sender (and so stdout) open.
                let me = Arc::downgrade(self);
                let wait = Duration::from_millis(self.fallback.load(Ordering::Relaxed));
                tokio::spawn(async move {
                    tokio::time::sleep(wait).await;
                    let Some(me) = me.upgrade() else { return };
                    let mut s = me.state();
                    if s.sleeping && s.epoch == epoch {
                        s.fd = None;
                    }
                });
            }
        } else {
            let need = {
                let mut s = self.state();
                s.sleeping = false;
                s.fd.is_none()
            };
            if need {
                self.inhibit(conn, "after resume").await;
            }
        }
        self.emit("PrepareForSleep", json!({"sleeping": start}), stop)
            .await
    }

    /// Take the delay inhibitor and hold its fd. `when` says which attempt this is, for the log.
    async fn inhibit(&self, conn: &Connection, when: &str) {
        let reply = conn
            .call_method(
                Some(DEST),
                PATH,
                Some(MANAGER),
                "Inhibit",
                &("sleep", "uQ", "Lock the screen before sleep", "delay"),
            )
            .await;
        match reply.and_then(|m| m.body().deserialize::<OwnedFd>()) {
            Ok(fd) => {
                self.state().fd = Some(fd);
                if self.inhibit_fail.ok() {
                    eprintln!("uqctl bridge: sleep inhibitor taken {when}");
                }
            }
            Err(e) => {
                let secs = self.retry.as_secs_f32();
                self.inhibit_fail.fail(format_args!(
                    "logind Inhibit failed {when} ({e}); sleep will not wait for the lock \
                     screen; retrying every {secs} s"
                ));
            }
        }
    }

    async fn emit(&self, event: &str, data: Value, stop: &CancellationToken) -> bool {
        let out = Outbound::Event {
            event: event.into(),
            data,
        };
        tokio::select! { _ = stop.cancelled() => false, r = self.tx.send(out) => r.is_ok() }
    }
}

/// A match rule for `member` signals from logind on `path`.
fn signal_rule(path: &str, iface: &str, member: &str) -> zbus::Result<MatchRule<'static>> {
    MatchRule::builder()
        .msg_type(MsgType::Signal)
        .sender(DEST)
        .path(path.to_owned())
        .interface(iface.to_owned())
        .member(member.to_owned())
        .build()
}

/// Our session. The bridge runs in a user service, outside any session scope, so the pid lookup
/// usually fails; then the user's display session (logind's pick of the graphical session); then
/// `XDG_SESSION_ID`, which may be missing or stale.
async fn find_session(
    conn: &Connection,
    uid: Option<u32>,
    xdg_id: Option<String>,
) -> Option<OwnedObjectPath> {
    let by_pid = conn
        .call_method(
            Some(DEST),
            PATH,
            Some(MANAGER),
            "GetSessionByPID",
            &(std::process::id(),),
        )
        .await;
    if let Ok(p) = by_pid.and_then(|m| m.body().deserialize::<OwnedObjectPath>()) {
        return Some(p);
    }
    if let Some(uid) = uid
        && let Some(p) = display_session(conn, uid).await
    {
        return Some(p);
    }
    let id = xdg_id.filter(|s| !s.is_empty())?;
    let reply = conn
        .call_method(Some(DEST), PATH, Some(MANAGER), "GetSession", &(id,))
        .await
        .ok()?;
    reply.body().deserialize::<OwnedObjectPath>().ok()
}

/// The `Display` session of user `uid`, if logind has one ("/" means none).
async fn display_session(conn: &Connection, uid: u32) -> Option<OwnedObjectPath> {
    let reply = conn
        .call_method(Some(DEST), PATH, Some(MANAGER), "GetUser", &(uid,))
        .await
        .ok()?;
    let user = reply.body().deserialize::<OwnedObjectPath>().ok()?;
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(DEST)
        .path(user)
        .build()
        .await
        .ok()?;
    let iface = zbus::names::InterfaceName::from_static_str(USER).ok()?;
    let value = props.get(iface, "Display").await.ok()?;
    let (_id, path) = <(String, OwnedObjectPath)>::try_from(value).ok()?;
    (path.as_str() != "/").then_some(path)
}

/// logind's `InhibitDelayMaxUSec`: how long it waits for delay inhibitors before sleeping.
async fn inhibit_delay_max(conn: &Connection) -> Option<u64> {
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(DEST)
        .path(PATH)
        .build()
        .await
        .ok()?;
    let iface = zbus::names::InterfaceName::from_static_str(MANAGER).ok()?;
    let value = props.get(iface, "InhibitDelayMaxUSec").await.ok()?;
    u64::try_from(value).ok()
}

/// Our uid: the owner of `/proc/self`.
fn own_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

#[cfg(test)]
mod tests;
