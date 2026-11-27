//! The Profiles page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::history::Entry;
use uq_core::profile::power_notice;
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};
use uq_core::theme::Variant;

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";
const TURBO: &str = "format = 1\nid = \"turbo\"\nname = \"Turbo\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Gaps 8, blur on, theme gruvbox (installed in the fake) in its light variant.
fn state() -> State {
    let mut st = State::default();
    st.set_section("look", json!({"gaps_in": 8, "blur": true}))
        .unwrap();
    st.theme = "gruvbox".into();
    st.variant = Variant::Light;
    st
}

async fn rig() -> (Rig, ProfilesState) {
    let r = fake_uqd::rig(state()).await;
    let page = ProfilesState::new(r.client.clone());
    (r, page)
}

fn seed(r: &Rig, text: &str) -> Profile {
    let p = Profile::parse(text).unwrap();
    r.profiles
        .store
        .lock()
        .unwrap()
        .insert(p.id.clone(), p.clone());
    p
}

fn stored(r: &Rig, id: &str) -> Option<Profile> {
    r.profiles.store.lock().unwrap().get(id).cloned()
}

fn gaps(r: &Rig) -> Option<i64> {
    r.state.lock().unwrap().section("look").unwrap()["gaps_in"].as_i64()
}

/// The `{domain}.{path}` of every key the snapshot shows for profile `i`.
fn shown_keys(state: &Value, i: usize) -> Vec<String> {
    state["profiles"][i]["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let path: Vec<&str> = k["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect();
            format!("{}.{}", k["domain"].as_str().unwrap(), path.join("."))
        })
        .collect()
}

/// Window rules "Float A" (on) and "B" (off), panel rule "Blur notifications" (on), and a
/// workspace rule, which has no switch.
fn rules_state() -> State {
    let mut st = state();
    let win = |id: &str, name: &str, on: bool| {
        json!({"id": id, "name": name, "enabled": on, "match": {"class": "a"},
            "effects": {"float": true}})
    };
    st.set_section(
        "rules",
        json!({"window": [win("r-000a", "Float A", true), win("r-000b", "B", false)],
            "layer": [{"id": "r-00c0", "name": "Blur notifications", "enabled": true,
                "match": {"namespace": "n"}, "effects": {"blur": true}}],
            "workspace": [{"workspace": "3", "persistent": true}]}),
    )
    .unwrap();
    st
}

async fn rules_rig() -> (Rig, ProfilesState) {
    let r = fake_uqd::rig(rules_state()).await;
    let page = ProfilesState::new(r.client.clone());
    (r, page)
}

const QUIET: &str = "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[rules]\n\"r-000a\" = false\n\"r-ffff\" = true\n";

fn rule_keys_of(list: &Value) -> Vec<Value> {
    list.as_array()
        .unwrap()
        .iter()
        .filter(|k| k["domain"] == "rules")
        .cloned()
        .collect()
}

/// One of your own shortcuts, off, beside uQ's own.
fn binds_state() -> State {
    let mut st = state();
    st.set_section(
        "keybinds",
        json!({"custom": [{"id": "k-00a1", "name": "Shot", "combo": "SUPER + F9",
            "enabled": false, "action": {"kind": "shell", "name": "lock"}}]}),
    )
    .unwrap();
    st
}

async fn binds_rig() -> (Rig, ProfilesState) {
    let r = fake_uqd::rig(binds_state()).await;
    let page = ProfilesState::new(r.client.clone());
    (r, page)
}

const BINDS_QUIET: &str = "format = 1\nid = \"quiet\"\nname = \"Quiet\"\n[keybinds]\n\"uq.close\" = false\n\"k-00a1\" = true\n";

fn bind_keys_of(list: &Value) -> Vec<Value> {
    list.as_array()
        .unwrap()
        .iter()
        .filter(|k| k["domain"] == "keybinds")
        .cloned()
        .collect()
}

mod apply_share;
mod create;
mod rules_keybinds;
