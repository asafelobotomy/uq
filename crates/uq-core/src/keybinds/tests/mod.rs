use super::*;
use crate::state::State;
use serde_json::json;

fn write(st: &mut State, v: Value) -> Result<(), SettingError> {
    st.set_section("keybinds", v)
}

fn err(v: Value) -> String {
    let mut st = State::default();
    write(&mut st, v).unwrap_err().to_string()
}

fn custom(name: &str, combo: &str, action: Value) -> Value {
    json!({"name": name, "combo": combo, "action": action})
}

fn shell(name: &str) -> Value {
    json!({"kind": "shell", "name": name})
}

fn term() -> Value {
    json!({"kind": "uq", "name": "terminal"})
}

mod checks;
mod parse;
