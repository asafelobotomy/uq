use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::paths::Paths;

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn paths(dir: &Path) -> Paths {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    paths
}

fn desktop_on(dir: &Path, comp: Arc<FakeCompositor>) -> Desktop {
    Desktop::new(paths(dir), comp, Arc::new(FakeSystem::default())).unwrap()
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let comp = Arc::new(FakeCompositor::running());
    let mut desktop = desktop_on(dir.path(), comp.clone());
    desktop.startup().await;
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        events,
    }
}

impl Rig {
    fn root(&self) -> PathBuf {
        self.desktop.paths().user_themes_dir.clone()
    }

    fn events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.events.try_recv().ok()).collect()
    }

    fn themes_changed(&mut self) -> usize {
        self.events()
            .iter()
            .filter(|e| matches!(e, Event::ThemesChanged))
            .count()
    }

    async fn import(
        &mut self,
        text: &str,
        replace: bool,
        look: bool,
    ) -> Result<String, DesktopError> {
        self.desktop
            .import_theme(text, replace, look, Origin::Settings)
            .await
    }

    fn user_ids(&mut self) -> Vec<String> {
        self.desktop.theme_info()["themes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["user"] == true)
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    }
}

/// A theme file for slug `slug` named `name`, made from gruvbox.
fn file(d: &Desktop, slug: &str, name: &str) -> String {
    d.export_theme("gruvbox", false)
        .unwrap()
        .replacen("id = \"gruvbox\"", &format!("id = \"{slug}\""), 1)
        .replacen("name = \"Gruvbox\"", &format!("name = \"{name}\""), 1)
}

fn with_look(text: &str) -> String {
    format!("{text}\n[look]\ngaps_in = 3\n")
}

fn write_theme(root: &Path, id: &str) {
    let d = tempfile::tempdir().unwrap();
    let desk = desktop_on(d.path(), Arc::new(FakeCompositor::running()));
    let t = uqtheme::parse(&file(&desk, id.trim_start_matches("user-"), "Hand")).unwrap();
    atomic_replace_dir(&root.join(id), &uqtheme::install_files(&t)).unwrap();
}

fn read(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn mtime(p: &Path) -> std::time::SystemTime {
    std::fs::metadata(p).unwrap().modified().unwrap()
}

mod delete;
mod import;
mod store;
