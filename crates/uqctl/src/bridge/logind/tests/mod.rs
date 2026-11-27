use super::*;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::time::Instant;
use uq_core::test_bus::{Bus, connect, private_bus};

const SESSION_PATH: &str = "/org/freedesktop/login1/session/_31";
const USER_PATH: &str = "/org/freedesktop/login1/user/_1000";
const DISPLAY_PATH: &str = "/org/freedesktop/login1/session/_32";

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
    /// logind's InhibitDelayMaxUSec; `None` makes reading it fail.
    delay_usec: Option<u64>,
}

impl Default for Fake {
    fn default() -> Fake {
        Fake {
            ends: Vec::new(),
            inhibit_args: Vec::new(),
            hints: Vec::new(),
            by_pid_fails: false,
            get_session_ids: Vec::new(),
            get_user_uids: Vec::new(),
            no_user: false,
            display: None,
            inhibit_fails: 0,
            hint_fails: false,
            hint_hangs: false,
            hint_calls: 0,
            // 2.5 s: a 2 s fallback, so the timing tests stay short.
            delay_usec: Some(TEST_DELAY_USEC),
        }
    }
}

const TEST_DELAY_USEC: u64 = 2_500_000;
/// The fallback with the default fake.
const TEST_FALLBACK_MS: u64 = 2000;

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

    #[zbus(property, name = "InhibitDelayMaxUSec")]
    fn inhibit_delay_max_usec(&self) -> zbus::fdo::Result<u64> {
        self.0
            .lock()
            .unwrap()
            .delay_usec
            .ok_or_else(|| zbus::fdo::Error::Failed("no property".into()))
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
    let c = zbus::connection::Builder::address(bus.address.as_str())
        .name(DEST)
        .serve_at(PATH, FakeManager(shared.clone()))
        .serve_at(SESSION_PATH, FakeSession(shared.clone()))
        .serve_at(DISPLAY_PATH, FakeSession(shared.clone()))
        .serve_at(USER_PATH, FakeUser(shared.clone()))
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
    tokio::spawn(logind.clone().run(connect(&bus).await, stop.clone()));
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

mod session;
mod sleep;
