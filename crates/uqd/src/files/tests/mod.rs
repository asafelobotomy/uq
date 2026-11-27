use super::*;
use std::path::Path;
use uq_core::autostart::FileChoice;
use uq_core::autostart::scan::read_folders;
use uq_core::compat::launch::OVERRIDE_HEADER;
use uq_core::compat::{AppFix, Platform};
use uq_core::env::{Env, Var, environment_d_lines};
use uq_core::startup_explain::OverrideState;

const CODE: &str = "code.desktop";
const SOURCE: &str = "[Desktop Entry]\nType=Application\nName=Code\nExec=/usr/bin/code %F\n";
const MINE: &str = "[Desktop Entry]\nType=Application\nName=Mine\nExec=mine\n";
const APPLET: &str = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";

struct Dirs {
    _dir: tempfile::TempDir,
    root: PathBuf,
    p: StartupPaths,
}

fn dirs() -> Dirs {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let p = StartupPaths::under(&root);
    Dirs { _dir: dir, root, p }
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn vars(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect()
}

/// No leftover temp files next to `path`.
fn no_temps(path: &Path) {
    let dir = path.parent().unwrap();
    for e in std::fs::read_dir(dir).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        assert!(!name.contains(".uq-") && !name.contains(".tmp-"), "{name}");
    }
}

// --- flag files ---

fn chromium_on() -> Compat {
    Compat {
        chromium_wayland: true,
        ..Compat::default()
    }
}

fn only(ids: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |id| ids.contains(&id)
}

fn flags(d: &Dirs, name: &str) -> PathBuf {
    d.p.config_home.join(name)
}

// --- overrides ---

fn source(d: &Dirs) -> PathBuf {
    d.root.join("share-dirs/applications").join(CODE)
}

fn target(d: &Dirs) -> PathBuf {
    d.p.data_home.join("applications").join(CODE)
}

fn fix(elsewhere: bool) -> Compat {
    let mut f = AppFix::new(CODE);
    f.platform = Platform::Wayland;
    f.elsewhere = elsewhere;
    Compat {
        apps: vec![f],
        ..Compat::default()
    }
}

fn state_of(r: &BTreeMap<String, OverrideState>) -> OverrideState {
    r.get(CODE).cloned().unwrap()
}

use std::collections::BTreeMap;

// --- hidden autostart copies ---

fn system_entry(d: &Dirs, id: &str, text: &str) {
    put(&d.root.join("xdg/autostart").join(id), text);
}

fn user_entry(d: &Dirs, id: &str) -> PathBuf {
    d.p.user_autostart().join(id)
}

fn found(d: &Dirs) -> Vec<uq_core::autostart::scan::Found> {
    read_folders(&d.p.user_autostart(), &d.p.system_autostart())
}

fn off(id: &str) -> Autostart {
    Autostart {
        files: vec![FileChoice {
            id: id.into(),
            enabled: Some(false),
            delay: None,
            minimize: None,
        }],
        added: Vec::new(),
        ..Autostart::default()
    }
}

use uq_core::autostart::Autostart;

mod env_flags;
mod overrides;
