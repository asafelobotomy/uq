use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::paths::Paths;
use uq_core::state::State;

const FIXTURE: &str = include_str!("../../../uq-core/tests/fixtures/monitors-all.json");

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn paths(dir: &std::path::Path) -> Paths {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    paths
}

fn desktop_on(dir: &std::path::Path) -> (Desktop, Arc<FakeCompositor>) {
    let comp = Arc::new(FakeCompositor::running());
    *comp.monitors_json.lock().unwrap() = FIXTURE.into();
    let kv = dir.join("kvantum");
    std::fs::create_dir_all(kv.join("KvGnome")).unwrap();
    std::fs::write(kv.join("KvGnome/KvGnome.kvconfig"), "").unwrap();
    let d = Desktop::new(paths(dir), comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_kvantum_dir(kv);
    (d, comp)
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let (mut desktop, comp) = desktop_on(dir.path());
    desktop.startup().await;
    comp.calls.lock().unwrap().clear();
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        events,
    }
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn files(r: &Rig) -> (Vec<u8>, Vec<u8>) {
    let p = r.desktop.paths();
    (
        std::fs::read(&p.state_file).unwrap(),
        std::fs::read(&p.generated_lua).unwrap(),
    )
}

fn marker(r: &Rig) -> PathBuf {
    r.desktop
        .paths()
        .state_file
        .with_file_name("monitors-pending.json")
}

fn journal_len(r: &Rig) -> usize {
    r.desktop.history(1000).len()
}

const SCALE: &str = r#"{"outputs":{"DP-1":{"scale":1.5}}}"#;

#[test]
fn revert_window_is_fifteen_seconds() {
    assert_eq!(REVERT_AFTER, Duration::from_secs(15));
}

#[tokio::test]
async fn confirm_saves_once() {
    let mut r = rig().await;
    let before = files(&r);
    let entries = journal_len(&r);
    let now = Instant::now();
    let (token, deadline) = r.desktop.apply_monitors(SCALE, now).await.unwrap();
    assert_eq!(token.len(), 32);
    assert_eq!(deadline, now + REVERT_AFTER);
    let evals: Vec<_> = r
        .comp
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("eval "))
        .collect();
    assert_eq!(evals.len(), 1, "{evals:?}");
    assert!(
        evals[0].contains(r#"hl.monitor({ output = "DP-1""#)
            && evals[0].contains(r#"scale = "1.5""#),
        "{evals:?}"
    );
    assert_eq!(files(&r), before, "apply must not save");
    assert_eq!(journal_len(&r), entries);
    assert!(marker(&r).is_file());

    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    let saved = State::load(&r.desktop.paths().state_file);
    let dp = &saved.settings.monitors.outputs["DP-1"];
    assert_eq!(dp.scale, Some(1.5));
    assert!(dp.description.as_deref().is_some_and(|d| !d.is_empty()));
    let generated = String::from_utf8(files(&r).1).unwrap();
    assert!(generated.contains(r#"scale = "1.5""#), "{generated}");
    assert_eq!(journal_len(&r), entries + 1);
    let e = &r.desktop.history(1)[0];
    assert_eq!(
        (e.domain.as_str(), e.origin.as_str()),
        ("monitors", "settings")
    );
    assert!(!marker(&r).exists());
    assert!(r.desktop.pending.is_none());
    assert_eq!(
        r.desktop.confirm_monitors(&token, Origin::Settings).await,
        Err(DesktopError::InvalidSetting(NOT_WAITING.into()))
    );
    let evs = drain(&mut r.events);
    assert_eq!(
        evs.iter().filter(|e| **e == Event::MonitorsChanged).count(),
        2
    );
}

#[tokio::test]
async fn timeout_reverts() {
    let mut r = rig().await;
    let before = files(&r);
    let entries = journal_len(&r);
    let now = Instant::now();
    let (token, _) = r.desktop.apply_monitors(SCALE, now).await.unwrap();
    drain(&mut r.events);
    assert!(
        !r.desktop
            .expire_monitors(&token, now + Duration::from_secs(14))
            .await
    );
    assert!(r.desktop.pending.is_some());
    assert!(r.desktop.expire_monitors(&token, now + REVERT_AFTER).await);
    let calls = r.comp.calls();
    let eval = calls.iter().position(|c| c.starts_with("eval ")).unwrap();
    assert_eq!(calls[eval + 1..], ["reload".to_string()], "{calls:?}");
    assert_eq!(files(&r), before);
    assert_eq!(journal_len(&r), entries);
    assert!(drain(&mut r.events).contains(&Event::MonitorsChanged));
    assert!(!marker(&r).exists());
    assert!(
        r.desktop
            .confirm_monitors(&token, Origin::Settings)
            .await
            .is_err()
    );
    assert!(!r.desktop.expire_monitors(&token, now + REVERT_AFTER).await);
}

#[tokio::test]
async fn crash_during_window_recovers_on_restart() {
    let r = rig().await;
    let Rig {
        dir, mut desktop, ..
    } = r;
    let entries = desktop.history(1000).len();
    let old = desktop.state.settings.monitors.clone();
    desktop.apply_monitors(SCALE, Instant::now()).await.unwrap();
    let marker = desktop
        .paths()
        .state_file
        .with_file_name("monitors-pending.json");
    assert!(marker.is_file());
    drop(desktop);

    let (mut again, comp) = desktop_on(dir.path());
    let mut rx = again.subscribe();
    again.startup().await;
    assert!(!marker.exists());
    assert_eq!(again.state.settings.monitors, old);
    assert!(comp.calls().contains(&"reload".to_string()));
    assert!(drain(&mut rx).contains(&Event::MonitorsChanged));
    assert_eq!(again.history(1000).len(), entries);
}

#[tokio::test]
async fn reboot_during_window_keeps_old_config() {
    let mut r = rig().await;
    r.desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    let (state, generated) = files(&r);
    assert!(!String::from_utf8(state).unwrap().contains("1.5"));
    assert!(!String::from_utf8(generated).unwrap().contains("hl.monitor"));
    let fresh = State::load(&r.desktop.paths().state_file);
    assert!(fresh.settings.monitors.outputs.is_empty());
}

#[tokio::test]
async fn second_apply_replaces_and_keeps_rollback() {
    let mut r = rig().await;
    // Store DP-1 at scale 1 first.
    let (t0, _) = r
        .desktop
        .apply_monitors(r#"{"outputs":{"DP-1":{"scale":1.0}}}"#, Instant::now())
        .await
        .unwrap();
    r.desktop
        .confirm_monitors(&t0, Origin::Settings)
        .await
        .unwrap();
    let entries = journal_len(&r);
    let now = Instant::now();
    let (a, _) = r
        .desktop
        .apply_monitors(r#"{"outputs":{"DP-1":{"scale":2.0}}}"#, now)
        .await
        .unwrap();
    let later = now + Duration::from_secs(5);
    let (b, deadline) = r
        .desktop
        .apply_monitors(r#"{"outputs":{"HDMI-A-1":{"enabled":false}}}"#, later)
        .await
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(deadline, later + REVERT_AFTER);
    assert!(
        r.desktop
            .confirm_monitors(&a, Origin::Settings)
            .await
            .is_err()
    );
    let p = r.desktop.pending.clone().unwrap();
    assert_eq!(p.rollback.outputs["DP-1"].scale, Some(1.0));
    assert_eq!(
        p.touched.iter().map(String::as_str).collect::<Vec<_>>(),
        ["DP-1", "HDMI-A-1"]
    );
    *r.comp.reload_error.lock().unwrap() = Some("no".into());
    r.comp.calls.lock().unwrap().clear();
    assert!(r.desktop.expire_monitors(&b, deadline).await);
    let calls = r.comp.calls();
    let fallback = calls.last().unwrap();
    assert!(
        fallback.contains(r#"output = "DP-1""#) && fallback.contains(r#"scale = "1.0""#),
        "{calls:?}"
    );
    assert!(
        fallback.contains(
            r#"output = "HDMI-A-1", mode = "preferred", position = "auto", scale = "auto""#
        ),
        "{calls:?}"
    );
    assert_eq!(journal_len(&r), entries);
}

#[tokio::test]
async fn apply_refuses_all_outputs_off() {
    let mut r = rig().await;
    let json = r#"{"outputs":{"eDP-1":{"enabled":false},"DP-1":{"mirror_of":"eDP-1"}}}"#;
    let res = r.desktop.apply_monitors(json, Instant::now()).await;
    assert_eq!(
        res,
        Err(DesktopError::InvalidSetting(
            "at least one display must stay on".into()
        ))
    );
    assert!(!r.comp.calls().iter().any(|c| c.starts_with("eval")));
    assert!(!marker(&r).exists());
    assert!(r.desktop.pending.is_none());
}

#[tokio::test]
async fn rejected_eval_reverts_at_once() {
    let mut r = rig().await;
    let entries = journal_len(&r);
    *r.comp.eval_error.lock().unwrap() = Some("bad monitor".into());
    let res = r.desktop.apply_monitors(SCALE, Instant::now()).await;
    assert_eq!(res, Err(DesktopError::ConfigInvalid("bad monitor".into())));
    assert!(r.desktop.pending.is_none());
    assert!(r.comp.calls().contains(&"reload".to_string()));
    assert!(!marker(&r).exists());
    assert_eq!(journal_len(&r), entries);
}

#[tokio::test]
async fn rejected_verify_applies_nothing() {
    let mut r = rig().await;
    *r.comp.verify_error.lock().unwrap() = Some("nope".into());
    let res = r.desktop.apply_monitors(SCALE, Instant::now()).await;
    assert_eq!(res, Err(DesktopError::ConfigInvalid("nope".into())));
    assert!(!r.comp.calls().iter().any(|c| c.starts_with("eval")));
    assert!(!marker(&r).exists());
    assert!(r.desktop.pending.is_none());
}

#[tokio::test]
async fn reload_during_window_reapplies_candidate() {
    let mut r = rig().await;
    let (token, _) = r
        .desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    r.comp.calls.lock().unwrap().clear();
    r.desktop
        .set_section("look", r#"{"gaps_in":7}"#, Origin::Settings)
        .await
        .unwrap();
    let calls = r.comp.calls();
    let n = calls.len();
    assert_eq!(calls[n - 2], "reload", "{calls:?}");
    assert!(calls[n - 1].starts_with("eval ") && calls[n - 1].contains(r#"scale = "1.5""#));
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    let saved = State::load(&r.desktop.paths().state_file);
    assert_eq!(saved.settings.look.gaps_in, Some(7));
    assert_eq!(saved.settings.monitors.outputs["DP-1"].scale, Some(1.5));
}

#[tokio::test]
async fn apply_refused_without_hyprland() {
    let mut r = rig().await;
    *r.comp.running.lock().unwrap() = false;
    assert_eq!(
        r.desktop.apply_monitors(SCALE, Instant::now()).await,
        Err(DesktopError::InvalidSetting(NO_DESKTOP.into()))
    );
    assert!(!marker(&r).exists());
}

#[tokio::test]
async fn revert_now_and_list_outputs() {
    let mut r = rig().await;
    let now = Instant::now();
    let (token, _) = r.desktop.apply_monitors(SCALE, now).await.unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_outputs(now + Duration::from_secs(3)).await).unwrap();
    assert_eq!(v["pending"]["token"], token.as_str());
    assert_eq!(v["pending"]["seconds_left"], 12);
    assert_eq!(v["live"][0]["name"], "eDP-1");
    assert!(v["stored"]["outputs"].as_object().unwrap().is_empty());
    assert!(r.desktop.revert_monitors("wrong").await.is_err());
    r.desktop.revert_monitors(&token).await.unwrap();
    assert!(r.desktop.pending.is_none());
    let v: serde_json::Value = serde_json::from_str(&r.desktop.list_outputs(now).await).unwrap();
    assert!(v["pending"].is_null());
}

#[tokio::test]
async fn undo_of_a_display_change_reloads_without_a_window() {
    let mut r = rig().await;
    let (token, _) = r
        .desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    let id = r.desktop.history(1)[0].id.clone();
    r.comp.calls.lock().unwrap().clear();
    r.desktop.undo(&id, false).await.unwrap();
    assert!(
        r.desktop
            .state
            .settings
            .monitors
            .outputs
            .get("DP-1")
            .is_none_or(|o| o.scale.is_none())
    );
    assert!(r.comp.calls().contains(&"reload".to_string()));
    assert_eq!(r.desktop.history(1)[0].origin, "restore");
}
