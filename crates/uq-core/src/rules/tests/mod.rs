use super::*;
use crate::state::State;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const SPEC: &str = r#"
[[rules.window]]
id = "r-7f2c"
name = "Pavucontrol floats"
enabled = true
match = { class = "^(org\\.pulseaudio\\.pavucontrol)$" }
effects = { float = true, size = [800, 600], center = true }

[[rules.layer]]
id = "r-0a91"
name = "Blur notifications"
enabled = true
match = { namespace = "^noti-fi$" }
effects = { blur = true, ignore_alpha = 0.3 }

[[rules.workspace]]
workspace = "3"
monitor = "DP-1"
default = false
persistent = true
gaps_in = 0
"#;

fn win(name: &str, m: Value, e: Value) -> Value {
    json!({"name": name, "enabled": true, "match": m, "effects": e})
}

fn section(window: Vec<Value>, layer: Vec<Value>, workspace: Vec<Value>) -> Value {
    json!({"window": window, "layer": layer, "workspace": workspace})
}

fn one(m: Value, e: Value) -> Value {
    section(vec![win("A rule", m, e)], vec![], vec![])
}

fn parse(v: Value) -> Result<Rules, SettingError> {
    Rules::from_section(&v, &Rules::default())
}

fn key_of(r: Result<Rules, SettingError>) -> String {
    match r {
        Err(SettingError::Invalid { key, .. }) => key,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn reason_of(r: Result<(), SettingError>) -> String {
    match r {
        Err(SettingError::Invalid { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

mod checks;
mod ids;
mod load;
