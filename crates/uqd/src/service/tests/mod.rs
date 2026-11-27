use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::fs;
use std::path::PathBuf;
use uq_core::history::Origin;
use uq_core::lua::Layout;
use uq_core::schedule::Schedule;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    sys: Arc<FakeSystem>,
    events: broadcast::Receiver<Event>,
}

fn rig_with(running: bool, prepare: impl FnOnce(&Paths)) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    prepare(&paths);
    let comp = Arc::new(FakeCompositor::default());
    *comp.running.lock().unwrap() = running;
    let sys = Arc::new(FakeSystem::default());
    let desktop = Desktop::new(paths, comp.clone(), sys.clone()).unwrap();
    let events = desktop.subscribe();
    Rig {
        _dir: dir,
        desktop,
        comp,
        sys,
        events,
    }
}

fn rig() -> Rig {
    rig_with(true, |_| {})
}

/// Every event so far.
fn drain_all(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

/// Events so far, leaving out `HistoryChanged`, which every journalled write adds.
fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    let mut evs = drain_all(rx);
    evs.retain(|e| *e != Event::HistoryChanged);
    evs
}

fn counts(r: &Rig) -> BTreeMap<String, u32> {
    r.desktop.state().launch_counts.clone()
}

fn journal_of(r: &Rig) -> Vec<uq_core::history::Entry> {
    Journal::open(&r.desktop.paths().history_dir).list(100)
}

fn user_lua(r: &Rig, text: &str) {
    let p = &r.desktop.paths().user_lua;
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

mod explain;
mod history;
mod launch;
mod layout_schedule;
mod settings;
mod theme;
mod verify_startup;
