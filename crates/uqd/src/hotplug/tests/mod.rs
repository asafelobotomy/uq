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

const FIXTURE: &str = include_str!("../../../../uq-core/tests/fixtures/monitors-all.json");
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

const ALL3: &str = r#"{"outputs":{"eDP-1":{},"DP-1":{"scale":2.0},"DP-2":{}}}"#;

mod plug;
mod startup;
