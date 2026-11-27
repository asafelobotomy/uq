//! The Shell page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::state::State;

async fn rig_in(app_dirs: Vec<PathBuf>, provider_dirs: Vec<PathBuf>) -> (Rig, ShellState) {
    let mut state = State::default();
    state
        .set_section(
            "shell",
            json!({"islands": {"work": false}, "clock_24h": false,
                   "providers": [{"id": "run", "enabled": false}, {"id": "org.gone.P"}],
                   "terminal": "kitty.desktop"}),
        )
        .unwrap();
    state.settings.pinned = vec!["a.desktop".into(), "b.desktop".into()];
    let r = fake_uqd::rig(state).await;
    let page = ShellState::with_dirs(r.client.clone(), app_dirs, provider_dirs);
    (r, page)
}

async fn rig() -> (Rig, ShellState) {
    rig_in(vec![], vec![]).await
}

fn shell(field: &str, value: Value) -> Op {
    Op::SetShell {
        field: field.into(),
        value,
    }
}

fn setting(key: &str, value: Value) -> Op {
    Op::SetSetting {
        key: key.into(),
        value,
    }
}

fn pin_move(id: &str, from: usize, to: usize) -> Op {
    Op::PinMove {
        id: id.into(),
        from,
        to,
    }
}

fn bar(v: Value) -> Op {
    Op::Bar {
        edit: serde_json::from_value(v).unwrap(),
    }
}

fn zone_ids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect()
}

mod fields;
mod pins_bar;
