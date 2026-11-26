use super::*;
use crate::theme::tests::FIXTURE;
use std::fs;
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn dark_text() -> String {
    FIXTURE.to_string()
}

/// Gruvbox light, renamed to match the dark fixture.
fn light_text() -> String {
    let t = fs::read_to_string(repo().join("themes/gruvbox/light.toml")).unwrap();
    t.replacen("name = \"Gruvbox\"", "name = \"Fixture\"", 1)
}

fn dark() -> Theme {
    Theme::from_toml(&dark_text(), "user-x", Variant::Dark).unwrap()
}

fn light() -> Theme {
    Theme::from_toml(&light_text(), "user-x", Variant::Light).unwrap()
}

fn put(root: &Path, id: &str, d: &str, l: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("dark.toml"), d).unwrap();
    fs::write(dir.join("light.toml"), l).unwrap();
}

fn good(root: &Path, id: &str) {
    put(root, id, &dark_text(), &light_text());
}

const HOSTILE_NAMES: [&str; 14] = [
    "X\nshell /bin/sh -c evil",
    "X */ * { color: red } /*",
    "{{ 7*7 }}",
    "{% if 1 %}",
    "{# x",
    "X\u{202e}",
    "X\u{200b}",
    "X\r",
    "X\u{2028}shell /bin/sh -c evil",
    "X\u{2029}",
    "X\u{feff}",
    " X",
    "",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
];

mod check;
mod scan;
