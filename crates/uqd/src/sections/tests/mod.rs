use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;
use uq_core::history::Origin;
use uq_core::paths::Paths;
use uq_core::state::State;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    sys: Arc<FakeSystem>,
    events: broadcast::Receiver<Event>,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let kv = dir.path().join("kvantum");
    std::fs::create_dir_all(kv.join("KvGnome")).unwrap();
    std::fs::write(kv.join("KvGnome/KvGnome.kvconfig"), "").unwrap();
    let comp = Arc::new(FakeCompositor::running());
    let sys = Arc::new(FakeSystem::default());
    let desktop = Desktop::new(paths, comp.clone(), sys.clone())
        .unwrap()
        .with_kvantum_dir(kv);
    let events = desktop.subscribe();
    Rig {
        _dir: dir,
        desktop,
        comp,
        sys,
        events,
    }
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn kvconfig(r: &Rig) -> PathBuf {
    r.desktop.paths().theme_dir.join("Kvantum/uq/uq.kvconfig")
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap()
}

mod appearance;
mod journal;
mod look_input;
