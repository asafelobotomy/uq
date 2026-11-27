use super::*;
use crate::history::Entry;
use crate::profile::{
    ActiveProfile, Profile, apply, changed_since, key_label, value_text, write_leaves as write,
};
use serde_json::json;
use std::collections::BTreeSet;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn with(body: &str) -> String {
    format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
}

/// The refusal's key and its one-line, plain text.
fn refused(body: &str) -> (String, String) {
    match Profile::parse(&with(body)) {
        Err(e @ ProfileError::Invalid { .. }) => {
            let ProfileError::Invalid { key, .. } = &e else {
                unreachable!()
            };
            let text = e.to_string();
            assert!(
                !text.contains('\n') && !text.chars().any(char::is_control),
                "{text:?}"
            );
            assert!(!text.contains("env.") && !text.contains('{'), "{text:?}");
            (key.clone(), text)
        }
        other => panic!("{body:?} gave {other:?}"),
    }
}

fn state() -> State {
    let mut st = State::default();
    st.set_section(
        "env",
        json!({"vars": [{"name": "A", "value": "1"}, {"name": "B", "value": "2"}]}),
    )
    .unwrap();
    st.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "wayland", "scale": 1.5}]}),
    )
    .unwrap();
    st.set_section(
        "autostart",
        json!({"files": [{"id": "nm-applet.desktop", "enabled": false}],
            "added": [{"id": "a-3f9c", "name": "Syncthing", "app": "syncthing-start.desktop",
                "enabled": true, "delay": 10}]}),
    )
    .unwrap();
    st
}

mod leaves;
mod tree;
