use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::notify::RecordingNotifier;
use crate::system::FakeSystem;
use crate::xrandr::RecordingXrandr;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use uq_core::paths::Paths;

const FIXTURE: &str = include_str!("../../../uq-core/tests/fixtures/monitors-all.json");
const SCALE: &str = r#"{"outputs":{"DP-1":{"scale":1.5}}}"#;
const DELL: &str = r#"{"make":"Dell Inc.","model":"DELL U2723QE","serial":"ABC1234"}"#;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    notes: Arc<RecordingNotifier>,
    xrandr: Arc<RecordingXrandr>,
    events: broadcast::Receiver<Event>,
}

/// The fixture's two screens (`eDP-1`, `DP-1`) plus, when asked, a third (`DP-2`).
fn screens(names: &[&str]) -> String {
    let all: Vec<Value> = serde_json::from_str(FIXTURE).unwrap();
    let mut out = Vec::new();
    for n in names {
        let o = match *n {
            "DP-2" => {
                let mut o = all[1].clone();
                o["name"] = "DP-2".into();
                o["description"] = "Dell Inc. DELL U2723QE XYZ9999".into();
                o["serial"] = "XYZ9999".into();
                o["x"] = 5440.into();
                o
            }
            "eDP-1" => all[0].clone(),
            _ => all[1].clone(),
        };
        out.push(o);
    }
    serde_json::to_string(&out).unwrap()
}

fn show(r: &Rig, names: &[&str]) {
    *r.comp.monitors_json.lock().unwrap() = screens(names);
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    *comp.monitors_json.lock().unwrap() = FIXTURE.into();
    let kv = dir.path().join("kvantum");
    std::fs::create_dir_all(kv.join("KvGnome")).unwrap();
    std::fs::write(kv.join("KvGnome/KvGnome.kvconfig"), "").unwrap();
    let notes = Arc::new(RecordingNotifier::default());
    let xrandr = Arc::new(RecordingXrandr::default());
    let mut desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_kvantum_dir(kv)
        .with_notifier(notes.clone())
        .with_xrandr(xrandr.clone());
    desktop.startup().await;
    comp.calls.lock().unwrap().clear();
    let events = desktop.subscribe();
    Rig {
        _dir: dir,
        desktop,
        comp,
        notes,
        xrandr,
        events,
    }
}

/// Let the notification tasks `outputs_changed` spawned run.
async fn settle() {
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn stored(r: &Rig) -> Value {
    serde_json::from_str(&r.desktop.get_section("monitors").unwrap()).unwrap()
}

async fn keep(r: &mut Rig, json: &str) {
    let (token, _) = r
        .desktop
        .apply_monitors(json, Instant::now())
        .await
        .unwrap();
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
}

fn scale_of(r: &Rig) -> Value {
    stored(r)["outputs"]["DP-1"]["scale"].clone()
}

fn count(r: &Rig, what: &str) -> usize {
    r.comp
        .calls()
        .iter()
        .filter(|c| c.starts_with(what))
        .count()
}

async fn set(r: &mut Rig, section: Value) {
    r.desktop
        .set_section("monitors", &section.to_string(), Origin::Settings)
        .await
        .unwrap();
}

/// "Desk" saved for the three screens with DP-1 at 1.5, then the layout changed to 2.0 on two.
async fn desk_saved(r: &mut Rig) {
    show(r, &["eDP-1", "DP-1", "DP-2"]);
    keep(r, SCALE).await;
    r.desktop
        .remember_monitors("Desk", Origin::Settings)
        .await
        .unwrap();
    show(r, &["eDP-1", "DP-1"]);
    keep(r, r#"{"outputs":{"DP-1":{"scale":2.0}}}"#).await;
    r.comp.calls.lock().unwrap().clear();
    drain(&mut r.events);
}

#[tokio::test]
async fn desk_arrangement_applies_on_plug() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    assert_eq!(scale_of(&r), 2.0);
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), 1.5, "{}", stored(&r));
    assert!(stored(&r)["outputs"]["DP-2"].is_object());
    assert_eq!(count(&r, "verify"), 1);
    assert_eq!(count(&r, "reload"), 1);
    assert!(r.desktop.pending.is_none(), "no revert window");
    assert!(drain(&mut r.events).contains(&Event::MonitorsChanged));
    let generated = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(
        generated
            .contains(r#"output = "DP-1", mode = "preferred", position = "auto", scale = "1.5""#),
        "{generated}"
    );
    assert!(r.notes.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn laptop_arrangement_on_unplug() {
    let mut r = rig().await;
    show(&r, &["eDP-1"]);
    keep(&mut r, r#"{"outputs":{"eDP-1":{"scale":1.25}}}"#).await;
    r.desktop
        .remember_monitors("Laptop", Origin::Settings)
        .await
        .unwrap();
    show(&r, &["eDP-1", "DP-1"]);
    keep(&mut r, r#"{"outputs":{"eDP-1":{"scale":2.0}}}"#).await;
    r.desktop.outputs_changed(true).await;
    show(&r, &["eDP-1"]);
    r.desktop.outputs_changed(false).await;
    assert_eq!(stored(&r)["outputs"]["eDP-1"]["scale"], 1.25);
    assert_eq!(count(&r, "reload"), 1);
}

#[tokio::test]
async fn no_match_changes_nothing() {
    let mut r = rig().await;
    let before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    r.desktop.outputs_changed(true).await;
    assert_eq!(r.comp.calls(), ["monitors"]);
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        before
    );
    assert!(r.notes.sent.lock().unwrap().is_empty());
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert!(v["arrangement"].is_null());
}

#[tokio::test]
async fn auto_off_changes_nothing_but_replans() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    let mut section = stored(&r);
    section["auto_profiles"] = false.into();
    section["workspaces"] =
        json!({"strategy":"sequential","count":10,"manual":[],"move_back":true});
    set(&mut r, section).await;
    r.comp.calls.lock().unwrap().clear();
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), 2.0, "the arrangement stays unused");
    // The plan was rendered again for the new screens, with the same state.
    assert_eq!(count(&r, "verify"), 1);
    assert_eq!(count(&r, "reload"), 1);
    let generated = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(generated.contains(r#"monitor = "DP-2""#), "{generated}");
}

#[tokio::test]
async fn already_applied_is_not_rewritten() {
    let mut r = rig().await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    keep(&mut r, SCALE).await;
    r.desktop
        .remember_monitors("Desk", Origin::Settings)
        .await
        .unwrap();
    r.comp.calls.lock().unwrap().clear();
    let before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    r.desktop.outputs_changed(true).await;
    assert_eq!(r.comp.calls(), ["monitors"]);
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        before
    );
}

#[tokio::test]
async fn hotplug_switch_not_journalled() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    let history = r.desktop.history(100).len();
    let points = r.desktop.restore_points().len();
    let state = std::fs::read(&r.desktop.paths().state_file).unwrap();
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    assert_ne!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        state,
        "the layout was written"
    );
    assert_eq!(r.desktop.history(100).len(), history);
    assert_eq!(r.desktop.restore_points().len(), points);
}

#[tokio::test]
async fn bad_arrangement_skipped_and_notified_once() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    let state = std::fs::read(&r.desktop.paths().state_file).unwrap();
    *r.comp.verify_error.lock().unwrap() = Some("monitor line is wrong".into());
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    for _ in 0..3 {
        r.desktop.outputs_changed(true).await;
    }
    settle().await;
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        state,
        "nothing changed"
    );
    {
        let sent = r.notes.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0].0, "Display arrangement not used");
        assert!(sent[0].1.starts_with("“Desk” couldn't be used"), "{sent:?}");
        assert!(sent[0].1.contains("monitor line is wrong"), "{sent:?}");
    }
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["arrangement"]["failed"], true, "{v}");
    // The set changes and comes back: told again.
    show(&r, &["eDP-1", "DP-1"]);
    r.desktop.outputs_changed(false).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    settle().await;
    assert_eq!(r.notes.sent.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn arrangement_turning_all_off_is_skipped() {
    let mut r = rig().await;
    show(&r, &["eDP-1", "DP-1"]);
    r.desktop
        .remember_monitors("Dark", Origin::Settings)
        .await
        .unwrap();
    let mut section = stored(&r);
    for o in section["profiles"][0]["outputs"].as_array_mut().unwrap() {
        o["config"] = json!({"enabled": false});
    }
    set(&mut r, section).await;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(true).await;
    settle().await;
    assert_eq!(count(&r, "verify"), 0);
    assert!(stored(&r)["outputs"].as_object().unwrap().is_empty());
    let sent = r.notes.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("at least one display must stay on"));
}

#[tokio::test]
async fn hotplug_waits_for_pending_change() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    let (token, _) = r
        .desktop
        .apply_monitors(r#"{"outputs":{"DP-1":{"scale":1.25}}}"#, Instant::now())
        .await
        .unwrap();
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(true).await;
    r.desktop.outputs_changed(true).await;
    assert!(r.comp.calls().is_empty(), "held: {:?}", r.comp.calls());
    assert_eq!(scale_of(&r), 2.0);
    r.desktop.revert_monitors(&token).await.unwrap();
    // The reload that put the old layout back, then the matcher, once.
    assert_eq!(scale_of(&r), 1.5, "{}", stored(&r));
    assert_eq!(count(&r, "verify"), 1);
    r.desktop.outputs_changed(true).await;
    assert_eq!(count(&r, "verify"), 1, "same screens: nothing more");
}

fn two_screens_workspaces(on: &[(u8, &str)]) -> String {
    serde_json::to_string(
        &on.iter()
            .map(|(id, m)| json!({"id": id, "name": id.to_string(), "monitor": m}))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

async fn planned(r: &mut Rig, move_back: bool) {
    set(
        r,
        json!({"outputs":{},"workspaces":{"strategy":"sequential","count":10,"manual":[],"move_back":move_back}}),
    )
    .await;
    r.desktop.hotplug.last_set = None;
    r.comp.calls.lock().unwrap().clear();
}

#[tokio::test]
async fn planned_workspaces_move_back() {
    let mut r = rig().await;
    // eDP-1 is left of DP-1: 1 to 5 belong on eDP-1, 6 to 10 on DP-1.
    *r.comp.workspaces_json.lock().unwrap() =
        two_screens_workspaces(&[(3, "DP-1"), (7, "DP-1"), (9, "DP-1")]);
    planned(&mut r, true).await;
    r.desktop.outputs_changed(true).await;
    let moves: Vec<String> = r
        .comp
        .calls()
        .into_iter()
        .filter(|c| c.contains("workspace.move"))
        .collect();
    assert_eq!(
        moves,
        [r#"eval hl.dispatch(hl.dsp.workspace.move({ workspace = "3", monitor = "eDP-1" }))"#]
    );

    // A removal moves nothing.
    r.desktop.hotplug.last_set = None;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(false).await;
    assert!(!r.comp.calls().iter().any(|c| c.contains("workspace.move")));

    // Already there: nothing.
    *r.comp.workspaces_json.lock().unwrap() = two_screens_workspaces(&[(3, "eDP-1"), (7, "DP-1")]);
    r.desktop.hotplug.last_set = None;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(true).await;
    assert!(!r.comp.calls().iter().any(|c| c.contains("workspace.move")));

    // Move back off: nothing.
    *r.comp.workspaces_json.lock().unwrap() = two_screens_workspaces(&[(3, "DP-1")]);
    planned(&mut r, false).await;
    r.desktop.outputs_changed(true).await;
    assert!(!r.comp.calls().iter().any(|c| c.contains("workspace.move")));
}

#[tokio::test]
async fn xrandr_argv_only_and_skips() {
    let mut r = rig().await;
    // No main display chosen: never called.
    r.desktop.outputs_changed(false).await;
    assert!(r.xrandr.calls.lock().unwrap().is_empty());
    // Choosing one tells X11 once.
    set(
        &mut r,
        json!({"outputs": {}, "primary": serde_json::from_str::<Value>(DELL).unwrap()}),
    )
    .await;
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1"]);
    // The same connector again, by a screen change or a rewrite: not again.
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(false).await;
    let mut section = stored(&r);
    section["bar"] = json!({"show":"main","on":[]});
    set(&mut r, section).await;
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1"]);
    // A different main display is a new call.
    let mut section = stored(&r);
    section["primary"] = json!({"make":"BOE","model":"0x0BCA","serial":""});
    set(&mut r, section).await;
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1", "eDP-1"]);
}

#[tokio::test]
async fn xrandr_is_silent_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    *comp.monitors_json.lock().unwrap() = FIXTURE.into();
    let d = Desktop::new(paths, comp, Arc::new(FakeSystem::default())).unwrap();
    // The default binary and notifier are the silent ones (a dev session, `serve()`).
    assert!(d.xrandr.set_primary("DP-1").await.is_ok());
    d.notifier.notify("x", "y").await;
}

#[tokio::test]
async fn startup_runs_the_matcher_once() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    // What `main` does after serving: the first match.
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 1.5);
    assert_eq!(count(&r, "verify"), 1);
    // And nothing again for the same screens.
    r.desktop.outputs_changed(false).await;
    assert_eq!(count(&r, "verify"), 1);
}

const ALL3: &str = r#"{"outputs":{"eDP-1":{},"DP-1":{"scale":2.0},"DP-2":{}}}"#;

#[tokio::test]
async fn kept_layout_survives_same_set_event() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    // The matcher has handled this set (it applied "Desk"); the user then keeps another layout.
    r.desktop.outputs_changed(true).await;
    keep(&mut r, r#"{"outputs":{"DP-1":{"scale":2.0}}}"#).await;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(true).await;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 2.0);
    assert_eq!(count(&r, "verify"), 0);
    // An undo of that Keep is also left alone by the burst it causes.
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    let kept = scale_of(&r);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), kept);
}

#[tokio::test]
async fn startup_keeps_covering_layout() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    keep(&mut r, ALL3).await;
    r.desktop.hotplug.last_set = None;
    r.comp.calls.lock().unwrap().clear();
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 2.0);
    assert_eq!(count(&r, "verify"), 0);
}

#[tokio::test]
async fn startup_with_a_new_set_switches() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.hotplug.last_set = None;
    r.desktop.outputs_changed(false).await;
    assert_eq!(scale_of(&r), 1.5);
}

#[tokio::test]
async fn connector_swap_is_noticed_without_switching() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    r.desktop.outputs_changed(false).await;
    drain(&mut r.events);
    // The same two screens, each now on the other's connector.
    let mut all: Vec<Value> = serde_json::from_str(&screens(&["eDP-1", "DP-1"])).unwrap();
    all[0]["name"] = "DP-1".into();
    all[1]["name"] = "eDP-1".into();
    *r.comp.monitors_json.lock().unwrap() = serde_json::to_string(&all).unwrap();
    r.desktop.outputs_changed(false).await;
    assert!(
        drain(&mut r.events)
            .iter()
            .any(|e| matches!(e, Event::MonitorsChanged))
    );
}

#[tokio::test]
async fn keep_saves_the_layout_only() {
    let mut r = rig().await;
    let (token, _) = r
        .desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    // Something else in the section changes during the window (a restore, say).
    r.desktop.state.settings.monitors.auto_profiles = false;
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(scale_of(&r), 1.5);
    assert!(!r.desktop.state.settings.monitors.auto_profiles);
}

#[tokio::test]
async fn undo_of_a_main_display_change_tells_x11() {
    let mut r = rig().await;
    set(
        &mut r,
        json!({"outputs": {}, "primary": serde_json::from_str::<Value>(DELL).unwrap()}),
    )
    .await;
    let mut section = stored(&r);
    section["primary"] = json!({"make":"BOE","model":"0x0BCA","serial":""});
    set(&mut r, section).await;
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1", "eDP-1"]);
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert_eq!(*r.xrandr.calls.lock().unwrap(), ["DP-1", "eDP-1", "DP-1"]);
}

#[tokio::test]
async fn a_later_successful_switch_clears_the_failure() {
    let mut r = rig().await;
    desk_saved(&mut r).await;
    *r.comp.verify_error.lock().unwrap() = Some("monitor line is wrong".into());
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["arrangement"]["failed"], true);
    *r.comp.verify_error.lock().unwrap() = None;
    show(&r, &["eDP-1", "DP-1"]);
    r.desktop.outputs_changed(false).await;
    show(&r, &["eDP-1", "DP-1", "DP-2"]);
    r.desktop.outputs_changed(true).await;
    assert_eq!(scale_of(&r), 1.5);
    let v: Value = serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_ne!(v["arrangement"]["failed"], true, "{v}");
}
