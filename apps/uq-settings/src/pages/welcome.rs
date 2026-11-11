//! Welcome, the first-run flow (Phase 5 spec §5.1): [`WelcomeState`] decides which steps show,
//! moves between them, marks Welcome done with uqd and drives the Apps and Fingerprint steps.
//! The Look, Wallpaper, Layouts and Displays steps use those pages' own models in QML, so every
//! change goes through the same uqd calls as the full pages.
//!
//! Welcome is marked done (`SetWelcomeDone`) as soon as it shows: closing the window at any step
//! counts as done (§5.1), and a restarted shell then never opens it a second time.
//!
//! State: `{flow, status, marked, monitors, reader, enroll, apps, bindle, setup_problem,
//! session_log, keybinds}`; `flow` is null until [`Op::Start`].

pub mod apps;
pub mod fingerprint;
pub mod steps;

use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::integration::scan::{Roots, scan_all};
use uq_core::welcome::{CURRENT_WELCOME_VERSION, session_log_path, setup_problem};

use self::fingerprint::{Enrolment, Reader};
use self::steps::{Context, Flow, Step};
use super::runner::{PageLogic, parse_json};
use super::sysbus::SystemBus;

/// What the page asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Refresh,
    /// Welcome is on show: work out the steps (once) and mark Welcome done (once uqd answers).
    /// `full` shows every step, as Settings → About → Show Welcome does.
    Start {
        full: bool,
    },
    Next,
    Back,
    Skip,
    /// Start bindle's app picker.
    OpenBindle,
    /// Enrol a finger (runs in the background; `enroll` in the state follows it).
    Enroll,
    CancelEnroll,
}

/// A keybind to know: the keys as people read them, what they do, and the bind in
/// `config/hypr/base.lua` that makes it so (checked by a test).
pub struct Keybind {
    pub keys: &'static str,
    pub does: &'static str,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "read by the test against base.lua")
    )]
    lua: &'static str,
}

/// The three on the Hello step (spec §5.1 step 1), then the rest the Done step lists.
pub const KEYBINDS: [Keybind; 10] = [
    Keybind {
        keys: "Super + Space",
        does: "Search for apps, files and settings",
        lua: r#"hl.bind(mainMod .. " + Space",   hl.dsp.global("uq:spotlight"))"#,
    },
    Keybind {
        keys: "Tap Super",
        does: "Open the app menu",
        lua: r#"hl.bind(mainMod .. " + SUPER_L", hl.dsp.global("uq:appmenu"), { release = true })"#,
    },
    Keybind {
        keys: "Super + A",
        does: "Open quick settings",
        lua: r#"hl.bind(mainMod .. " + A",       hl.dsp.global("uq:quick-settings"))"#,
    },
    Keybind {
        keys: "Super + Return",
        does: "Open a terminal",
        lua: r#"hl.bind(mainMod .. " + Return", hl.dsp.exec_cmd(terminal))"#,
    },
    Keybind {
        keys: "Super + Q",
        does: "Close the window",
        lua: r#"hl.bind(mainMod .. " + Q",      hl.dsp.window.close())"#,
    },
    Keybind {
        keys: "Super + N",
        does: "Show your notifications",
        lua: r#"hl.bind(mainMod .. " + N",       hl.dsp.global("uq:notif-centre"))"#,
    },
    Keybind {
        keys: "Super + L",
        does: "Lock the screen",
        lua: r#"hl.bind(mainMod .. " + L",       hl.dsp.global("uq:lock"))"#,
    },
    Keybind {
        keys: "Super + Escape",
        does: "Restart, shut down or log out",
        lua: r#"hl.bind(mainMod .. " + Escape",  hl.dsp.global("uq:power-menu"))"#,
    },
    Keybind {
        keys: "Super + 1 to 0",
        does: "Go to workspace 1 to 10",
        lua: r#"hl.bind(mainMod .. " + " .. key,         hl.dsp.focus({ workspace = i }))"#,
    },
    Keybind {
        keys: "Super + Shift + 1 to 0",
        does: "Move the window to workspace 1 to 10",
        lua: r#"hl.bind(mainMod .. " + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }))"#,
    },
];

fn keybinds_json() -> Value {
    let row = |k: &Keybind| json!({"keys": k.keys, "does": k.does});
    json!({
        "first": KEYBINDS[..3].iter().map(row).collect::<Vec<_>>(),
        "all": KEYBINDS.iter().map(row).collect::<Vec<_>>(),
    })
}

/// Starts a program detached (tests record the argv instead).
pub type Launcher = fn(&[&str]) -> std::io::Result<()>;

fn launch_detached(argv: &[&str]) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    std::process::Command::new(argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

/// Where the page looks, so tests can point it at a temporary tree.
pub struct Places {
    /// `$XDG_STATE_HOME` (for `uq/session.log`).
    pub state_home: PathBuf,
    /// The integration roots `uqctl integrations list` reads.
    pub roots: Roots,
    /// Where apps' programs are (`/usr/bin`).
    pub bin_dir: PathBuf,
}

impl Places {
    pub fn system() -> Places {
        let state_home = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
                    .join(".local/state")
            });
        Places {
            state_home,
            roots: Roots::system(),
            bin_dir: PathBuf::from("/usr/bin"),
        }
    }
}

struct Progress {
    flow: Option<Flow>,
    status: Option<Value>,
    marked: bool,
    monitors: usize,
    reader: Reader,
}

pub struct WelcomeState {
    conn: zbus::Connection,
    system: Arc<SystemBus>,
    places: Places,
    launch: Launcher,
    write: Mutex<()>,
    progress: StdMutex<Progress>,
    pub enrolment: Enrolment,
}

impl WelcomeState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with(conn, SystemBus::system(), Places::system(), launch_detached)
    }

    pub fn with(
        conn: zbus::Connection,
        system: SystemBus,
        places: Places,
        launch: Launcher,
    ) -> Self {
        Self {
            conn,
            system: Arc::new(system),
            places,
            launch,
            write: Mutex::new(()),
            progress: StdMutex::new(Progress {
                flow: None,
                status: None,
                marked: false,
                monitors: 1,
                reader: Reader::default(),
            }),
            enrolment: Enrolment::default(),
        }
    }

    fn progress(&self) -> std::sync::MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Move through the flow; nothing happens before [`Op::Start`].
    fn step(&self, f: impl FnOnce(&mut Flow)) {
        if let Some(flow) = self.progress().flow.as_mut() {
            f(flow);
        }
    }

    async fn start(&self, full: bool) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let status = match proxy.get_welcome().await {
            Ok(s) => Some(parse_json(&s)?),
            Err(e) if client::no_owner(&e) => None,
            Err(e) => return Err(e),
        };
        if self.progress().flow.is_none() {
            let monitors = screens(&proxy).await;
            let reader = fingerprint::reader(&self.system).await;
            let seen = match &status {
                Some(s) if !full && s["done"] == true => s["version"].as_u64().unwrap_or(0) as u32,
                _ => 0,
            };
            let ctx = Context {
                monitors,
                fingerprint: reader.offer(),
                seen_version: seen,
            };
            let mut p = self.progress();
            p.flow = Some(Flow::new(&ctx));
            p.monitors = monitors;
            p.reader = reader;
        }
        self.progress().status = status.clone();
        if status.is_some() && !self.progress().marked {
            proxy.set_welcome_done(CURRENT_WELCOME_VERSION).await?;
            self.progress().marked = true;
            self.progress().status = parse_json(&proxy.get_welcome().await?).ok();
        }
        Ok(())
    }

    fn open_bindle(&self) -> zbus::Result<()> {
        if !self.bindle_installed() {
            return Err(zbus::Error::Failure("bindle isn't installed.".into()));
        }
        // bindle's app picker runs in a terminal; `uqctl launch` opens its menu entry the way
        // the launcher does.
        (self.launch)(&["uqctl", "launch", "bindle"])
            .map_err(|e| zbus::Error::Failure(format!("bindle couldn't be started: {e}")))
    }

    /// bindle's program is there (`/usr/bin/bindle`).
    fn bindle_installed(&self) -> bool {
        self.places.bin_dir.join(apps::BINDLE).is_file()
    }

    fn start_enroll(&self) -> zbus::Result<()> {
        if self.enrolment.busy() {
            return Ok(());
        }
        let enrolment = self.enrolment.clone();
        let system = self.system.clone();
        tokio::spawn(async move { enrolment.run(&system).await });
        Ok(())
    }
}

/// The screens that are on, from uqd's `ListOutputs`; 1 when it can't say.
async fn screens(proxy: &DesktopProxy<'_>) -> usize {
    let Ok(v) = proxy.list_outputs().await.and_then(|t| parse_json(&t)) else {
        return 1;
    };
    let on = v["live"]
        .as_array()
        .map_or(0, |l| l.iter().filter(|o| o["disabled"] != true).count());
    on.max(1)
}

impl PageLogic for WelcomeState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::Start { full } => self.start(full).await,
            Op::Next => {
                self.step(Flow::next);
                Ok(())
            }
            Op::Back => {
                self.step(Flow::back);
                Ok(())
            }
            Op::Skip => {
                if self.progress().flow.as_ref().map(Flow::current) == Some(Step::Fingerprint) {
                    self.enrolment.cancel();
                }
                self.step(Flow::skip);
                Ok(())
            }
            Op::OpenBindle => self.open_bindle(),
            Op::Enroll => self.start_enroll(),
            Op::CancelEnroll => {
                self.enrolment.cancel();
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let entries = scan_all(&self.places.roots);
        let app_rows = apps::list(&entries, &apps::bin_in(&self.places.bin_dir));
        let log = session_log_path(&self.places.state_home);
        let p = self.progress();
        Ok(json!({
            "flow": p.flow.as_ref().map(Flow::to_json),
            "status": p.status,
            "marked": p.marked,
            "monitors": p.monitors,
            "reader": p.reader,
            "enroll": self.enrolment.get(),
            "apps": app_rows,
            "bindle": self.bindle_installed(),
            "setup_problem": setup_problem(&log),
            "session_log": log.display().to_string(),
            "keybinds": keybinds_json(),
        }))
    }
}

#[cfg(test)]
mod tests;
