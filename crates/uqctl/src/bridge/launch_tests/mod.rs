use super::*;
use crate::bridge::run::{RunState, run_item};

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

use serde_json::json;

fn dirs_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Dirs) {
    let t = tempfile::tempdir().unwrap();
    let apps = t.path().join("applications");
    std::fs::create_dir_all(&apps).unwrap();
    for (n, c) in files {
        std::fs::write(apps.join(n), c).unwrap();
    }
    let d = Dirs::new(Some(t.path().join("none/applications")), vec![apps]);
    (t, d)
}

fn htop_plan(d: &Dirs, terminal: Option<&str>) -> Plan {
    plan(
        Some(&json!({"desktop_id":"t.desktop"})),
        d,
        0,
        &RunState::default(),
        terminal,
        None,
    )
    .unwrap()
}

const HTOP: (&str, &str) = ("t.desktop", "[Desktop Entry]\nExec=htop\nTerminal=true\n");

mod scope;
mod terminal;
