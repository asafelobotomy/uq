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
        plan: Plan::Run {
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: Some(PathBuf::from("/")),
            delay,
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
            "systemd-cat",
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
    fake2.alive.store(true, Ordering::SeqCst);
    assert_eq!(row(&again, &plan, NM).await["status"], "running");
    fake2.alive.store(false, Ordering::SeqCst);
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

    let live = Arc::new(FakeSpawner::default());
    live.alive.store(true, Ordering::SeqCst);
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
