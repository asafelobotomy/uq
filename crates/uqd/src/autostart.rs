//! The autostart runner (spec §7.3, R5–R7, R20, F15): once per login, after the shell is up
//! (or [`SHELL_WAIT`] after uqd starts), each startup app that is on runs in its own scope.
//!
//! The runner sits beside the desktop, outside its lock. Its own mutex is a plain
//! `std::sync::Mutex`, taken only for short bookkeeping: never across a sleep, a spawn, a wait
//! or a journal read (its guard can't be held across an `await` in a spawned task). Writes to
//! the startup section never start or stop anything; they show at the next login.

pub mod conditions;
pub mod hide;
mod list;
mod record;
mod restore;
pub mod spawn;
mod trigger;

pub use list::list;
pub use trigger::{shell_ready, spawn_fallback};

use hide::{Hidden, Hider};
use record::{Record, Started, valid_session};
use spawn::{ExitInfo, Spawner, SystemSpawner, is_service_var};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::Semaphore;
use uq_core::autostart::When;
use uq_core::autostart::scan::{Plan, Resolved};
use uq_core::desktop_entry::{random_suffix, unit_id};
mod run;
pub use run::*;

/// How long uqd waits for `ShellReady` before it starts the apps anyway (R5).
pub const SHELL_WAIT: Duration = Duration::from_secs(20);
/// The journal lines kept for a failed entry (F16).
pub const MAX_LOG_LINES: usize = 10;
/// The most journal reads at once (F16).
pub const MAX_TAILS: usize = 4;

const DEV_SESSION: &str = "Not started in a test session";
const NO_SESSION: &str = "Not started: uQ couldn't tell which login this is";

pub type SharedRunner = Arc<Mutex<Runner>>;

/// Asks, after an entry's delay, whether it is still on now (L1): `true` to start it.
pub type Recheck = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// What the runner may do. Only `main.rs` builds the system one (F1); `serve()` gets
/// [`AutostartConfig::disabled`], which never starts a process.
pub struct AutostartConfig {
    spawner: Option<Arc<dyn Spawner>>,
    record: PathBuf,
    session: Option<String>,
    dev: bool,
    hider: Option<Arc<dyn Hider>>,
    /// Where the network and power are read from (`/`, or a test folder).
    probe_root: PathBuf,
}

impl AutostartConfig {
    /// The real runner: `systemd-run` from uqd's `PATH`, the record under
    /// `$XDG_RUNTIME_DIR/uq/`. Without either it never starts anything, and says why in the log.
    pub fn system(dev: bool, session: Option<String>) -> AutostartConfig {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        let Some(runtime) = runtime else {
            tracing::warn!("[START-001] XDG_RUNTIME_DIR isn't set; startup apps won't be started");
            return AutostartConfig {
                dev,
                session,
                ..AutostartConfig::disabled()
            };
        };
        let spawner = SystemSpawner::new().map(|s| Arc::new(s) as Arc<dyn Spawner>);
        if spawner.is_none() {
            tracing::warn!(
                "[START-001] systemd-run or systemd-cat isn't on uqd's PATH; startup apps won't be started"
            );
        }
        AutostartConfig {
            spawner,
            record: runtime.join("uq/autostart-run.json"),
            session,
            dev,
            hider: None,
            probe_root: PathBuf::from("/"),
        }
    }

    /// No spawner and no fallback timer: `ShellReady` only counts.
    pub fn disabled() -> AutostartConfig {
        AutostartConfig {
            spawner: None,
            record: PathBuf::new(),
            session: None,
            dev: false,
            hider: None,
            probe_root: PathBuf::from("/"),
        }
    }

    /// What hides the windows of apps set to start minimised. Without one they open as usual.
    pub fn with_hider(mut self, hider: Arc<dyn Hider>) -> AutostartConfig {
        self.hider = Some(hider);
        self
    }

    /// Read the network and power under `root` instead of `/` (tests).
    pub fn with_probe_root(mut self, root: PathBuf) -> AutostartConfig {
        self.probe_root = root;
        self
    }

    pub fn has_spawner(&self) -> bool {
        self.spawner.is_some()
    }

    /// A runner over any spawner (tests use `FakeSpawner`).
    pub fn new(
        spawner: Arc<dyn Spawner>,
        record: PathBuf,
        session: Option<String>,
        dev: bool,
    ) -> AutostartConfig {
        AutostartConfig {
            spawner: Some(spawner),
            record,
            session,
            dev,
            hider: None,
            probe_root: PathBuf::from("/"),
        }
    }
}

/// What happened to one entry this uqd knows about.
#[derive(Debug, Clone)]
pub(crate) enum Status {
    Waiting,
    /// Started by this uqd, which waits for it.
    Running,
    /// Started earlier this login (or a live pid from an earlier one); `alive` tells.
    Adopted(Started),
    Exited,
    Failed {
        run: Started,
        exit: ExitInfo,
    },
    /// It ended, but how isn't known.
    Gone,
    SpawnFailed,
    /// Stopped with Stop now.
    Stopped,
    /// Not started this login: its condition wasn't met.
    Held(&'static str),
}

pub struct Runner {
    cfg: AutostartConfig,
    started: bool,
    statuses: BTreeMap<String, Status>,
    /// How each app that started minimised was hidden (this uqd's runs only).
    hidden: BTreeMap<String, Hidden>,
    /// When this login's run began, and how many seconds after it each app started.
    t0: Option<tokio::time::Instant>,
    offsets: BTreeMap<String, u32>,
    /// Apps that were started a second time after they failed.
    retried: BTreeSet<String>,
    /// Apps stopped on purpose, so their end isn't a failure and isn't retried.
    stopped: BTreeSet<String>,
    shell_ready_calls: u32,
    /// What each entry waits for, from the state at login.
    when: BTreeMap<String, When>,
    /// When a waiting entry starts (Unix seconds), so the page can count down.
    due: BTreeMap<String, i64>,
    /// Entries waiting for their condition now, with what the page says.
    holding: BTreeMap<String, &'static str>,
    /// Last session's apps to reopen at this login, each with its workspace.
    restore: Vec<(Resolved, i64)>,
    /// The cleaned last lines of a failed run, by `(unit, exit)` (F16).
    logs: HashMap<(String, ExitInfo), Vec<String>>,
    record: Record,
    spawns: u32,
    tails: Arc<Semaphore>,
    recheck: Option<Recheck>,
}

fn lock(r: &Mutex<Runner>) -> MutexGuard<'_, Runner> {
    r.lock().unwrap_or_else(|e| e.into_inner())
}

impl Runner {
    pub fn new(cfg: AutostartConfig) -> Runner {
        Runner {
            cfg,
            started: false,
            statuses: BTreeMap::new(),
            hidden: BTreeMap::new(),
            t0: None,
            offsets: BTreeMap::new(),
            retried: BTreeSet::new(),
            stopped: BTreeSet::new(),
            shell_ready_calls: 0,
            when: BTreeMap::new(),
            due: BTreeMap::new(),
            holding: BTreeMap::new(),
            restore: Vec::new(),
            logs: HashMap::new(),
            record: Record::default(),
            spawns: 0,
            tails: Arc::new(Semaphore::new(MAX_TAILS)),
            recheck: None,
        }
    }

    /// How a delayed entry is checked again before it starts. Without one, it always starts.
    pub fn set_recheck(&mut self, recheck: Recheck) {
        self.recheck = Some(recheck);
    }

    /// What each entry waits for at this login's run.
    pub fn set_when(&mut self, when: BTreeMap<String, When>) {
        self.when = when;
    }

    /// Last session's apps, started with this login's run and moved to their workspaces.
    pub fn set_restore(&mut self, restore: Vec<(Resolved, i64)>) {
        self.restore = restore;
    }

    pub fn shared(cfg: AutostartConfig) -> SharedRunner {
        Arc::new(Mutex::new(Runner::new(cfg)))
    }

    /// How many times the shell said it is up (F39).
    pub fn shell_ready_calls(&self) -> u32 {
        self.shell_ready_calls
    }

    /// Whether it has a spawner at all (`serve()`'s hasn't).
    pub fn enabled(&self) -> bool {
        self.cfg.has_spawner()
    }

    /// How many processes it tried to start.
    pub fn spawns(&self) -> u32 {
        self.spawns
    }

    /// The plain reason an entry that is on wasn't started, when it isn't "next login".
    fn not_run_reason(&self) -> Option<&'static str> {
        if self.cfg.dev {
            Some(DEV_SESSION)
        } else if self.enabled() && !self.session_ok() {
            Some(NO_SESSION)
        } else {
            None
        }
    }

    fn session_ok(&self) -> bool {
        self.cfg.session.as_deref().is_some_and(valid_session)
    }
}

/// How long after a failure an app is started again, when that is on.
pub const RETRY_AFTER: Duration = Duration::from_secs(10);

/// Start an entry now, as the page's Run now does: outside the once-per-login plan, with no
/// delay. `false` when the runner can't start anything or the app is running already.
pub async fn run_now(
    runner: &SharedRunner,
    job_plan: &Resolved,
    env: Vec<(String, String)>,
    emit: impl Fn() + Send + Sync + 'static,
) -> bool {
    let Plan::Run {
        argv,
        cwd: Some(cwd),
        minimize,
        retry,
        ..
    } = job_plan.plan.clone()
    else {
        return false;
    };
    let (spawner, path, hider) = {
        let r = lock(runner);
        if r.not_run_reason().is_some() {
            return false;
        }
        let Some(spawner) = r.cfg.spawner.clone() else {
            return false;
        };
        (spawner, r.cfg.record.clone(), r.cfg.hider.clone())
    };
    if is_running(runner, &job_plan.id, &spawner).await {
        return false;
    }
    let probe_root = lock(runner).cfg.probe_root.clone();
    let ctx = Ctx {
        probe_root,
        hider,
        recheck: None,
        runner: runner.clone(),
        spawner,
        path,
        env: Arc::new(
            env.into_iter()
                .filter(|(k, _)| !is_service_var(k))
                .collect(),
        ),
        emit: Arc::new(emit),
    };
    let job = Job {
        id: job_plan.id.clone(),
        argv,
        cwd,
        delay: 0,
        minimize,
        retry,
        when: None,
        workspace: None,
        at_login: false,
    };
    launch(ctx, job, 0).await;
    true
}

async fn is_running(runner: &SharedRunner, id: &str, spawner: &Arc<dyn Spawner>) -> bool {
    let status = lock(runner).statuses.get(id).cloned();
    match status {
        Some(Status::Running | Status::Waiting) => true,
        Some(Status::Adopted(run)) => spawner.alive(run.pid, &run.unit).await,
        _ => false,
    }
}

/// Stop an app that is running, as the page's Stop now does. `false` when it isn't running.
pub async fn stop_now(runner: &SharedRunner, id: &str, emit: impl Fn()) -> bool {
    let (spawner, unit) = {
        let r = lock(runner);
        let Some(spawner) = r.cfg.spawner.clone() else {
            return false;
        };
        let unit = match r.statuses.get(id) {
            Some(Status::Running | Status::Adopted(_)) => {
                r.record.started.get(id).map(|s| s.unit.clone())
            }
            _ => None,
        };
        (spawner, unit)
    };
    let Some(unit) = unit else {
        return false;
    };
    lock(runner).stopped.insert(id.to_string());
    let stopped = spawner.stop(&unit).await;
    if !stopped {
        lock(runner).stopped.remove(id);
    } else {
        // An app an earlier uqd started has no waiter to say it ended.
        let mut r = lock(runner);
        if matches!(r.statuses.get(id), Some(Status::Adopted(_))) {
            r.statuses.insert(id.to_string(), Status::Stopped);
        }
    }
    emit();
    stopped
}

#[cfg(test)]
mod tests;
