//! logind on the system bus (spec §2.7): the sleep-delay inhibitor, `PrepareForSleep`, the
//! session's `Lock` signal, `SetLockedHint` and the `Lock` call.
//!
//! The bridge holds a delay inhibitor at all times. On `PrepareForSleep(true)` it reports the
//! event and keeps the inhibitor until the shell says the lock is secure (`SleepLockReady`), or
//! until `SLEEP_FALLBACK_MS` passes. On `PrepareForSleep(false)` it takes a fresh one. A failed
//! `Inhibit` is retried every `INHIBIT_RETRY` while no inhibitor is held and no sleep is pending.
//!
//! `hint_loop` forwards the latest lock state from the relay in `mod.rs` to `SetLockedHint`,
//! off the relay's path, so a logind that stops answering never delays a `LockState` event.

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use zbus::message::Type as MsgType;
use zbus::zvariant::{OwnedFd, OwnedObjectPath};
use zbus::{Connection, MatchRule, MessageStream};

const DEST: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
const USER: &str = "org.freedesktop.login1.User";

/// How long a pending sleep may wait for `SleepLockReady` before the inhibitor is dropped anyway.
pub const SLEEP_FALLBACK_MS: u64 = 2000;
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
            self.send_hint(&conn, &path, locked).await;
        }
    }

    /// Send the latest state in `hints` to `SetLockedHint`: once the session is found, then on
    /// every change. A slow call only delays this loop; states that arrive meanwhile collapse
    /// into the latest. Ends when the sender is dropped.
    pub async fn hint_loop(self: Arc<Self>, mut hints: watch::Receiver<Option<bool>>) {
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
                self.send_hint(&conn, &path, locked).await;
            }
            if hints.changed().await.is_err() {
                return;
            }
        }
    }

    async fn send_hint(&self, conn: &Connection, path: &OwnedObjectPath, locked: bool) {
        let r = conn
            .call_method(Some(DEST), path, Some(SESSION), "SetLockedHint", &(locked,))
            .await;
        match r {
            Ok(_) => {
                self.hint_fail.ok();
            }
            Err(e) => {
                self.hint_fail.fail(format_args!(
                    "SetLockedHint failed ({e}); will try on the next change"
                ));
            }
        }
    }

    /// Connect to the system bus (`addr`, or the default system bus) and run the watch.
    pub async fn connect_and_run(self: Arc<Self>, addr: Option<String>, stop: CancellationToken) {
        let conn = match addr {
            Some(a) => match zbus::connection::Builder::address(a.as_str()) {
                Ok(b) => b.build().await,
                Err(e) => Err(e),
            },
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
            Ok(r) => match MessageStream::for_match_rule(r, &conn, Some(16)).await {
                Ok(s) => s,
                Err(e) => return self.warn(e),
            },
            Err(e) => return self.warn(e),
        };
        let mut locks = None;
        match find_session(&conn, own_uid(), std::env::var("XDG_SESSION_ID").ok()).await {
            Some(path) => {
                if let Ok(r) = signal_rule(path.as_str(), SESSION, "Lock") {
                    locks = MessageStream::for_match_rule(r, &conn, Some(16)).await.ok();
                }
                self.session.send_replace(Some((conn.clone(), path)));
            }
            None => self.warn("no session"),
        }
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
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(SLEEP_FALLBACK_MS)).await;
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
    Ok(MatchRule::builder()
        .msg_type(MsgType::Signal)
        .sender(DEST)?
        .path(path.to_owned())?
        .interface(iface.to_owned())?
        .member(member.to_owned())?
        .build())
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
        .ok()?
        .path(user)
        .ok()?
        .build()
        .await
        .ok()?;
    let iface = zbus::names::InterfaceName::from_static_str(USER).ok()?;
    let value = props.get(iface, "Display").await.ok()?;
    let (_id, path) = <(String, OwnedObjectPath)>::try_from(value).ok()?;
    (path.as_str() != "/").then_some(path)
}

/// Our uid: the owner of `/proc/self`.
fn own_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};
    use std::time::Instant;

    const SESSION_PATH: &str = "/org/freedesktop/login1/session/_31";
    const USER_PATH: &str = "/org/freedesktop/login1/user/_1000";
    const DISPLAY_PATH: &str = "/org/freedesktop/login1/session/_32";

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
            .prefix("uql")
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

    #[derive(Default)]
    struct Fake {
        /// Our end of each inhibitor handed out; it reads EOF once the bridge drops its fd.
        ends: Vec<UnixStream>,
        inhibit_args: Vec<(String, String, String, String)>,
        hints: Vec<bool>,
        by_pid_fails: bool,
        get_session_ids: Vec<String>,
        /// Uids asked for with GetUser.
        get_user_uids: Vec<u32>,
        /// GetUser fails (no such user).
        no_user: bool,
        /// The user's Display property; `None` reads as ("", "/").
        display: Option<(String, String)>,
        /// Fail this many Inhibit calls before succeeding again.
        inhibit_fails: usize,
        hint_fails: bool,
        /// SetLockedHint records the hint, then never answers.
        hint_hangs: bool,
        hint_calls: usize,
    }
    type Shared = Arc<Mutex<Fake>>;

    struct FakeManager(Shared);

    #[zbus::interface(name = "org.freedesktop.login1.Manager")]
    impl FakeManager {
        fn inhibit(
            &self,
            what: String,
            who: String,
            why: String,
            mode: String,
        ) -> zbus::fdo::Result<OwnedFd> {
            let mut f = self.0.lock().unwrap();
            f.inhibit_args.push((what, who, why, mode));
            if f.inhibit_fails > 0 {
                f.inhibit_fails -= 1;
                return Err(zbus::fdo::Error::Failed("inhibit refused".into()));
            }
            let (ours, theirs) = UnixStream::pair().unwrap();
            f.ends.push(ours);
            Ok(OwnedFd::from(std::os::fd::OwnedFd::from(theirs)))
        }

        #[zbus(name = "GetSessionByPID")]
        fn get_session_by_pid(&self, _pid: u32) -> zbus::fdo::Result<OwnedObjectPath> {
            if self.0.lock().unwrap().by_pid_fails {
                return Err(zbus::fdo::Error::Failed("no session for pid".into()));
            }
            Ok(OwnedObjectPath::try_from(SESSION_PATH).unwrap())
        }

        fn get_session(&self, id: String) -> OwnedObjectPath {
            self.0.lock().unwrap().get_session_ids.push(id);
            OwnedObjectPath::try_from(SESSION_PATH).unwrap()
        }

        fn get_user(&self, uid: u32) -> zbus::fdo::Result<OwnedObjectPath> {
            let mut f = self.0.lock().unwrap();
            f.get_user_uids.push(uid);
            if f.no_user {
                return Err(zbus::fdo::Error::Failed("no such user".into()));
            }
            Ok(OwnedObjectPath::try_from(USER_PATH).unwrap())
        }
    }

    struct FakeUser(Shared);

    #[zbus::interface(name = "org.freedesktop.login1.User")]
    impl FakeUser {
        #[zbus(property)]
        fn display(&self) -> (String, OwnedObjectPath) {
            let (id, path) = self
                .0
                .lock()
                .unwrap()
                .display
                .clone()
                .unwrap_or_else(|| (String::new(), "/".into()));
            (id, OwnedObjectPath::try_from(path).unwrap())
        }
    }

    struct FakeSession(Shared);

    #[zbus::interface(name = "org.freedesktop.login1.Session")]
    impl FakeSession {
        async fn set_locked_hint(&self, locked: bool) -> zbus::fdo::Result<()> {
            let hang = {
                let mut f = self.0.lock().unwrap();
                f.hint_calls += 1;
                if f.hint_fails {
                    return Err(zbus::fdo::Error::Failed("hint refused".into()));
                }
                f.hints.push(locked);
                f.hint_hangs
            };
            if hang {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
    }

    async fn serve_fake(bus: &Bus, fake: Fake) -> (Connection, Shared) {
        let shared = Arc::new(Mutex::new(fake));
        let c = zbus::connection::Builder::address(bus.1.as_str())
            .unwrap()
            .name(DEST)
            .unwrap()
            .serve_at(PATH, FakeManager(shared.clone()))
            .unwrap()
            .serve_at(SESSION_PATH, FakeSession(shared.clone()))
            .unwrap()
            .serve_at(DISPLAY_PATH, FakeSession(shared.clone()))
            .unwrap()
            .serve_at(USER_PATH, FakeUser(shared.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        (c, shared)
    }

    struct Rig {
        _bus: Bus,
        fake_conn: Connection,
        fake: Shared,
        logind: Arc<Logind>,
        rx: mpsc::Receiver<Outbound>,
        stop: CancellationToken,
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            self.stop.cancel();
        }
    }

    /// A bus with a fake logind and a running watch that has taken its first inhibitor.
    async fn rig() -> Rig {
        let (tx, rx) = mpsc::channel(16);
        rig_with(Logind::new(tx), rx, Fake::default()).await
    }

    /// As `rig`, with a given handle (and its event receiver) and fake.
    async fn rig_with(logind: Arc<Logind>, rx: mpsc::Receiver<Outbound>, fake: Fake) -> Rig {
        let bus = private_bus();
        let (fake_conn, fake) = serve_fake(&bus, fake).await;
        let stop = CancellationToken::new();
        tokio::spawn(logind.clone().run(conn(&bus.1).await, stop.clone()));
        let r = Rig {
            _bus: bus,
            fake_conn,
            fake,
            logind,
            rx,
            stop,
        };
        r.wait_inhibits(1).await;
        r
    }

    impl Rig {
        async fn wait_inhibits(&self, n: usize) {
            let t = Instant::now();
            while self.fake.lock().unwrap().ends.len() < n {
                assert!(t.elapsed() < Duration::from_secs(5), "inhibit {n} in time");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        async fn wait_hints(&self, n: usize) {
            let t = Instant::now();
            while self.fake.lock().unwrap().hint_calls < n {
                assert!(t.elapsed() < Duration::from_secs(5), "hint {n} in time");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        fn attempts(&self) -> usize {
            self.fake.lock().unwrap().inhibit_args.len()
        }

        fn inhibits(&self) -> usize {
            self.fake.lock().unwrap().ends.len()
        }

        fn closed(&self, i: usize) -> bool {
            let f = self.fake.lock().unwrap();
            let mut s = &f.ends[i];
            s.set_nonblocking(true).unwrap();
            matches!(s.read(&mut [0u8; 1]), Ok(0))
        }

        /// Wait up to `within` for inhibitor `i` to be released.
        async fn wait_closed(&self, i: usize, within: Duration) -> bool {
            let t = Instant::now();
            while t.elapsed() < within {
                if self.closed(i) {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            self.closed(i)
        }

        async fn prepare(&self, start: bool) {
            self.fake_conn
                .emit_signal(None::<()>, PATH, MANAGER, "PrepareForSleep", &(start,))
                .await
                .unwrap();
        }

        async fn next(&mut self) -> (String, Value) {
            match tokio::time::timeout(Duration::from_secs(5), self.rx.recv())
                .await
                .expect("event in time")
                .expect("channel open")
            {
                Outbound::Event { event, data } => (event, data),
                _ => panic!("expected event"),
            }
        }
    }

    fn sleeping(b: bool) -> (String, Value) {
        ("PrepareForSleep".into(), json!({"sleeping": b}))
    }

    #[test]
    fn lock_argv_is_fixed() {
        assert_eq!(
            lock_argv(),
            [
                "systemctl",
                "--user",
                "start",
                "--no-block",
                "uq-lock.service"
            ]
        );
    }

    #[tokio::test]
    async fn lock_rejects_args() {
        assert!(lock(&[json!("x")]).await.is_err());
    }

    #[tokio::test]
    async fn inhibit_taken_at_start() {
        let r = rig().await;
        assert_eq!(
            r.fake.lock().unwrap().inhibit_args,
            vec![(
                "sleep".to_string(),
                "uQ".to_string(),
                "Lock the screen before sleep".to_string(),
                "delay".to_string()
            )]
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!r.closed(0), "the inhibitor is held");
    }

    #[tokio::test]
    async fn sleep_emits_event_and_ready_releases() {
        let mut r = rig().await;
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        assert!(!r.closed(0), "held until ready");
        r.logind.sleep_lock_ready();
        assert!(r.wait_closed(0, Duration::from_millis(500)).await);
    }

    #[tokio::test]
    async fn sleep_fallback_releases() {
        let mut r = rig().await;
        let t = Instant::now();
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!r.closed(0), "held before the fallback");
        assert!(r.wait_closed(0, Duration::from_millis(1500)).await);
        assert!(t.elapsed() < Duration::from_millis(3000));
    }

    #[tokio::test]
    async fn resume_retakes() {
        let mut r = rig().await;
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        r.logind.sleep_lock_ready();
        assert!(r.wait_closed(0, Duration::from_millis(500)).await);
        r.prepare(false).await;
        assert_eq!(r.next().await, sleeping(false));
        assert_eq!(r.inhibits(), 2, "a fresh inhibitor before the event");
        assert!(!r.closed(1));
        // The old fallback timer does not drop the fresh inhibitor.
        tokio::time::sleep(Duration::from_millis(SLEEP_FALLBACK_MS + 300)).await;
        assert!(!r.closed(1));
    }

    #[tokio::test]
    async fn stale_fallback_spares_next_sleeps_inhibitor() {
        let mut r = rig().await;
        let t1 = Instant::now();
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        r.logind.sleep_lock_ready();
        assert!(r.wait_closed(0, Duration::from_millis(500)).await);
        r.prepare(false).await;
        assert_eq!(r.next().await, sleeping(false));
        assert_eq!(r.inhibits(), 2);
        // Sleep again within sleep 1's fallback window.
        tokio::time::sleep(Duration::from_millis(600)).await;
        let t2 = Instant::now();
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        let fallback = Duration::from_millis(SLEEP_FALLBACK_MS);
        assert!(t2 - t1 < fallback, "sleep 2 starts inside sleep 1's window");

        // Past sleep 1's deadline (and before sleep 2's), the fresh inhibitor is still held.
        tokio::time::sleep_until((t1 + fallback + Duration::from_millis(200)).into()).await;
        assert!(t2.elapsed() < fallback);
        assert!(
            !r.closed(1),
            "sleep 1's timer must not release sleep 2's inhibitor"
        );
        // Sleep 2's own fallback releases it.
        assert!(r.wait_closed(1, Duration::from_millis(1500)).await);
        assert!(t2.elapsed() >= fallback - Duration::from_millis(50));
    }

    #[test]
    fn streak_logs_once_until_ok() {
        let s = Streak::default();
        assert!(s.fail("one"));
        assert!(!s.fail("two"));
        assert!(s.ok(), "ok ends a streak");
        assert!(!s.ok());
        assert!(s.fail("three"));
    }

    #[tokio::test]
    async fn failed_inhibit_is_retried_while_awake() {
        let (tx, rx) = mpsc::channel(16);
        let logind = Logind::with_retry(tx, Duration::from_millis(200));
        let fake = Fake {
            inhibit_fails: 2,
            ..Fake::default()
        };
        let mut r = rig_with(logind, rx, fake).await;
        assert_eq!(r.attempts(), 3, "two failures, then held");
        // The fake records the inhibitor before the bridge reads the reply: give it a moment.
        let t = Instant::now();
        while r.logind.inhibit_fail.0.load(Ordering::Relaxed) {
            assert!(
                t.elapsed() < Duration::from_secs(1),
                "success ended the streak"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(r.attempts(), 3, "no retry while held");

        // No retry while a sleep is pending, even though no inhibitor is held.
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        r.logind.sleep_lock_ready();
        assert!(r.wait_closed(0, Duration::from_millis(500)).await);
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(r.attempts(), 3, "no retry while sleeping");

        // A failed re-take on resume is retried too.
        r.fake.lock().unwrap().inhibit_fails = 1;
        r.prepare(false).await;
        assert_eq!(r.next().await, sleeping(false));
        r.wait_inhibits(2).await;
        assert_eq!(r.attempts(), 5);
        assert!(!r.closed(1));
    }

    #[tokio::test]
    async fn hint_loop_sends_latest_once_session_found() {
        let (tx, rx) = mpsc::channel(16);
        let logind = Logind::new(tx);
        // Two states before the session is known: only the latest is sent once it is found.
        let (hint_tx, hint_rx) = tokio::sync::watch::channel(None);
        hint_tx.send_replace(Some(true));
        hint_tx.send_replace(Some(false));
        let hints = tokio::spawn(logind.clone().hint_loop(hint_rx));
        let r = rig_with(logind, rx, Fake::default()).await;
        r.wait_hints(1).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(r.fake.lock().unwrap().hints, vec![false]);
        hint_tx.send_replace(Some(true));
        r.wait_hints(2).await;
        assert_eq!(r.fake.lock().unwrap().hints, vec![false, true]);
        hints.abort();
    }

    #[tokio::test]
    async fn lock_state_not_blocked_by_hung_hint() {
        let (tx, rx) = mpsc::channel(16);
        let logind = Logind::new(tx);
        let fake = Fake {
            hint_hangs: true,
            ..Fake::default()
        };
        let r = rig_with(logind.clone(), rx, fake).await;
        let (hint_tx, hint_rx) = tokio::sync::watch::channel(None);
        let hints = tokio::spawn(logind.hint_loop(hint_rx));
        let (out_tx, mut out_rx) = mpsc::channel(16);
        let (lock_tx, lock_rx) = mpsc::unbounded_channel();
        let relay = tokio::spawn(super::super::relay_locks(lock_rx, None, hint_tx, out_tx));
        let mut next = async || match tokio::time::timeout(Duration::from_secs(1), out_rx.recv())
            .await
            .expect("LockState promptly")
            .unwrap()
        {
            Outbound::Event { event, data } => (event, data),
            _ => panic!("expected event"),
        };

        lock_tx.send(true).unwrap();
        assert_eq!(next().await, ("LockState".into(), json!({"locked": true})));
        r.wait_hints(1).await; // SetLockedHint(true) is now stuck in logind
        lock_tx.send(false).unwrap();
        assert_eq!(next().await, ("LockState".into(), json!({"locked": false})));
        assert_eq!(
            r.fake.lock().unwrap().hint_calls,
            1,
            "still stuck on the first"
        );
        relay.abort();
        hints.abort();
    }

    #[tokio::test]
    async fn hint_failures_streak() {
        let r = rig().await;
        r.fake.lock().unwrap().hint_fails = true;
        r.logind.set_locked_hint(false).await;
        r.logind.set_locked_hint(true).await;
        assert!(
            r.logind.hint_fail.0.load(Ordering::Relaxed),
            "in a failure streak"
        );
        r.fake.lock().unwrap().hint_fails = false;
        r.logind.set_locked_hint(false).await;
        assert!(
            !r.logind.hint_fail.0.load(Ordering::Relaxed),
            "success ends it"
        );
        assert_eq!(r.fake.lock().unwrap().hints, vec![false]);
    }

    #[tokio::test]
    async fn ready_without_sleep_is_noop() {
        let r = rig().await;
        r.logind.sleep_lock_ready();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!r.closed(0));
    }

    #[tokio::test]
    async fn double_prepare_does_not_stack() {
        let mut r = rig().await;
        // Resume while the inhibitor is still held: no second inhibitor.
        r.prepare(false).await;
        assert_eq!(r.next().await, sleeping(false));
        assert_eq!(r.inhibits(), 1);

        r.prepare(true).await;
        r.prepare(true).await;
        assert_eq!(r.next().await, sleeping(true));
        assert_eq!(r.next().await, sleeping(true));
        r.logind.sleep_lock_ready();
        assert!(r.wait_closed(0, Duration::from_millis(500)).await);

        r.prepare(false).await;
        r.prepare(false).await;
        assert_eq!(r.next().await, sleeping(false));
        assert_eq!(r.next().await, sleeping(false));
        assert_eq!(r.inhibits(), 2, "one fresh inhibitor, not two");
        assert!(!r.closed(1));
    }

    #[tokio::test]
    async fn lock_signal_emits_lock_requested() {
        let mut r = rig().await;
        r.fake_conn
            .emit_signal(None::<()>, SESSION_PATH, SESSION, "Lock", &())
            .await
            .unwrap();
        assert_eq!(r.next().await, ("LockRequested".into(), json!({})));
    }

    #[tokio::test]
    async fn run_uses_display_session_without_pid() {
        let (tx, rx) = mpsc::channel(16);
        let fake = Fake {
            by_pid_fails: true,
            display: Some(("2".into(), DISPLAY_PATH.into())),
            ..Fake::default()
        };
        let mut r = rig_with(Logind::new(tx), rx, fake).await;
        // The bridge is subscribed by the time it takes its inhibitor (rig_with waits for it).
        r.fake_conn
            .emit_signal(None::<()>, DISPLAY_PATH, SESSION, "Lock", &())
            .await
            .unwrap();
        assert_eq!(r.next().await, ("LockRequested".into(), json!({})));
        r.logind.set_locked_hint(true).await;
        assert_eq!(r.fake.lock().unwrap().hints, vec![true]);
        assert!(!r.logind.warned.load(Ordering::Relaxed), "session found");
    }

    #[tokio::test]
    async fn locked_hint_forwarded() {
        let r = rig().await;
        r.logind.set_locked_hint(true).await;
        r.logind.set_locked_hint(false).await;
        assert_eq!(r.fake.lock().unwrap().hints, vec![true, false]);
    }

    #[tokio::test]
    async fn session_by_pid_comes_first() {
        let bus = private_bus();
        let fake = Fake {
            display: Some(("2".into(), DISPLAY_PATH.into())),
            ..Fake::default()
        };
        let (_fake_conn, shared) = serve_fake(&bus, fake).await;
        let c = conn(&bus.1).await;
        assert_eq!(
            find_session(&c, Some(1000), Some("7".into()))
                .await
                .unwrap()
                .as_str(),
            SESSION_PATH
        );
        let f = shared.lock().unwrap();
        assert!(f.get_user_uids.is_empty());
        assert!(f.get_session_ids.is_empty());
    }

    #[tokio::test]
    async fn session_falls_back_to_user_display() {
        let bus = private_bus();
        let fake = Fake {
            by_pid_fails: true,
            display: Some(("2".into(), DISPLAY_PATH.into())),
            ..Fake::default()
        };
        let (_fake_conn, shared) = serve_fake(&bus, fake).await;
        let c = conn(&bus.1).await;
        assert_eq!(
            find_session(&c, Some(1000), Some("7".into()))
                .await
                .unwrap()
                .as_str(),
            DISPLAY_PATH,
            "the display session wins over a (maybe stale) XDG_SESSION_ID"
        );
        let f = shared.lock().unwrap();
        assert_eq!(f.get_user_uids, vec![1000]);
        assert!(f.get_session_ids.is_empty());
    }

    #[tokio::test]
    async fn session_falls_back_to_xdg_session_id() {
        let bus = private_bus();
        // No display session ("/"), then no user at all: both fall through to XDG_SESSION_ID.
        for no_user in [false, true] {
            let fake = Fake {
                by_pid_fails: true,
                no_user,
                ..Fake::default()
            };
            let (fake_conn, shared) = serve_fake(&bus, fake).await;
            let c = conn(&bus.1).await;
            assert_eq!(
                find_session(&c, Some(1000), Some("7".into()))
                    .await
                    .unwrap()
                    .as_str(),
                SESSION_PATH
            );
            assert_eq!(shared.lock().unwrap().get_user_uids, vec![1000]);
            assert_eq!(shared.lock().unwrap().get_session_ids, vec!["7"]);
            assert_eq!(find_session(&c, Some(1000), None).await, None);
            // Without a uid the user step is skipped.
            assert_eq!(
                find_session(&c, None, Some("8".into()))
                    .await
                    .unwrap()
                    .as_str(),
                SESSION_PATH
            );
            assert_eq!(shared.lock().unwrap().get_user_uids, vec![1000, 1000]);
            drop(c);
            fake_conn.release_name(DEST).await.unwrap();
        }
    }

    #[tokio::test]
    async fn no_logind_degrades() {
        // A bus with no logind on it.
        let bus = private_bus();
        let (tx, _rx) = mpsc::channel(16);
        let logind = Logind::new(tx);
        let stop = CancellationToken::new();
        let task = tokio::spawn(logind.clone().run(conn(&bus.1).await, stop.clone()));
        tokio::time::sleep(Duration::from_millis(200)).await;
        logind.sleep_lock_ready();
        tokio::time::timeout(Duration::from_secs(2), logind.set_locked_hint(true))
            .await
            .expect("set_locked_hint answers");
        assert!(logind.warned.load(Ordering::Relaxed));
        stop.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("watch stops on cancel")
            .unwrap();

        // No system bus at all: the watch gives up, and the calls still answer.
        let (tx, _rx) = mpsc::channel(16);
        let logind = Logind::new(tx);
        tokio::time::timeout(
            Duration::from_secs(2),
            logind
                .clone()
                .connect_and_run(Some("unix:path=/tmp/uq-no-such-bus".into()), stop.clone()),
        )
        .await
        .expect("connect fails fast");
        assert!(logind.warned.load(Ordering::Relaxed));
        logind.sleep_lock_ready();
        logind.set_locked_hint(false).await;
    }
}
