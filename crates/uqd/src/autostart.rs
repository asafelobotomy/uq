//! The autostart runner (spec §7.3, R5–R7, R20, F15): once per login, after the shell is up
//! (or [`SHELL_WAIT`] after uqd starts), each startup app that is on runs in its own scope.
//!
//! The runner sits beside the desktop, outside its lock. Its own mutex is a plain
//! `std::sync::Mutex`, taken only for short bookkeeping: never across a sleep, a spawn, a wait
//! or a journal read (its guard can't be held across an `await` in a spawned task). Writes to
//! the startup section never start or stop anything; they show at the next login.

pub mod hide;
mod list;
mod record;
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
use uq_core::autostart::scan::{Plan, Resolved};
use uq_core::desktop_entry::{random_suffix, unit_id};

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
        }
    }

    /// What hides the windows of apps set to start minimised. Without one they open as usual.
    pub fn with_hider(mut self, hider: Arc<dyn Hider>) -> AutostartConfig {
        self.hider = Some(hider);
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

/// One entry to start.
#[derive(Clone)]
struct Job {
    id: String,
    argv: Vec<String>,
    cwd: PathBuf,
    delay: u32,
    minimize: bool,
    retry: bool,
    /// Part of this login's run, so its start counts as "N s after login". Run now's isn't.
    at_login: bool,
}

/// Run the plan once per login (R7). The caller built `plan` and `env` under the desktop lock
/// and dropped it. `emit` is called on every status change.
pub async fn start_once(
    runner: &SharedRunner,
    plan: Vec<Resolved>,
    env: Vec<(String, String)>,
    emit: impl Fn() + Send + Sync + 'static,
) {
    let (spawner, path, session) = {
        let mut r = lock(runner);
        if r.started {
            return;
        }
        r.started = true;
        if let Some(why) = r.not_run_reason() {
            tracing::info!("startup apps: {why}");
            return;
        }
        let Some(spawner) = r.cfg.spawner.clone() else {
            return;
        };
        let session = r.cfg.session.clone().unwrap_or_default();
        (spawner, r.cfg.record.clone(), session)
    };
    let old = Record::load(&path);
    let same = old.session == session;
    let mut record = Record {
        session,
        ran: same && old.ran,
        pending: BTreeSet::new(),
        started: BTreeMap::new(),
    };
    if same {
        record.started = old.started;
        record.pending = old.pending;
    } else {
        for (id, s) in old.started {
            if spawner.alive(s.pid, &s.unit).await {
                record.started.insert(id, s);
            }
        }
    }
    // A same-login restart only finishes what the first run planned (F15): an entry turned
    // on since login isn't in `pending`, so it waits for the next login.
    let jobs: Vec<Job> = if record.ran {
        tracing::info!("startup apps already ran this login; starting nothing");
        Vec::new()
    } else {
        plan.into_iter()
            .filter(|e| !record.started.contains_key(&e.id))
            .filter(|e| !same || record.pending.contains(&e.id))
            .filter_map(|e| match e.plan {
                // `Desktop::autostart_plan` always gives a working directory.
                Plan::Run {
                    argv,
                    cwd: Some(cwd),
                    delay,
                    minimize,
                    retry,
                } => Some(Job {
                    id: e.id,
                    argv,
                    cwd,
                    delay,
                    minimize,
                    retry,
                    at_login: true,
                }),
                _ => None,
            })
            .collect()
    };
    if !same {
        record.pending = jobs.iter().map(|j| j.id.clone()).collect();
    }
    {
        let mut r = lock(runner);
        for (id, s) in &record.started {
            r.statuses.insert(id.clone(), Status::Adopted(s.clone()));
        }
        for j in &jobs {
            r.statuses.insert(j.id.clone(), Status::Waiting);
        }
        r.t0 = Some(tokio::time::Instant::now());
        record.save(&path);
        r.record = record;
    }
    emit();
    let emit: Arc<dyn Fn() + Send + Sync> = Arc::new(emit);
    let env: Arc<Vec<(String, String)>> = Arc::new(
        env.into_iter()
            .filter(|(k, _)| !is_service_var(k))
            .collect(),
    );
    let recheck = lock(runner).recheck.clone();
    let hider = lock(runner).cfg.hider.clone();
    let ctx = Ctx {
        hider,
        recheck,
        runner: runner.clone(),
        spawner,
        path: path.clone(),
        env,
        emit,
    };
    let handles: Vec<_> = jobs
        .into_iter()
        .map(|job| tokio::spawn(run_one(ctx.clone(), job)))
        .collect();
    for h in handles {
        let _ = h.await;
    }
    let mut r = lock(runner);
    r.record.ran = true;
    r.record.save(&path);
}

#[derive(Clone)]
struct Ctx {
    hider: Option<Arc<dyn Hider>>,
    recheck: Option<Recheck>,
    runner: SharedRunner,
    spawner: Arc<dyn Spawner>,
    path: PathBuf,
    env: Arc<Vec<(String, String)>>,
    emit: Arc<dyn Fn() + Send + Sync>,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// How long after a failure an app is started again, when that is on.
pub const RETRY_AFTER: Duration = Duration::from_secs(10);

/// Sleep the delay, then start the entry.
async fn run_one(ctx: Ctx, job: Job) {
    if job.delay > 0 {
        tokio::time::sleep(Duration::from_secs(job.delay.into())).await;
        if let Some(check) = &ctx.recheck
            && !check(job.id.clone()).await
        {
            let id = job.id;
            tracing::info!("startup app {id} was turned off during its delay; not starting it");
            {
                let mut r = lock(&ctx.runner);
                r.statuses.remove(&id);
                r.record.pending.remove(&id);
                r.record.save(&ctx.path);
            }
            (ctx.emit)();
            return;
        }
    }
    launch(ctx, job, 0).await;
}

/// Start the entry in its own scope, record it, and wait for it in a task. `attempt` is 1 for
/// the second start after a failure.
fn launch(ctx: Ctx, job: Job, attempt: u32) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(launch_inner(ctx, job, attempt))
}

async fn launch_inner(ctx: Ctx, job: Job, attempt: u32) {
    let id = job.id.clone();
    let sid = unit_id(id.strip_suffix(".desktop").unwrap_or(&id));
    let unit = format!("app-uq-autostart-{sid}-{:08x}", random_suffix());
    let tag = format!("uq-autostart-{sid}");
    let at = now_secs();
    let spawned = ctx
        .spawner
        .spawn(&unit, &tag, &job.argv, &ctx.env, &job.cwd)
        .await;
    let child = {
        let mut r = lock(&ctx.runner);
        r.spawns += 1;
        match spawned {
            Err(e) => {
                tracing::warn!("[START-002] starting the startup app {id}: {}", e.kind());
                r.statuses.insert(id.clone(), Status::SpawnFailed);
                None
            }
            Ok(child) => {
                let run = Started {
                    unit,
                    pid: child.pid,
                    at,
                };
                r.statuses.insert(id.clone(), Status::Running);
                r.stopped.remove(&id);
                r.hidden.remove(&id);
                match r.t0 {
                    Some(t0) if job.at_login => {
                        let secs = t0.elapsed().as_secs().min(u64::from(u32::MAX)) as u32;
                        r.offsets.insert(id.clone(), secs);
                    }
                    // Started by hand: no "after login" time, and no "again after it failed"
                    // left over from the login's run.
                    _ => {
                        r.offsets.remove(&id);
                        r.retried.remove(&id);
                    }
                }
                if attempt > 0 {
                    r.retried.insert(id.clone());
                }
                r.record.started.insert(id.clone(), run.clone());
                r.record.save(&ctx.path);
                Some((child, run))
            }
        }
    };
    (ctx.emit)();
    let Some((child, run)) = child else {
        return;
    };
    if job.minimize
        && let Some(hider) = ctx.hider.clone()
    {
        let (runner, emit, id, unit) = (
            ctx.runner.clone(),
            ctx.emit.clone(),
            id.clone(),
            run.unit.clone(),
        );
        tokio::spawn(async move {
            if let Some(how) = hider.hide(&unit).await {
                lock(&runner).hidden.insert(id, how);
                emit();
            }
        });
    }
    tokio::spawn(async move {
        let exit = child.wait.await;
        let on_purpose = lock(&ctx.runner).stopped.contains(&id);
        let status = match exit {
            _ if on_purpose => Status::Exited,
            ExitInfo::Code(0) => Status::Exited,
            ExitInfo::Unknown => Status::Gone,
            exit => Status::Failed { run, exit },
        };
        let failed = matches!(status, Status::Failed { .. });
        lock(&ctx.runner).statuses.insert(id.clone(), status);
        (ctx.emit)();
        if failed && job.retry && attempt == 0 {
            tokio::time::sleep(RETRY_AFTER).await;
            launch(ctx, job, 1).await;
        }
    });
}

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
    let ctx = Ctx {
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
            r.statuses.insert(id.to_string(), Status::Exited);
        }
    }
    emit();
    stopped
}

#[cfg(test)]
mod tests;
