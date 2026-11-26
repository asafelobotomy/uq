use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::service::Event;
use crate::system::FakeSystem;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::history::Origin;
use uq_core::paths::Paths;
use uq_core::settings::{MonitorId, OutputConfig, Strategy, WorkspacePlan};

const CODE: &str = "code.desktop";
const SOURCE: &str = "[Desktop Entry]\nType=Application\nName=Code\nExec=/usr/bin/code %F\n";
const ELSEWHERE: &str = r#"{"apps":[{"id":"code.desktop","platform":"wayland","elsewhere":true}]}"#;

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn rig_dev(dev: bool) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_dev_session(dev);
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        events,
    }
}

fn rig() -> Rig {
    rig_dev(false)
}

fn root(r: &Rig) -> &Path {
    r.dir.path()
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

async fn write(r: &mut Rig, domain: &str, json: &str) -> Result<(), DesktopError> {
    r.desktop.set_section(domain, json, Origin::Settings).await
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn env_file(r: &Rig) -> Option<String> {
    std::fs::read_to_string(files::environment_d_path(&r.desktop.paths().startup)).ok()
}

fn state_toml(r: &Rig) -> Option<String> {
    std::fs::read_to_string(&r.desktop.paths().state_file).ok()
}

fn override_path(r: &Rig) -> PathBuf {
    files::override_target(&r.desktop.paths().startup, CODE).unwrap()
}

fn put_source(r: &Rig) {
    put(&root(r).join("share-dirs/applications").join(CODE), SOURCE);
}

fn dp1(r: &Rig, scale: f64) {
    *r.comp.monitors_json.lock().unwrap() =
        format!(r#"[{{"name":"DP-1","x":0,"y":0,"scale":{scale}}}]"#);
}

/// Two screens: DP-1 (1.25, at 0,0, serial A) and DP-2 (1.5, at 2560,0, serial B).
fn two_screens(r: &Rig) {
    let one = |name: &str, serial: &str, x: i32, scale: f64| {
        format!(
            r#"{{"name":"{name}","make":"Dell","model":"U2720Q","serial":"{serial}","x":{x},"y":0,"scale":{scale},"width":2560,"height":1440}}"#
        )
    };
    *r.comp.monitors_json.lock().unwrap() = format!(
        "[{},{}]",
        one("DP-1", "A", 0, 1.25),
        one("DP-2", "B", 2560, 1.5)
    );
}

fn id(serial: &str) -> MonitorId {
    MonitorId {
        make: "Dell".into(),
        model: "U2720Q".into(),
        serial: serial.into(),
    }
}

fn monitor_reads(r: &Rig) -> usize {
    r.comp.calls().iter().filter(|c| *c == "monitors").count()
}

async fn explain(r: &Rig, domain: &str) -> Value {
    r.desktop.explain(domain).await.unwrap()
}

async fn assert_refused(r: &mut Rig, message: &str) {
    let before = state_toml(r);
    let err = write(r, "compat", ELSEWHERE).await.unwrap_err();
    assert_eq!(err, DesktopError::InvalidSetting(message.into()));
    assert_eq!(state_toml(r), before);
    assert!(r.desktop.history(10).is_empty());
    assert!(r.desktop.state().compat.apps.is_empty());
}

/// Every regular file and symlink under `dir`.
fn tree(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => out.extend(tree(&p)),
            _ => out.push(p),
        }
    }
    out
}

fn inode(p: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(p).unwrap().ino()
}

const MINE_APPLET: &str = "[Desktop Entry]\nType=Application\nName=Mine\nExec=mine\n";

mod compat;
mod env;
mod explain_restore;
mod monitors;
