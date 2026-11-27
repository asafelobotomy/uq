//! The Window rules page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::rules::hypr;
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

fn win(class: &str, title: &str) -> WindowInfo {
    WindowInfo {
        class: class.into(),
        title: title.into(),
        initial_class: class.into(),
        initial_title: title.into(),
        workspace_id: 1,
        workspace_name: "1".into(),
        ..Default::default()
    }
}

fn windows() -> Vec<WindowInfo> {
    vec![
        win("firefox", "Mozilla Firefox"),
        win("kitty", "~"),
        win("org.pulseaudio.pavucontrol", "Volume Control"),
    ]
}

fn state_with(rules: Value) -> State {
    let mut st = State::default();
    st.set_section("rules", rules).unwrap();
    st
}

async fn rig_with(st: State) -> (Rig, RulesState) {
    let r = fake_uqd::rig(st).await;
    r.windows.set(windows());
    let page = RulesState::new(r.client.clone());
    (r, page)
}

async fn rig() -> (Rig, RulesState) {
    rig_with(State::default()).await
}

fn rule(name: &str, class: &str, effects: Value) -> Value {
    json!({"name": name, "match": {"class": class}, "effects": effects})
}

fn ids(r: &Rig) -> Vec<String> {
    r.state
        .lock()
        .unwrap()
        .rules
        .window
        .iter()
        .map(|x| x.id.clone())
        .collect()
}

fn names(state: &Value, list: &str) -> Vec<String> {
    state[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect()
}

/// Save as the editor does: a window or panel rule with an id replaces that rule; anything else
/// (and every workspace rule) is an add.
async fn save(page: &RulesState, kind: &str, rule: Value) -> crate::pages::runner::Outcome {
    let replaces = match rule.get("id").and_then(Value::as_str) {
        Some(id) if kind != "workspace" && !id.is_empty() => Some(id.to_string()),
        _ => None,
    };
    save_as(page, kind, rule, replaces).await
}

async fn save_as(
    page: &RulesState,
    kind: &str,
    rule: Value,
    replaces: Option<String>,
) -> crate::pages::runner::Outcome {
    run(
        page,
        Op::Save {
            kind: kind.into(),
            rule,
            replaces,
        },
    )
    .await
}

fn ws_state() -> State {
    state_with(json!({"workspace": [
        {"workspace": "3", "monitor": "DP-1", "persistent": true},
        {"workspace": "name:web", "persistent": true},
    ]}))
}

const ROMFORGE: &str = r#"
[[window_rule]]
id = "romforge.emulators"
name = "Emulators open full screen"
class = ["^(retroarch|rpcs3)$"]
workspace = "name:game"
fullscreen = true
"#;

mod apps;
mod edit;
mod preview_pick;
mod workspace;
