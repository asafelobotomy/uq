//! logind on the system bus (spec §2.7): the sleep-delay inhibitor, `PrepareForSleep`, the
//! session's `Lock` signal, `SetLockedHint` and the `Lock` call.
//!
//! The bridge holds a delay inhibitor at all times. On `PrepareForSleep(true)` it reports the
//! event and keeps the inhibitor until the shell says the lock is secure (`SleepLockReady`), or
//! until `SLEEP_FALLBACK_MS` passes. On `PrepareForSleep(false)` it takes a fresh one.
//!
//! Task 2 reaches `set_locked_hint` through the `Arc<Logind>` handle that `mod.rs` builds and
//! passes to `handle`.

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use zbus::message::Type as MsgType;
use zbus::zvariant::{OwnedFd, OwnedObjectPath};
use zbus::{Connection, MatchRule, MessageStream};

const DEST: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

/// How long a pending sleep may wait for `SleepLockReady` before the inhibitor is dropped anyway.
pub const SLEEP_FALLBACK_MS: u64 = 2000;

/// The `Lock` call's command: a fixed argv, run directly, never through a shell.
pub fn lock_argv() -> [&'static str; 4] {
    ["systemctl", "--user", "start", "uq-lock.service"]
}

/// The bridge's `Lock` call: start the lock unit.
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
    /// Our session object, once found.
    session: Option<(Connection, OwnedObjectPath)>,
}

/// The bridge's handle on logind. Calls on it answer whether or not logind is reachable.
pub struct Logind {
    state: Mutex<State>,
    tx: mpsc::Sender<Outbound>,
    warned: AtomicBool,
}

impl Logind {
    pub fn new(tx: mpsc::Sender<Outbound>) -> Arc<Logind> {
        Arc::new(Logind {
            state: Mutex::default(),
            tx,
            warned: AtomicBool::new(false),
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

    /// Forward the lock state to logind's `SetLockedHint`. Without a session this does nothing.
    // Called by the lock-state watcher (slice 5, Task 2).
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn set_locked_hint(&self, locked: bool) {
        let Some((conn, path)) = self.state().session.clone() else {
            return;
        };
        if let Err(e) = conn
            .call_method(
                Some(DEST),
                &path,
                Some(SESSION),
                "SetLockedHint",
                &(locked,),
            )
            .await
        {
            eprintln!("uqctl bridge: SetLockedHint failed: {e}");
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
        match find_session(&conn, std::env::var("XDG_SESSION_ID").ok()).await {
            Some(path) => {
                if let Ok(r) = signal_rule(path.as_str(), SESSION, "Lock") {
                    locks = MessageStream::for_match_rule(r, &conn, Some(16)).await.ok();
                }
                self.state().session = Some((conn.clone(), path));
            }
            None => self.warn("no session"),
        }
        // Take the inhibitor last, so the subscriptions above are live once logind sees it.
        self.inhibit(&conn).await;

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
                self.inhibit(conn).await;
            }
        }
        self.emit("PrepareForSleep", json!({"sleeping": start}), stop)
            .await
    }

    /// Take the delay inhibitor and hold its fd.
    async fn inhibit(&self, conn: &Connection) {
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
            Ok(fd) => self.state().fd = Some(fd),
            Err(e) => self.warn(format!("Inhibit: {e}")),
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

/// Our session: by our pid, else by `XDG_SESSION_ID`.
async fn find_session(conn: &Connection, xdg_id: Option<String>) -> Option<OwnedObjectPath> {
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
    let id = xdg_id.filter(|s| !s.is_empty())?;
    let reply = conn
        .call_method(Some(DEST), PATH, Some(MANAGER), "GetSession", &(id,))
        .await
        .ok()?;
    reply.body().deserialize::<OwnedObjectPath>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};
    use std::time::Instant;

    const SESSION_PATH: &str = "/org/freedesktop/login1/session/_31";

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
    }
    type Shared = Arc<Mutex<Fake>>;

    struct FakeManager(Shared);

    #[zbus::interface(name = "org.freedesktop.login1.Manager")]
    impl FakeManager {
        fn inhibit(&self, what: String, who: String, why: String, mode: String) -> OwnedFd {
            let (ours, theirs) = UnixStream::pair().unwrap();
            let mut f = self.0.lock().unwrap();
            f.inhibit_args.push((what, who, why, mode));
            f.ends.push(ours);
            OwnedFd::from(std::os::fd::OwnedFd::from(theirs))
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
    }

    struct FakeSession(Shared);

    #[zbus::interface(name = "org.freedesktop.login1.Session")]
    impl FakeSession {
        fn set_locked_hint(&self, locked: bool) {
            self.0.lock().unwrap().hints.push(locked);
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
        let bus = private_bus();
        let (fake_conn, fake) = serve_fake(&bus, Fake::default()).await;
        let (tx, rx) = mpsc::channel(16);
        let logind = Logind::new(tx);
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
            ["systemctl", "--user", "start", "uq-lock.service"]
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
    async fn locked_hint_forwarded() {
        let r = rig().await;
        r.logind.set_locked_hint(true).await;
        r.logind.set_locked_hint(false).await;
        assert_eq!(r.fake.lock().unwrap().hints, vec![true, false]);
    }

    #[tokio::test]
    async fn session_falls_back_to_xdg_session_id() {
        let bus = private_bus();
        let fake = Fake {
            by_pid_fails: true,
            ..Fake::default()
        };
        let (_fake_conn, shared) = serve_fake(&bus, fake).await;
        let c = conn(&bus.1).await;
        assert_eq!(
            find_session(&c, Some("7".into())).await.unwrap().as_str(),
            SESSION_PATH
        );
        assert_eq!(shared.lock().unwrap().get_session_ids, vec!["7"]);
        assert_eq!(find_session(&c, None).await, None);
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
