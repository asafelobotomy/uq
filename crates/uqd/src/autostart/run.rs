//! Starting the planned entries once per session: delays, scopes, retries and their records.

use super::*;

/// One entry to start.
#[derive(Clone)]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) argv: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) delay: u32,
    pub(crate) minimize: bool,
    pub(crate) retry: bool,
    /// Part of this login's run, so its start counts as "N s after login". Run now's isn't.
    pub(crate) at_login: bool,
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
pub(crate) struct Ctx {
    pub(crate) hider: Option<Arc<dyn Hider>>,
    pub(crate) recheck: Option<Recheck>,
    pub(crate) runner: SharedRunner,
    pub(crate) spawner: Arc<dyn Spawner>,
    pub(crate) path: PathBuf,
    pub(crate) env: Arc<Vec<(String, String)>>,
    pub(crate) emit: Arc<dyn Fn() + Send + Sync>,
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Sleep the delay, then start the entry.
pub(crate) async fn run_one(ctx: Ctx, job: Job) {
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
pub(crate) fn launch(ctx: Ctx, job: Job, attempt: u32) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(launch_inner(ctx, job, attempt))
}

pub(crate) async fn launch_inner(ctx: Ctx, job: Job, attempt: u32) {
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
