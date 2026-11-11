use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::dbus::SharedDesktop;
use crate::service::Desktop;
use crate::system::FakeSystem;
use uq_core::history::Origin;
use uq_core::paths::Paths;

struct DRig {
    dir: tempfile::TempDir,
    desktop: SharedDesktop,
}

fn desktop_rig() -> DRig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let desktop = Desktop::new(
        paths,
        Arc::new(FakeCompositor::running()),
        Arc::new(FakeSystem::default()),
    )
    .unwrap();
    DRig {
        dir,
        desktop: Arc::new(tokio::sync::Mutex::new(desktop)),
    }
}

fn put_user(d: &DRig, id: &str, text: &str) {
    let dir = d.dir.path().join("config/autostart");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(id), text).unwrap();
}

fn app(name: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={name}\n{extra}")
}

async fn plan_of(d: &DRig) -> (Vec<Resolved>, Vec<(String, String)>) {
    d.desktop.lock().await.autostart_plan()
}

async fn set_autostart(d: &DRig, json: &str) {
    d.desktop
        .lock()
        .await
        .set_section("autostart", json, Origin::Settings)
        .await
        .unwrap();
}

#[tokio::test]
async fn runner_skips_off_and_skipped_entries() {
    let d = desktop_rig();
    put_user(&d, "gnome.desktop", &app("gnome", "OnlyShowIn=GNOME;\n"));
    put_user(
        &d,
        "gone.desktop",
        &app("gone", "TryExec=/nonexistent/gone\n"),
    );
    put_user(&d, "hidden.desktop", &app("hidden", "Hidden=true\n"));
    put_user(&d, "term.desktop", &app("term", "Terminal=true\n"));
    put_user(
        &d,
        "dep.desktop",
        "[Desktop Entry]\nType=Application\nName=dep\nExec=dep %d\n",
    );
    put_user(&d, "off.desktop", &app("off", ""));
    put_user(&d, "good.desktop", &app("good", ""));
    set_autostart(&d, r#"{"files":[{"id":"off.desktop","enabled":false}]}"#).await;
    let r = rig();
    let (plan, env) = plan_of(&d).await;
    start_once(&r.runner, plan.clone(), env, || {}).await;
    let calls = r.fake.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].argv, ["good"]);
    assert_eq!(calls[0].cwd, d.dir.path().join("home"));
    let rows = rows(&r.runner, &plan).await;
    let get = |id: &str| rows.iter().find(|v| v["id"] == id).unwrap().clone();
    assert_eq!(get("off.desktop")["status"], "off");
    for (id, reason) in [
        ("gnome.desktop", "Not for this desktop"),
        ("gone.desktop", "Not installed"),
        (
            "hidden.desktop",
            "Turned off by a file in your autostart folder",
        ),
        (
            "term.desktop",
            "Needs a terminal, which uQ doesn't open at login",
        ),
        ("dep.desktop", "Its command can't be read"),
    ] {
        let v = get(id);
        assert_eq!(v["status"], "skipped", "{id}");
        assert_eq!(v["reason"], reason, "{id}");
        assert_eq!(v["can_toggle"], false, "{id}");
    }
    assert_eq!(get("good.desktop")["source"], "yours");
}

#[tokio::test]
async fn runner_env_matches_child_env() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    d.desktop
        .lock()
        .await
        .set_section(
            "env",
            r#"{"vars":[{"name":"FOO","value":"bar"}]}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let (plan, mut env) = plan_of(&d).await;
    let child = d.desktop.lock().await.child_env();
    assert_eq!(env, child);
    assert!(env.contains(&("FOO".into(), "bar".into())));
    env.push(("INVOCATION_ID".into(), "x".into()));
    env.push(("JOURNAL_STREAM".into(), "1:2".into()));
    let r = rig();
    start_once(&r.runner, plan, env, || {}).await;
    let calls = r.fake.calls();
    assert_eq!(calls[0].env, child);
    assert!(
        calls[0]
            .env
            .iter()
            .all(|(k, _)| k != "INVOCATION_ID" && k != "JOURNAL_STREAM")
    );
}

#[tokio::test]
async fn shell_ready_twice_runs_once() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    let r = rig();
    shell_ready(&d.desktop, &r.runner);
    shell_ready(&d.desktop, &r.runner);
    until(async || r.runner.lock().unwrap().record.ran).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(r.fake.calls().len(), 1);
    assert_eq!(r.runner.lock().unwrap().shell_ready_calls(), 2);
}

#[tokio::test(start_paused = true)]
async fn fallback_starts_after_shell_wait() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    let r = rig();
    let _timer = spawn_fallback(d.desktop.clone(), r.runner.clone());
    tokio::time::sleep(SHELL_WAIT - Duration::from_secs(1)).await;
    assert!(r.fake.calls().is_empty());
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(r.fake.calls().len(), 1);
    // A late ShellReady starts nothing more.
    shell_ready(&d.desktop, &r.runner);
    tokio::time::sleep(Duration::from_secs(30)).await;
    assert_eq!(r.fake.calls().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn disabled_runner_never_spawns() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    let runner = Runner::shared(AutostartConfig::disabled());
    assert!(!runner.lock().unwrap().enabled());
    shell_ready(&d.desktop, &runner);
    shell_ready(&d.desktop, &runner);
    tokio::time::sleep(Duration::from_secs(25)).await;
    {
        let guard = runner.lock().unwrap();
        assert_eq!(guard.shell_ready_calls(), 2);
        // It only counted: it never began a run, so it never scanned or wrote a record.
        assert!(!guard.started);
        assert!(guard.statuses.is_empty());
        assert_eq!(guard.spawns(), 0);
    }
    assert!(!d.dir.path().join("run/uq/autostart-run.json").exists());
    let (plan, _) = plan_of(&d).await;
    assert_eq!(row(&runner, &plan, NM).await["status"], "not_started");
}

#[tokio::test(start_paused = true)]
async fn waiting_entry_turned_off_does_not_start() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    put_user(&d, "keep.desktop", &app("keep", ""));
    set_autostart(
        &d,
        r#"{"files":[{"id":"nm-applet.desktop","delay":10},{"id":"keep.desktop","delay":10}]}"#,
    )
    .await;
    let r = rig();
    shell_ready(&d.desktop, &r.runner);
    tokio::time::sleep(Duration::from_secs(5)).await;
    let (plan, _) = plan_of(&d).await;
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "waiting");
    set_autostart(
        &d,
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":10},
                     {"id":"keep.desktop","delay":10}]}"#,
    )
    .await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    let calls = r.fake.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].argv, ["keep"]);
    let (plan, _) = plan_of(&d).await;
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "off");
    let rec = record::Record::load(&record_path(r.dir.path()));
    assert!(rec.ran && !rec.pending.contains(NM), "{rec:?}");
}

#[tokio::test]
async fn set_section_does_not_start_or_stop() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    put_user(&d, "keep.desktop", &app("keep", ""));
    set_autostart(
        &d,
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false}]}"#,
    )
    .await;
    let r = rig();
    let (plan, env) = plan_of(&d).await;
    start_once(&r.runner, plan.clone(), env, || {}).await;
    assert_eq!(r.fake.calls().len(), 1);
    assert_eq!(row(&r.runner, &plan, NM).await["status"], "off");

    set_autostart(
        &d,
        r#"{"files":[{"id":"nm-applet.desktop","enabled":true},
                     {"id":"keep.desktop","enabled":false}]}"#,
    )
    .await;
    let (plan, _) = plan_of(&d).await;
    let v = row(&r.runner, &plan, NM).await;
    assert_eq!(v["status"], "not_started");
    assert_eq!(v["reason"], "Starts next time you log in");
    // Turning a running entry off doesn't close it.
    let keep = row(&r.runner, &plan, "keep.desktop").await;
    assert_eq!(
        (keep["status"].clone(), keep["enabled"].clone()),
        ("running".into(), false.into())
    );
    assert_eq!(r.fake.calls().len(), 1);
}

#[tokio::test]
async fn added_command_lists_its_argv() {
    let d = desktop_rig();
    put_user(&d, NM, &app("nm-applet", ""));
    set_autostart(
        &d,
        r#"{"added":[{"id":"a-1234","name":"Backup","argv":["backup","--quiet"],"enabled":false}]}"#,
    )
    .await;
    let r = rig();
    let (plan, commands) = {
        let g = d.desktop.lock().await;
        (g.autostart_plan().0, g.autostart_commands())
    };
    let rows = list(&r.runner, &plan, &commands).await;
    let added = rows.iter().find(|v| v["id"] == "a-1234").unwrap();
    assert_eq!(added["argv"], serde_json::json!(["backup", "--quiet"]));
    assert_eq!(added["source"], "added");
    assert_eq!(added["status"], "off");
    let nm = rows.iter().find(|v| v["id"] == NM).unwrap();
    assert!(nm.get("argv").is_none());
    assert!(nm.get("exit").is_none() && nm.get("log").is_none());
}
