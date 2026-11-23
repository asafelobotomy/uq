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

#[tokio::test]
async fn runner_starts_each_entry_once_with_scope_and_tag() {
    let r = rig();
    let plan = vec![entry(NM, &["nm-applet", "--indicator"], 0)];
    start(&r.runner, plan.clone()).await;
    start(&r.runner, plan.clone()).await;
    let calls = r.fake.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let c = &calls[0].command;
    assert_eq!(
        c[..6],
        [
            "/fake/bin/systemd-run",
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "--slice=app.slice"
        ]
    );
    let suffix = c[6]
        .strip_prefix("--unit=app-uq-autostart-nm-applet-")
        .unwrap();
    assert!(
        suffix.len() == 8 && suffix.bytes().all(|b| b.is_ascii_hexdigit()),
        "{c:?}"
    );
    assert_eq!(
        c[7..],
        [
            "--",
            "/fake/bin/systemd-cat",
            "-t",
            "uq-autostart-nm-applet",
            "--",
            "nm-applet",
            "--indicator"
        ]
    );
    assert_eq!(calls[0].tag, "uq-autostart-nm-applet");
    assert_eq!(format!("--unit={}", calls[0].unit), c[6]);
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "running");
}

#[test]
fn scope_command_runs_no_shell_and_drops_service_vars() {
    let argv = vec!["x".to_string(), "; rm -rf ~".to_string()];
    let env = vec![("FOO".to_string(), "$(bar)".to_string())];
    let cmd = scope_command(
        Path::new("/usr/bin/systemd-run"),
        Path::new("/usr/bin/systemd-cat"),
        "app-uq-autostart-x-0000abcd",
        "uq-autostart-x",
        &argv,
        &env,
        Path::new("/home/u"),
    );
    let cmd = cmd.as_std();
    assert_eq!(cmd.get_program(), "/usr/bin/systemd-run");
    let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
    assert_eq!(args.last(), Some(&"; rm -rf ~"));
    assert_eq!(cmd.get_current_dir(), Some(Path::new("/home/u")));
    let envs: Vec<_> = cmd.get_envs().collect();
    assert!(envs.contains(&("FOO".as_ref(), Some("$(bar)".as_ref()))));
    for (k, _) in std::env::vars() {
        if spawn::is_service_var(&k) {
            assert!(envs.contains(&(k.as_ref(), None)), "{k} kept");
        }
    }
    assert!(spawn::is_service_var("INVOCATION_ID") && spawn::is_service_var("LISTEN_FDS"));
    assert!(!spawn::is_service_var("PATH"));
}

#[test]
fn system_spawner_needs_both_tools_by_absolute_path() {
    let all = |n: &str| Some(PathBuf::from(format!("/usr/bin/{n}")));
    let s = spawn::SystemSpawner::with_finder(all).unwrap();
    assert_eq!(
        s.tools(),
        (
            Path::new("/usr/bin/systemd-run"),
            Path::new("/usr/bin/systemd-cat")
        )
    );
    let no_cat = |n: &str| (n != "systemd-cat").then(|| PathBuf::from(format!("/usr/bin/{n}")));
    assert!(spawn::SystemSpawner::with_finder(no_cat).is_none());
    let no_run = |n: &str| (n != "systemd-run").then(|| PathBuf::from(format!("/usr/bin/{n}")));
    assert!(spawn::SystemSpawner::with_finder(no_run).is_none());
}

#[test]
fn cgroup_names_the_scope() {
    let t =
        "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-uq-autostart-x-1.scope\n";
    assert!(spawn::cgroup_has_unit(t, "app-uq-autostart-x-1"));
    assert!(!spawn::cgroup_has_unit(t, "app-uq-autostart-x"));
}

#[tokio::test(start_paused = true)]
async fn runner_waits_for_delay() {
    let r = rig();
    let plan = vec![entry(NM, &["nm-applet"], 5)];
    let runner = r.runner.clone();
    let task = tokio::spawn({
        let plan = plan.clone();
        async move { start(&runner, plan).await }
    });
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "waiting");
    assert!(r.fake.calls().is_empty());
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "running");
    assert_eq!(r.fake.calls().len(), 1);
    task.await.unwrap();
}

#[tokio::test]
async fn runner_records_and_does_not_rerun_after_restart() {
    let r = rig();
    let plan = vec![entry(NM, &["nm-applet"], 0)];
    start(&r.runner, plan.clone()).await;
    let rec = record::Record::load(&record_path(r.dir.path()));
    assert_eq!(rec.session, SESSION);
    assert!(rec.ran);
    assert_eq!(rec.started[NM].unit, r.fake.calls()[0].unit);

    let fake2 = Arc::new(FakeSpawner::default());
    let again = runner_for(&fake2, r.dir.path(), Some(SESSION), false);
    start(&again, plan.clone()).await;
    assert!(fake2.calls().is_empty());
    let first = rec.started[NM].clone();
    // Alive, but under another unit: not this run.
    fake2.set_alive(first.pid, "app-uq-autostart-other-00000000");
    assert_eq!(row(&again, &plan, NM).await["status"], "unknown_exit");
    fake2.set_alive(first.pid, &first.unit);
    assert_eq!(row(&again, &plan, NM).await["status"], "running");
    assert_eq!(
        fake2.alive_calls.lock().unwrap().last(),
        Some(&(first.pid, first.unit.clone()))
    );
    fake2.live.lock().unwrap().clear();
    let v = row(&again, &plan, NM).await;
    assert_eq!(v["status"], "unknown_exit");
    assert_eq!(v["reason"], "Started earlier; finished");
}

#[tokio::test]
async fn runner_restart_starts_nothing_new() {
    let r = rig();
    start(&r.runner, vec![entry(NM, &["nm-applet"], 0)]).await;
    let fake2 = Arc::new(FakeSpawner::default());
    let again = runner_for(&fake2, r.dir.path(), Some(SESSION), false);
    let plan = vec![
        entry(NM, &["nm-applet"], 0),
        entry("new.desktop", &["new"], 0),
    ];
    start(&again, plan.clone()).await;
    assert!(fake2.calls().is_empty());
    let v = row(&again, &plan, "new.desktop").await;
    assert_eq!(v["status"], "not_started");
    assert_eq!(v["reason"], "Starts next time you log in");
}

#[tokio::test]
async fn runner_new_session_skips_live_pid() {
    let r = rig();
    let plan = vec![entry(NM, &["nm-applet"], 0)];
    start(&r.runner, plan.clone()).await;

    let first = record::Record::load(&record_path(r.dir.path())).started[NM].clone();
    let live = Arc::new(FakeSpawner::default());
    live.set_alive(first.pid, &first.unit);
    let next = runner_for(&live, r.dir.path(), Some("other_session"), false);
    start(&next, plan.clone()).await;
    assert!(live.calls().is_empty());
    assert_eq!(row(&next, &plan, NM).await["status"], "running");

    let dead = Arc::new(FakeSpawner::default());
    let third = runner_for(&dead, r.dir.path(), Some("third_session"), false);
    start(&third, plan.clone()).await;
    assert_eq!(dead.calls().len(), 1);
}

#[tokio::test]
async fn runner_failed_entry_has_exit_and_log() {
    let r = rig();
    r.fake.exit("uq-autostart-bad", ExitInfo::Code(3));
    *r.fake.journal.lock().unwrap() = (0..12)
        .map(|i| format!("line {i}"))
        .chain(["evil \u{202E}txt\u{7}".to_string()])
        .collect();
    let plan = vec![entry("bad.desktop", &["bad"], 0)];
    let before = chrono::Utc::now().timestamp();
    start(&r.runner, plan.clone()).await;
    let runner = r.runner.clone();
    let p = plan.clone();
    until(async || row(&runner, &p, "bad.desktop").await["status"] == "failed").await;
    let v = row(&r.runner, &plan, "bad.desktop").await;
    assert_eq!(v["exit"], 3);
    let log: Vec<&str> = v["log"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert_eq!(log.len(), MAX_LOG_LINES);
    assert_eq!(log.last(), Some(&"evil  txt "));
    let tails = r.fake.tails.lock().unwrap().clone();
    let call = &r.fake.calls()[0];
    assert_eq!(
        (tails[0].0.as_str(), tails[0].1.as_str()),
        (call.unit.as_str(), "uq-autostart-bad")
    );
    assert!(tails[0].2 >= before && tails[0].2 <= chrono::Utc::now().timestamp());
}

#[tokio::test]
async fn journal_tail_is_cached_and_bounded() {
    let r = rig();
    *r.fake.journal_delay.lock().unwrap() = Some(Duration::from_millis(50));
    *r.fake.journal.lock().unwrap() = vec!["boom".into()];
    let plan: Vec<Resolved> = (0..5)
        .map(|i| {
            r.fake
                .exit(&format!("uq-autostart-f{i}"), ExitInfo::Code(1));
            entry(&format!("f{i}.desktop"), &["x"], 0)
        })
        .collect();
    start(&r.runner, plan.clone()).await;
    until(async || {
        let g = r.runner.lock().unwrap();
        let failed = g.statuses.values();
        failed
            .filter(|s| matches!(s, Status::Failed { .. }))
            .count()
            == 5
    })
    .await;
    let first = rows(&r.runner, &plan).await;
    assert!(first.iter().all(|v| v["status"] == "failed"));
    rows(&r.runner, &plan).await;
    assert_eq!(r.fake.tails.lock().unwrap().len(), 5);
    assert!(r.fake.tails_max.load(Ordering::SeqCst) <= MAX_TAILS);
}

#[tokio::test]
async fn failed_or_empty_journal_reads_are_not_cached() {
    let r = rig();
    r.fake.exit("uq-autostart-bad", ExitInfo::Code(1));
    r.fake.journal_fails.store(true, Ordering::SeqCst);
    let plan = vec![entry("bad.desktop", &["bad"], 0)];
    start(&r.runner, plan.clone()).await;
    let runner = r.runner.clone();
    let p = plan.clone();
    until(async || row(&runner, &p, "bad.desktop").await["status"] == "failed").await;
    let tails = || r.fake.tails.lock().unwrap().len();
    let n = tails();
    // A read that failed shows no lines and is tried again.
    assert_eq!(
        row(&r.runner, &plan, "bad.desktop").await["log"],
        serde_json::json!([])
    );
    assert_eq!(tails(), n + 1);
    // So is one that found nothing yet.
    r.fake.journal_fails.store(false, Ordering::SeqCst);
    assert_eq!(
        row(&r.runner, &plan, "bad.desktop").await["log"],
        serde_json::json!([])
    );
    assert_eq!(tails(), n + 2);
    // Lines, once read, are kept.
    *r.fake.journal.lock().unwrap() = vec!["boom".into()];
    let v = row(&r.runner, &plan, "bad.desktop").await;
    assert_eq!(v["log"], serde_json::json!(["boom"]));
    row(&r.runner, &plan, "bad.desktop").await;
    assert_eq!(tails(), n + 3);
}

#[tokio::test(start_paused = true)]
async fn restart_before_ran_starts_only_what_was_planned() {
    let r = rig();
    let plan = vec![entry(NM, &["nm-applet"], 300)];
    let runner = r.runner.clone();
    let first = tokio::spawn({
        let plan = plan.clone();
        async move { start(&runner, plan).await }
    });
    tokio::time::sleep(Duration::from_secs(10)).await;
    let rec = record::Record::load(&record_path(r.dir.path()));
    assert!(!rec.ran);
    assert_eq!(rec.pending.iter().collect::<Vec<_>>(), [NM]);
    first.abort();
    // uqd restarts mid-delay; "new" was turned on since login.
    let fake2 = Arc::new(FakeSpawner::default());
    let again = runner_for(&fake2, r.dir.path(), Some(SESSION), false);
    let plan = vec![
        entry(NM, &["nm-applet"], 0),
        entry("new.desktop", &["new"], 0),
    ];
    start(&again, plan.clone()).await;
    let calls = fake2.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].argv, ["nm-applet"]);
    assert_eq!(
        row(&again, &plan, "new.desktop").await["status"],
        "not_started"
    );
    assert!(record::Record::load(&record_path(r.dir.path())).ran);
}

#[tokio::test]
async fn runner_does_nothing_in_dev_session() {
    let r = rig_with(Some(SESSION), true);
    let plan = vec![entry(NM, &["nm-applet"], 0)];
    start(&r.runner, plan.clone()).await;
    assert!(r.fake.calls().is_empty());
    let v = row(&r.runner, &plan, NM).await;
    assert_eq!(v["status"], "not_started");
    assert_eq!(v["reason"], "Not started in a test session");
    assert!(!record_path(r.dir.path()).exists());
}

#[tokio::test]
async fn runner_bad_session_id_runs_nothing() {
    for session in [None, Some(""), Some("a/../b"), Some("x y")] {
        let r = rig_with(session, false);
        let plan = vec![entry(NM, &["nm-applet"], 0)];
        start(&r.runner, plan.clone()).await;
        assert!(r.fake.calls().is_empty(), "{session:?}");
        assert_eq!(row(&r.runner, &plan, NM).await["status"], "not_started");
        assert!(!record_path(r.dir.path()).exists());
    }
    assert!(record::valid_session(&"a".repeat(128)));
    assert!(!record::valid_session(&"a".repeat(129)));
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

#[tokio::test]
async fn only_apps_set_to_start_minimised_are_hidden() {
    let plan = vec![minimised("a.desktop", true), minimised("b.desktop", false)];
    let (_rig, hider, out) = with_hider(plan, Some(Hidden::Tray)).await;
    let asked = hider.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].starts_with("app-uq-autostart-a-"), "{asked:?}");
    assert_eq!(out[0]["minimize"], true);
    assert_eq!(out[0]["hidden"], "tray");
    assert_eq!(out[1]["minimize"], false);
    assert!(out[1].get("hidden").is_none());
}

#[tokio::test]
async fn minimised_without_a_tray_icon_is_reported() {
    let (_rig, _h, out) =
        with_hider(vec![minimised("a.desktop", true)], Some(Hidden::Minimised)).await;
    assert_eq!(out[0]["hidden"], "minimised");
}

#[tokio::test]
async fn an_unreachable_compositor_reports_nothing() {
    let (_rig, _h, out) = with_hider(vec![minimised("a.desktop", true)], None).await;
    assert!(out[0].get("hidden").is_none());
    assert_eq!(out[0]["status"], "running");
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

#[tokio::test(start_paused = true)]
async fn a_failed_app_is_started_once_more_when_retry_is_on() {
    let r = rig();
    r.fake.exit_once("uq-autostart-a", ExitInfo::Code(1));
    start(&r.runner, vec![with_retry("a.desktop")]).await;
    settle().await;
    assert_eq!(r.fake.calls().len(), 1);
    tokio::time::sleep(RETRY_AFTER + Duration::from_secs(1)).await;
    settle().await;
    assert_eq!(r.fake.calls().len(), 2, "started again after it failed");
    let out = rows(&r.runner, &[with_retry("a.desktop")]).await;
    assert_eq!(out[0]["status"], "running");
    assert_eq!(out[0]["retried"], true);
}

#[tokio::test(start_paused = true)]
async fn retry_is_once_and_only_when_asked() {
    let r = rig();
    r.fake.exit("uq-autostart-a", ExitInfo::Code(1));
    r.fake.exit("uq-autostart-b", ExitInfo::Code(1));
    start(
        &r.runner,
        vec![with_retry("a.desktop"), entry("b.desktop", &["app"], 0)],
    )
    .await;
    tokio::time::sleep(RETRY_AFTER * 3).await;
    settle().await;
    let units: Vec<String> = r.fake.calls().into_iter().map(|c| c.tag).collect();
    assert_eq!(
        units.iter().filter(|t| *t == "uq-autostart-a").count(),
        2,
        "{units:?}"
    );
    assert_eq!(units.iter().filter(|t| *t == "uq-autostart-b").count(), 1);
    let out = rows(&r.runner, &[with_retry("a.desktop")]).await;
    assert_eq!(out[0]["status"], "failed");
}

#[tokio::test]
async fn stop_now_ends_a_running_app_without_calling_it_a_failure() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 0)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    assert!(!stop_now(&r.runner, "nope.desktop", || {}).await);
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    assert_eq!(r.fake.stops.lock().unwrap().len(), 1);
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["status"], "exited");
    // Nothing runs now, so there is nothing to stop.
    assert!(!stop_now(&r.runner, "a.desktop", || {}).await);
}

#[tokio::test]
async fn run_now_starts_an_app_that_is_not_running_once() {
    let r = rig();
    let e = entry("a.desktop", &["app"], 600);
    // Not started by the login plan (it is still waiting for its delay) counts as running.
    assert!(run_now(&r.runner, &e, Vec::new(), || {}).await);
    settle().await;
    assert_eq!(r.fake.calls().len(), 1);
    assert!(
        !run_now(&r.runner, &e, Vec::new(), || {}).await,
        "already running"
    );
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    assert!(run_now(&r.runner, &e, Vec::new(), || {}).await);
    assert_eq!(r.fake.calls().len(), 2);
    let off = Resolved {
        plan: Plan::Off,
        ..e
    };
    assert!(!run_now(&r.runner, &off, Vec::new(), || {}).await);
}

#[tokio::test(start_paused = true)]
async fn rows_say_how_long_after_login_an_app_started() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 7)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["start_in"], 7);
    assert_eq!(out[0]["started_after"], 7);
}

#[tokio::test(start_paused = true)]
async fn run_now_after_login_has_no_after_login_time() {
    let r = rig();
    let plan = vec![entry("a.desktop", &["app"], 0)];
    start(&r.runner, plan.clone()).await;
    settle().await;
    assert!(stop_now(&r.runner, "a.desktop", || {}).await);
    settle().await;
    tokio::time::sleep(std::time::Duration::from_secs(90)).await;
    assert!(run_now(&r.runner, &plan[0], Vec::new(), || {}).await);
    settle().await;
    let out = rows(&r.runner, &plan).await;
    assert_eq!(out[0]["status"], "running");
    assert!(
        out[0].get("started_after").is_none_or(Value::is_null),
        "Run now isn't a start after login: {}",
        out[0]
    );
}
