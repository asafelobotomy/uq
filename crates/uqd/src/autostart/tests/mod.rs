use super::spawn::{ExitInfo, FakeSpawner, scope_command};
use super::*;
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::Ordering;
use uq_core::autostart::scan::{Plan, Resolved, Source};

const NM: &str = "nm-applet.desktop";
const SESSION: &str = "abc_123_456";

struct Rig {
    dir: tempfile::TempDir,
    fake: Arc<FakeSpawner>,
    runner: SharedRunner,
}

fn record_path(dir: &Path) -> PathBuf {
    dir.join("run/uq/autostart-run.json")
}

fn rig_with(session: Option<&str>, dev: bool) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(FakeSpawner::default());
    let runner = runner_for(&fake, dir.path(), session, dev);
    Rig { dir, fake, runner }
}

fn runner_for(
    fake: &Arc<FakeSpawner>,
    dir: &Path,
    session: Option<&str>,
    dev: bool,
) -> SharedRunner {
    let cfg = AutostartConfig::new(
        fake.clone(),
        record_path(dir),
        session.map(str::to_string),
        dev,
    );
    Runner::shared(cfg)
}

fn rig() -> Rig {
    rig_with(Some(SESSION), false)
}

fn entry(id: &str, argv: &[&str], delay: u32) -> Resolved {
    Resolved {
        id: id.into(),
        name: id.trim_end_matches(".desktop").into(),
        icon: None,
        source: Source::System,
        enabled: true,
        can_toggle: true,
        delay,
        minimize: false,
        plan: Plan::Run {
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: Some(PathBuf::from("/")),
            delay,
            minimize: false,
            retry: false,
        },
        orphan_copy: None,
    }
}

async fn start(r: &SharedRunner, plan: Vec<Resolved>) {
    start_once(r, plan, Vec::new(), || {}).await;
}

async fn rows(r: &SharedRunner, plan: &[Resolved]) -> Vec<Value> {
    list(r, plan, &BTreeMap::new()).await
}

async fn row(r: &SharedRunner, plan: &[Resolved], id: &str) -> Value {
    rows(r, plan)
        .await
        .into_iter()
        .find(|v| v["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

/// Wait (real time) until `f` holds.
async fn until(mut f: impl AsyncFnMut() -> bool) {
    for _ in 0..500 {
        if f().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition never held");
}

/// Through a real `Desktop` and the folder scan.
mod desktop;

/// A hider that records the scopes it was asked about and answers with `how`.
struct FakeHider {
    asked: Mutex<Vec<String>>,
    how: Option<Hidden>,
}

#[async_trait::async_trait]
impl Hider for FakeHider {
    async fn hide(&self, unit: &str) -> Option<Hidden> {
        self.asked.lock().unwrap().push(unit.to_string());
        self.how
    }
}

fn minimised(id: &str, minimize: bool) -> Resolved {
    let mut e = entry(id, &["app"], 0);
    e.minimize = minimize;
    if let Plan::Run { minimize: m, .. } = &mut e.plan {
        *m = minimize;
    }
    e
}

async fn with_hider(plan: Vec<Resolved>, how: Option<Hidden>) -> (Rig, Arc<FakeHider>, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(FakeSpawner::default());
    let hider = Arc::new(FakeHider {
        asked: Mutex::new(Vec::new()),
        how,
    });
    let cfg = AutostartConfig::new(
        fake.clone(),
        record_path(dir.path()),
        Some(SESSION.into()),
        false,
    )
    .with_hider(hider.clone());
    let runner = Runner::shared(cfg);
    start(&runner, plan.clone()).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let out = rows(&runner, &plan).await;
    (Rig { dir, fake, runner }, hider, out)
}

fn with_retry(id: &str) -> Resolved {
    let mut e = entry(id, &["app"], 0);
    if let Plan::Run { retry, .. } = &mut e.plan {
        *retry = true;
    }
    e
}

async fn settle() {
    for _ in 0..30 {
        tokio::task::yield_now().await;
    }
}

mod control;
mod runner;
