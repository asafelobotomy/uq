use super::*;
use crate::theme::ThemeRegistry;
use crate::theme::tests::FIXTURE;
use serde_json::json;
use std::fs;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn light_text() -> String {
    fs::read_to_string(repo().join("themes/gruvbox/light.toml"))
        .unwrap()
        .replacen("name = \"Gruvbox\"", "name = \"Fixture\"", 1)
}

fn good() -> UqTheme {
    let d = Theme::from_toml(FIXTURE, "user-x", Variant::Dark).unwrap();
    let l = Theme::from_toml(&light_text(), "user-x", Variant::Light).unwrap();
    from_pair(&d, &l, Some("Sam"), None)
}

fn good_text() -> String {
    to_text(&good())
}

/// The good file as a table, changed by `f`, and written back.
fn edited(f: impl FnOnce(&mut toml::Table)) -> String {
    let mut t: toml::Table = good_text().parse().unwrap();
    f(&mut t);
    toml::to_string(&t).unwrap()
}

fn sub<'a>(t: &'a mut toml::Table, path: &[&str]) -> &'a mut toml::Table {
    let mut cur = t;
    for k in path {
        cur = cur.get_mut(*k).unwrap().as_table_mut().unwrap();
    }
    cur
}

fn val(v: impl Into<toml::Value>) -> toml::Value {
    v.into()
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

fn dark_problem(e: ImportError) -> user::Problem {
    match e {
        ImportError::Variant {
            variant: Variant::Dark,
            problem,
        } => problem,
        other => panic!("expected a dark version problem, got {other:?}"),
    }
}

/// xorshift64, so the fuzz run is the same every time.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

mod parse_refuses;
mod round_trips;
