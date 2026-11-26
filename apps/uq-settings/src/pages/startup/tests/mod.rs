//! The Startup page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

/// A system entry, a skipped one, and the person's own one, as uqd lists them.
fn found_rows() -> Vec<Value> {
    vec![
        json!({"id": "nm-applet.desktop", "name": "Network", "icon": "nm-applet",
               "source": "system", "enabled": true, "can_toggle": true, "delay": 0,
               "status": "running"}),
        json!({"id": "tracker.desktop", "name": "Tracker", "icon": null,
               "source": "system", "enabled": false, "can_toggle": false, "delay": 0,
               "status": "skipped", "reason": "Turned off by the system"}),
        json!({"id": "notes.desktop", "name": "Notes", "icon": null,
               "source": "yours", "enabled": true, "can_toggle": true, "delay": 5,
               "status": "exited"}),
    ]
}

fn app_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("syncthing-start.desktop"),
        "[Desktop Entry]\nType=Application\nName=Syncthing\nIcon=syncthing\nExec=syncthing\n",
    )
    .unwrap();
    let long = "L".repeat(150);
    std::fs::write(
        d.path().join("long.desktop"),
        format!("[Desktop Entry]\nType=Application\nName={long}\nExec=long\n"),
    )
    .unwrap();
    d
}

async fn rig_with(st: State) -> (Rig, StartupState, tempfile::TempDir) {
    let r = fake_uqd::rig(st).await;
    r.autostart.set(found_rows());
    let dir = app_dir();
    let page = StartupState::with_dirs(r.client.clone(), vec![dir.path().to_path_buf()]);
    (r, page, dir)
}

async fn rig() -> (Rig, StartupState, tempfile::TempDir) {
    rig_with(State::default()).await
}

fn app<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("no app {id} in {state}"))
}

fn on(id: &str, on: bool) -> Op {
    Op::SetEnabled { id: id.into(), on }
}

fn set_var(index: Option<usize>, name: &str, value: &str) -> Op {
    Op::SetVar {
        index,
        name: name.into(),
        value: value.into(),
    }
}

fn section(r: &Rig, domain: &str) -> Value {
    r.state.lock().unwrap().section(domain).unwrap()
}

fn added_ids(r: &Rig) -> Vec<String> {
    let st = r.state.lock().unwrap();
    st.autostart
        .added
        .iter()
        .map(|a| a.id().to_string())
        .collect()
}

mod entries;
mod env;
mod run;
