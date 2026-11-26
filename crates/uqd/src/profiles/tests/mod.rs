use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::power::PowerError;
use crate::power::fake::FakePower;
use crate::system::FakeSystem;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uq_core::history::{AutoReason, Origin, PointKind};
use uq_core::paths::Paths;
use uq_core::profile::{Profile, power_notice, read_leaf, save_active};
use uq_core::schedule::Schedule;
use uq_core::theme::Variant;

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";
const WORK: &str =
    "format = 1\nid = \"work\"\nname = \"Work\"\n[look]\ngaps_in = 8\nrounding = 2\n";
const TURBO: &str = "format = 1\nid = \"turbo\"\nname = \"Turbo\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";
const NIGHT: &str =
    "format = 1\nid = \"night\"\nname = \"Night\"\n[appearance]\nvariant = \"dark\"\n";

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    power: Arc<FakePower>,
    events: broadcast::Receiver<Event>,
}

fn paths(dir: &std::path::Path) -> Paths {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    paths
}

/// A desktop on `dir`'s paths, as uqd builds it at start.
fn desktop_on(dir: &std::path::Path, comp: Arc<FakeCompositor>, power: Arc<FakePower>) -> Desktop {
    Desktop::new(paths(dir), comp, Arc::new(FakeSystem::default()))
        .unwrap()
        .with_power(power)
}

async fn rig_with(power: FakePower) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let comp = Arc::new(FakeCompositor::running());
    let power = Arc::new(power);
    let mut desktop = desktop_on(dir.path(), comp.clone(), power.clone());
    desktop.startup().await;
    // Start every test from gaps 5 and blur on, as a person's own settings.
    desktop
        .set_section("look", r#"{"gaps_in":5,"blur":true}"#, Origin::Settings)
        .await
        .unwrap();
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        power,
        events,
    }
}

async fn rig() -> Rig {
    rig_with(FakePower::new("balanced")).await
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn profile_events(rx: &mut broadcast::Receiver<Event>) -> Vec<String> {
    drain(rx)
        .into_iter()
        .filter_map(|e| match e {
            Event::ProfileChanged(id) => Some(id),
            _ => None,
        })
        .collect()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn look(r: &Rig, key: &str) -> Option<Value> {
    read_leaf(r.desktop.state(), "look", &s(&[key]))
}

/// Change one `look` key by hand, keeping the rest of the section.
async fn set_look(r: &mut Rig, key: &str, v: Value) {
    let mut section: Value = serde_json::from_str(&r.desktop.get_section("look").unwrap()).unwrap();
    section[key] = v;
    r.desktop
        .set_section("look", &section.to_string(), Origin::Settings)
        .await
        .unwrap();
}

async fn add(r: &mut Rig, toml: &str) {
    r.desktop.set_profile(toml, false).unwrap();
}

fn journal_len(r: &Rig) -> usize {
    r.desktop.history(1000).len()
}

fn read(path: &std::path::Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Reads at once, never finishes a set.
struct SlowSet(Arc<FakePower>);

#[async_trait::async_trait]
impl crate::power::PowerProfiles for SlowSet {
    async fn active(&self) -> Result<String, PowerError> {
        self.0.active().await
    }
    async fn set_active(&self, _mode: &str) -> Result<(), PowerError> {
        std::future::pending().await
    }
}

/// A window rule "Pavucontrol floats", on; returns its id.
async fn add_rule(r: &mut Rig) -> String {
    r.desktop
        .set_section(
            "rules",
            r#"{"window":[{"name":"Pavucontrol floats","enabled":true,
                "match":{"class":"^pavucontrol$"},"effects":{"float":true}}]}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    r.desktop.state().rules.window[0].id.clone()
}

fn rules_profile(body: &str) -> String {
    format!("format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[rules]\n{body}")
}

mod apply_off;
mod power;
mod rules_shortcuts;
mod store;
