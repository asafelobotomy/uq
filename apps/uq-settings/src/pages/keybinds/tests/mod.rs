//! The Keyboard shortcuts page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

async fn rig() -> (Rig, KeybindsState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = KeybindsState::with_dirs(r.client.clone(), Vec::new());
    (r, page)
}

async fn op(page: &KeybindsState, op: Op) -> Outcome {
    run(page, op).await
}

fn row<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["binds"].as_array().unwrap())
        .chain(state["custom"].as_array().unwrap())
        .find(|b| b["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn custom(name: &str, combo: &str) -> Value {
    json!({"name": name, "combo": combo, "action": {"kind": "uq", "name": "terminal"}})
}

fn custom_ids(r: &Rig) -> Vec<String> {
    r.state
        .lock()
        .unwrap()
        .keybinds
        .custom
        .iter()
        .map(|b| b.id.clone())
        .collect()
}

mod edit;
mod page;
