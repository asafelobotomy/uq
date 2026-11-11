//! `ListAutostart`: the page's rows, from the plan and what the runner knows. Liveness is
//! re-checked for entries an earlier uqd started (F15), and a failed run's last journal lines
//! are read outside every lock and cached once they hold lines, at most [`MAX_TAILS`](super::MAX_TAILS) at a time (F16).

use super::spawn::ExitInfo;
use super::{MAX_LOG_LINES, SharedRunner, Status, lock};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use uq_core::autostart::scan::{Plan, Resolved, Source};
use uq_core::desktop_entry::clean_display;

const NEXT_LOGIN: &str = "Starts next time you log in";
const FINISHED: &str = "Started earlier; finished";
const SPAWN_FAILED: &str = "uQ couldn't start it";
const MAX_LOG_CHARS: usize = 200;

fn source_name(s: Source) -> &'static str {
    match s {
        Source::System => "system",
        Source::Yours => "yours",
        Source::Added => "added",
    }
}

fn tag_of(unit: &str) -> String {
    // `app-uq-autostart-<sid>-<8 hex>` → `uq-autostart-<sid>`.
    let base = unit.strip_prefix("app-").unwrap_or(unit);
    base.rsplit_once('-').map_or(base, |(b, _)| b).to_string()
}

/// The rows, in plan order: `{id, name, icon, source, enabled, can_toggle, delay, status,
/// reason?, exit?, signal?, log?, argv?}`. `commands` holds the argv of each added command.
pub async fn list(
    runner: &SharedRunner,
    plan: &[Resolved],
    commands: &BTreeMap<String, Vec<String>>,
) -> Vec<Value> {
    let (statuses, spawner, why_not, sem, mut logs) = {
        let r = lock(runner);
        let statuses: BTreeMap<String, Status> = plan
            .iter()
            .filter_map(|e| Some((e.id.clone(), r.statuses.get(&e.id)?.clone())))
            .collect();
        let logs: HashMap<(String, ExitInfo), Vec<String>> = statuses
            .values()
            .filter_map(|s| match s {
                Status::Failed { run, exit } => {
                    let key = (run.unit.clone(), *exit);
                    Some((key.clone(), r.logs.get(&key)?.clone()))
                }
                _ => None,
            })
            .collect();
        (
            statuses,
            r.cfg.spawner.clone(),
            r.not_run_reason(),
            r.tails.clone(),
            logs,
        )
    };
    let mut alive = HashMap::new();
    if let Some(sp) = &spawner {
        for (id, s) in &statuses {
            if let Status::Adopted(run) = s {
                alive.insert(id.clone(), sp.alive(run.pid, &run.unit).await);
            }
        }
        let reads: Vec<_> = statuses
            .values()
            .filter_map(|s| match s {
                Status::Failed { run, exit } if !logs.contains_key(&(run.unit.clone(), *exit)) => {
                    let (sp, sem, run, exit) = (sp.clone(), sem.clone(), run.clone(), *exit);
                    Some(tokio::spawn(async move {
                        let _permit = sem.acquire_owned().await.ok()?;
                        let lines = sp
                            .journal_tail(&run.unit, &tag_of(&run.unit), run.at, MAX_LOG_LINES)
                            .await?;
                        let start = lines.len().saturating_sub(MAX_LOG_LINES);
                        let lines: Vec<String> = lines[start..]
                            .iter()
                            .map(|l| clean_display(l, MAX_LOG_CHARS))
                            .collect();
                        Some(((run.unit, exit), lines))
                    }))
                }
                _ => None,
            })
            .collect();
        let mut fresh = Vec::new();
        for h in reads {
            if let Ok(Some(got)) = h.await {
                fresh.push(got);
            }
        }
        // Only lines are cached: a read that failed or found nothing yet (journald may not
        // have the stream indexed when the exit is first listed) is tried again next time.
        if fresh.iter().any(|(_, v)| !v.is_empty()) {
            let mut r = lock(runner);
            for (k, v) in fresh.iter().filter(|(_, v)| !v.is_empty()) {
                r.logs.insert(k.clone(), v.clone());
            }
        }
        logs.extend(fresh);
    }
    plan.iter()
        .map(|e| {
            let st = statuses.get(&e.id);
            let mut o = row_base(e);
            let (status, reason): (&str, Option<&str>) = match (st, &e.plan) {
                (Some(Status::Waiting), _) => ("waiting", None),
                (Some(Status::Running), _) => ("running", None),
                (Some(Status::Adopted(_)), _) if alive.get(&e.id) == Some(&true) => {
                    ("running", None)
                }
                (Some(Status::Adopted(_) | Status::Gone), _) => ("unknown_exit", Some(FINISHED)),
                (Some(Status::Exited), _) => ("exited", None),
                (Some(Status::Failed { run, exit }), _) => {
                    match exit {
                        ExitInfo::Code(c) => o.insert("exit".into(), json!(c)),
                        ExitInfo::Signal(s) => o.insert("signal".into(), json!(s)),
                        ExitInfo::Unknown => None,
                    };
                    let log = logs.get(&(run.unit.clone(), *exit)).cloned();
                    o.insert("log".into(), json!(log.unwrap_or_default()));
                    ("failed", None)
                }
                (Some(Status::SpawnFailed), _) => ("failed", Some(SPAWN_FAILED)),
                (None, Plan::Off) => ("off", None),
                (None, Plan::Skip(r)) => ("skipped", Some(*r)),
                (None, Plan::Run { .. }) => ("not_started", Some(why_not.unwrap_or(NEXT_LOGIN))),
            };
            o.insert("status".into(), json!(status));
            if let Some(r) = reason {
                o.insert("reason".into(), json!(r));
            }
            if e.source == Source::Added
                && let Some(argv) = commands.get(&e.id)
            {
                o.insert("argv".into(), json!(argv));
            }
            Value::Object(o)
        })
        .collect()
}

fn row_base(e: &Resolved) -> Map<String, Value> {
    let v = json!({
        "id": e.id,
        "name": e.name,
        "icon": e.icon,
        "source": source_name(e.source),
        "enabled": e.enabled,
        "can_toggle": e.can_toggle,
        "delay": e.delay,
    });
    match v {
        Value::Object(o) => o,
        _ => unreachable!("a json object"),
    }
}
