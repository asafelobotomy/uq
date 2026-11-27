//! The Apps and scaling page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::compat::hypr::{PLATFORM_ENV, SwitchKey};
use uq_core::compat::valid_scale;
use uq_core::rules::WindowInfo;
use uq_core::startup_explain::{BrowserFile, OverrideState};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

fn app_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("code.desktop"),
        "[Desktop Entry]\nType=Application\nName=Visual Studio Code\nIcon=vscode\n\
         Exec=/usr/bin/code --unity-launch %F\nStartupWMClass=Code\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("chromium.desktop"),
        "[Desktop Entry]\nType=Application\nName=Chromium\nIcon=chromium\nExec=chromium %U\n",
    )
    .unwrap();
    d
}

fn win(class: &str, title: &str, xwayland: bool) -> WindowInfo {
    WindowInfo {
        class: class.into(),
        title: title.into(),
        initial_class: class.into(),
        initial_title: title.into(),
        xwayland,
        workspace_id: 1,
        workspace_name: "1".into(),
        ..Default::default()
    }
}

async fn rig_with(st: State) -> (Rig, CompatState, tempfile::TempDir) {
    let r = fake_uqd::rig(st).await;
    let dir = app_dir();
    let page = CompatState::with_dirs(r.client.clone(), vec![dir.path().to_path_buf()]);
    (r, page, dir)
}

async fn rig() -> (Rig, CompatState, tempfile::TempDir) {
    rig_with(State::default()).await
}

fn section(r: &Rig) -> Value {
    r.state.lock().unwrap().section("compat").unwrap()
}

fn switch<'a>(state: &'a Value, key: &str) -> &'a Value {
    state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("no switch {key} in {state}"))
}

fn save(fix: Value) -> Op {
    Op::SaveFix { fix }
}

fn code_fix() -> Value {
    json!({"id": "code", "platform": "x11", "scale": 1.5,
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"]})
}

fn entry(id: &str, wm: Option<&str>, exec: Option<&str>) -> AppEntry {
    AppEntry {
        id: id.into(),
        name: id.into(),
        icon: String::new(),
        terminal_emulator: false,
        wm_class: wm.map(String::from),
        exec_name: exec.map(String::from),
    }
}

mod fixes;
mod switches;
