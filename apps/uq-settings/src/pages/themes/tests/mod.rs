//! The theme row's logic against the fake uqd.

use super::editor::draft_colors;
use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};
use uq_core::theme::derive;
use uq_core::theme::{Theme, Variant};

async fn rig() -> (Rig, ThemesState, tempfile::TempDir) {
    let mut state = State {
        theme: "gruvbox".into(),
        ..State::default()
    };
    state
        .set_section("look", json!({"gaps_in": 8, "blur": true}))
        .unwrap();
    let r = fake_uqd::rig(state).await;
    let page = ThemesState::new(r.client.clone());
    (r, page, tempfile::tempdir().unwrap())
}

/// A `.uqtheme` for slug `slug` named `name`, made from gruvbox.
fn text(r: &Rig, slug: &str, name: &str) -> String {
    r.themes
        .text_of("gruvbox")
        .unwrap()
        .replacen("id = \"gruvbox\"", &format!("id = \"{slug}\""), 1)
        .replacen("name = \"Gruvbox\"", &format!("name = \"{name}\""), 1)
}

fn file(dir: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn import(path: &Path, replace: bool, with_look: bool) -> Op {
    Op::Import {
        path: path.to_path_buf(),
        replace,
        with_look,
    }
}

use std::path::Path;

fn row<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["themes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap_or_else(|| panic!("no theme {id} in {}", state["themes"]))
}

fn ids(state: &Value) -> Vec<String> {
    state["themes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

/// The editor's own colours as a draft with `name`.
fn draft(state: &Value, name: &str) -> Value {
    json!({
        "name": name,
        "author": "",
        "dark": state["editor"]["colors"]["dark"],
        "light": state["editor"]["colors"]["light"],
    })
}

/// What the QML tests' fixture holds.
const FIXTURE: &str = include_str!("../../../../../../tests/settings-qml/fixtures/themes.js");

fn fixture(name: &str) -> Value {
    let lead = format!("var {name} = ");
    let start = FIXTURE.find(&lead).expect("the fixture sets it") + lead.len();
    let rest = &FIXTURE[start..];
    let end = rest
        .find("\n;")
        .expect("the fixture ends it with a `;` line");
    serde_json::from_str(&rest[..end]).expect("the fixture is JSON")
}

mod editor;
mod store;
