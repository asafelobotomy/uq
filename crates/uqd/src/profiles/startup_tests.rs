//! Profiles holding environment variables, app fixes and startup apps (D1, Task 13).

use super::*;
use crate::autostart::spawn::FakeSpawner;
use crate::autostart::{AutostartConfig, Runner, list, start_once};
use crate::compositor::fake::FakeCompositor;
use crate::files::{self, MarkerKind};
use crate::system::FakeSystem;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uq_core::history::Origin;
use uq_core::paths::Paths;

const CODE: &str = "code.desktop";
const SOURCE: &str = "[Desktop Entry]\nType=Application\nName=Code\nExec=/usr/bin/code %F\n";
const APPLET: &str = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";
const APPS_NOTICE: &str =
    "Apps you start from now on use this profile's settings; apps already open keep theirs.";
const LOGIN_NOTICE: &str = "Startup apps change next time you log in.";

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let mut desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    desktop.startup().await;
    Rig { dir, desktop, comp }
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn profile(id: &str, body: &str) -> String {
    format!("format = 1\nid = \"{id}\"\nname = \"{id}\"\n{body}")
}

async fn add(r: &mut Rig, id: &str, body: &str) {
    r.desktop.set_profile(&profile(id, body), false).unwrap();
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn env_file(r: &Rig) -> Option<String> {
    std::fs::read_to_string(files::environment_d_path(&r.desktop.paths().startup)).ok()
}

fn put_applet(r: &Rig) {
    put(
        &r.dir.path().join("xdg/autostart/nm-applet.desktop"),
        APPLET,
    );
}

fn hidden_copy(r: &Rig) -> PathBuf {
    r.desktop
        .paths()
        .startup
        .user_autostart()
        .join("nm-applet.desktop")
}

#[tokio::test]
async fn profile_sets_env_and_back() {
    let mut r = rig().await;
    add(&mut r, "dev", "[env]\nFOO = \"bar\"\n").await;
    let before = r.desktop.history(1000).len();
    let report = r.desktop.apply_profile("dev").await.unwrap();
    assert_eq!(report.notices, [APPS_NOTICE]);
    let h = r.desktop.history(1000);
    let new = &h[..h.len() - before];
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(
        (new[0].domain.as_str(), new[0].origin.as_str()),
        ("env", "profile:dev")
    );
    assert_eq!(new[0].summary, "Environment variable FOO added");
    assert!(r.comp.calls().iter().any(|c| c.starts_with("verify")));
    assert!(generated(&r).contains(r#"hl.env("FOO", "bar", true)"#));
    assert!(env_file(&r).unwrap().contains("FOO='bar'"));
    // The restore point was saved before the change.
    let point = r.desktop.restore_points().into_iter().next().unwrap();
    assert!(point.name.contains("dev"), "{point:?}");
    assert!(
        r.desktop
            .restore
            .load(&point.id)
            .unwrap()
            .env
            .vars
            .is_empty()
    );

    let report = r.desktop.profile_off().await.unwrap();
    assert!(report.kept.is_empty(), "{report:?}");
    assert!(r.desktop.state().env.vars.is_empty());
    assert!(!generated(&r).contains("FOO"));
    assert!(env_file(&r).is_none_or(|f| !f.contains("FOO")));
    assert_eq!(r.desktop.history(1).first().unwrap().origin, "profile:dev");
}

#[tokio::test]
async fn profile_switch_updates_override() {
    let mut r = rig().await;
    put(
        &r.dir.path().join("share-dirs/applications").join(CODE),
        SOURCE,
    );
    let fix = r#"{"apps":[{"id":"code.desktop","platform":"wayland","elsewhere":true}]}"#;
    r.desktop
        .set_section("compat", fix, Origin::Settings)
        .await
        .unwrap();
    let target = files::override_target(&r.desktop.paths().startup, CODE).unwrap();
    let text = || std::fs::read_to_string(&target).unwrap();
    assert!(text().contains("GDK_BACKEND=wayland,x11"), "{}", text());
    add(
        &mut r,
        "old",
        "[compat.apps.\"code.desktop\"]\nplatform = \"x11\"\n",
    )
    .await;
    r.desktop.apply_profile("old").await.unwrap();
    assert!(files::is_ours(&target, MarkerKind::Override));
    assert!(text().contains("GDK_BACKEND=x11"), "{}", text());
    assert!(r.desktop.state().compat.apps[0].elsewhere);
    r.desktop.profile_off().await.unwrap();
    assert!(text().contains("GDK_BACKEND=wayland,x11"), "{}", text());
}

#[tokio::test]
async fn profile_turns_system_entry_off_and_back() {
    let mut r = rig().await;
    put_applet(&r);
    add(
        &mut r,
        "quiet",
        "[autostart.\"nm-applet.desktop\"]\nenabled = false\n",
    )
    .await;
    let report = r.desktop.apply_profile("quiet").await.unwrap();
    assert_eq!(report.notices, [LOGIN_NOTICE]);
    let t = std::fs::read_to_string(hidden_copy(&r)).unwrap();
    assert!(
        t.contains("Hidden=true") && t.contains("X-uQ-Autostart=true"),
        "{t}"
    );
    let rows = r.desktop.explain("autostart").await.unwrap();
    assert_eq!(rows["nm-applet.desktop"]["source"], "profile");
    assert_eq!(rows["nm-applet.desktop"]["detail"]["name"], "quiet");
    assert_eq!(r.desktop.history(1)[0].origin, "profile:quiet");
    r.desktop.profile_off().await.unwrap();
    assert!(!files::present(&hidden_copy(&r)));
    assert!(r.desktop.state().autostart.files.is_empty());
}

#[tokio::test]
async fn profile_autostart_never_starts_or_stops() {
    let mut r = rig().await;
    put_applet(&r);
    r.desktop
        .set_section(
            "autostart",
            r#"{"files":[{"id":"nm-applet.desktop","enabled":false}]}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let fake = Arc::new(FakeSpawner::default());
    let record = r.dir.path().join("run/uq/autostart-run.json");
    let runner = Runner::shared(AutostartConfig::new(
        fake.clone(),
        record,
        Some("s1".into()),
        false,
    ));
    let (plan, env) = r.desktop.autostart_plan();
    start_once(&runner, plan, env, || {}).await;
    assert!(fake.calls().is_empty());
    add(
        &mut r,
        "on",
        "[autostart.\"nm-applet.desktop\"]\nenabled = true\ndelay = 0\n",
    )
    .await;
    r.desktop.apply_profile("on").await.unwrap();
    let (plan, _) = r.desktop.autostart_plan();
    let rows = list(&runner, &plan, &BTreeMap::new()).await;
    let row = rows
        .iter()
        .find(|v| v["id"] == "nm-applet.desktop")
        .unwrap();
    assert_eq!(row["status"], "not_started", "{row}");
    r.desktop.profile_off().await.unwrap();
    assert!(fake.calls().is_empty(), "{:?}", fake.calls());
}

#[tokio::test]
async fn unknown_startup_id_is_a_notice() {
    let mut r = rig().await;
    put_applet(&r);
    let body = "[autostart.\"nm-applet.desktop\"]\ndelay = 5\n\
                [autostart.\"gone.desktop\"]\nenabled = false\n\
                [autostart.\"a-1234\"]\nenabled = false\n";
    add(&mut r, "mixed", body).await;
    let report = r.desktop.apply_profile("mixed").await.unwrap();
    assert_eq!(
        report.notices,
        [
            "2 startup apps in this profile aren't on this computer, so they were left out.",
            LOGIN_NOTICE,
        ]
    );
    let a = &r.desktop.state().autostart;
    assert_eq!(a.files.len(), 1);
    assert_eq!(a.choice("nm-applet.desktop").unwrap().delay, Some(5));
    // The file keeps them.
    let text = r.desktop.get_profile("mixed").unwrap();
    assert!(
        text.contains("gone.desktop") && text.contains("a-1234"),
        "{text}"
    );
}

#[tokio::test]
async fn startup_notice_in_report() {
    let mut r = rig().await;
    put_applet(&r);
    add(
        &mut r,
        "both",
        "[compat]\nqt_wayland = true\n[autostart.\"nm-applet.desktop\"]\nenabled = false\n",
    )
    .await;
    let report = r.desktop.apply_profile("both").await.unwrap();
    assert_eq!(report.notices, [APPS_NOTICE, LOGIN_NOTICE]);
    add(&mut r, "plain", "[look]\ngaps_in = 3\n").await;
    let report = r.desktop.apply_profile("plain").await.unwrap();
    assert!(report.notices.is_empty(), "{report:?}");
}

#[tokio::test]
async fn rejected_verify_changes_nothing() {
    let mut r = rig().await;
    add(&mut r, "dev", "[env]\nFOO = \"bar\"\n").await;
    let journal = r.desktop.history(1000).len();
    let file = env_file(&r);
    *r.comp.verify_error.lock().unwrap() = Some("bad env".into());
    let e = r.desktop.apply_profile("dev").await.unwrap_err();
    assert!(matches!(e, DesktopError::ConfigInvalid(_)), "{e:?}");
    assert_eq!(r.desktop.history(1000).len(), journal);
    assert_eq!(env_file(&r), file);
    assert!(r.desktop.state().env.vars.is_empty());
    assert!(r.desktop.active_profile().is_none());
}

#[tokio::test]
async fn too_many_variables_are_refused() {
    let mut r = rig().await;
    let vars: Vec<serde_json::Value> = (0..100)
        .map(|i| serde_json::json!({"name": format!("V{i}"), "value": "x"}))
        .collect();
    r.desktop
        .set_section(
            "env",
            &serde_json::json!({ "vars": vars }).to_string(),
            Origin::Settings,
        )
        .await
        .unwrap();
    add(&mut r, "more", "[env]\nEXTRA = \"1\"\n").await;
    let journal = r.desktop.history(1000).len();
    let points = r.desktop.restore_points().len();
    let e = r.desktop.apply_profile("more").await.unwrap_err();
    assert_eq!(
        e.to_string(),
        "This profile would make more than 100 environment variables."
    );
    assert_eq!(r.desktop.history(1000).len(), journal);
    assert_eq!(r.desktop.restore_points().len(), points);
    assert_eq!(r.desktop.state().env.vars.len(), 100);
    assert!(r.desktop.active_profile().is_none());
}
