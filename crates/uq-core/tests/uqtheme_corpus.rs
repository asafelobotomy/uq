//! The `.uqtheme` fixtures: one good file per system theme (checked in, rewritten with
//! `UQ_BLESS=1`), and a hostile corpus that must be refused with a plain sentence.

use std::fs;
use std::path::PathBuf;
use uq_core::theme::uqtheme::{self, MAX_FILE_BYTES};
use uq_core::theme::{ThemeRegistry, Variant};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures(kind: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/uqtheme")
        .join(kind)
}

fn files(kind: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(fixtures(kind))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "uqtheme"))
        .collect();
    v.sort();
    v
}

#[test]
fn good_fixtures_parse_and_match_system_themes() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let bless = std::env::var_os("UQ_BLESS").is_some();
    for id in reg.ids() {
        let dark = reg.get(id, Variant::Dark).unwrap();
        let light = reg.get(id, Variant::Light).unwrap();
        let author = reg.info(id).and_then(|i| i.author.clone());
        let text = uqtheme::to_text(&uqtheme::from_pair(dark, light, author.as_deref(), None));
        let path = fixtures("good").join(format!("{id}.uqtheme"));
        if bless {
            fs::write(&path, &text).unwrap();
        }
        let on_disk = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing {}; run with UQ_BLESS=1", path.display()));
        assert_eq!(on_disk, text, "{id}: run with UQ_BLESS=1");
        let parsed = uqtheme::parse(&on_disk).unwrap();
        assert_eq!(parsed.dark.colors, dark.colors, "{id} dark");
        assert_eq!(parsed.light.colors, light.colors, "{id} light");
        assert_eq!(parsed.dark.terminal, dark.terminal, "{id}");
        assert_eq!(uqtheme::to_text(&parsed), on_disk, "{id} is stable");
    }
    assert_eq!(files("good").len(), reg.ids().len());
}

/// Words a person must not read in a refusal.
const JARGON: [&str; 8] = [
    "toml",
    "serde",
    "uqd",
    "themeerror",
    "{",
    "[",
    "panicked",
    "unwrap",
];

fn plain_refusal(what: &str, e: &uqtheme::ImportError) {
    let m = e.to_string();
    assert!(!m.is_empty(), "{what}");
    assert!(!m.chars().any(char::is_control), "{what}: {m:?}");
    let lower = m.to_lowercase();
    for word in JARGON {
        assert!(!lower.contains(word), "{what}: {m:?} has {word:?}");
    }
}

#[test]
fn bad_fixtures_are_all_refused_plainly() {
    let bad = files("bad");
    for prefix in ["name-", "id-", "schema-", "contrast-"] {
        assert!(
            bad.iter()
                .any(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix)),
            "no bad fixture starts with {prefix}"
        );
    }
    for path in &bad {
        let what = path.file_name().unwrap().to_string_lossy().into_owned();
        // The reader and the parser together, as the app and uqctl use them.
        let refused = uqtheme::read_file(path).and_then(|t| uqtheme::parse(&t).map(|_| ()));
        match refused {
            Ok(()) => panic!("{what} was accepted"),
            Err(e) => plain_refusal(&what, &e),
        }
    }
}

#[test]
fn made_at_test_time_files_are_refused_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.uqtheme");
    fs::write(&big, vec![b'#'; MAX_FILE_BYTES as usize + 1]).unwrap();
    let e = uqtheme::read_file(&big).unwrap_err();
    plain_refusal("65537 bytes", &e);
    // A file of exactly the limit is read (and then isn't a theme).
    let edge = dir.path().join("edge.uqtheme");
    fs::write(&edge, vec![b'#'; MAX_FILE_BYTES as usize]).unwrap();
    let text = uqtheme::read_file(&edge).unwrap();
    plain_refusal("64 KiB of comment", &uqtheme::parse(&text).unwrap_err());

    let fifo = dir.path().join("x.uqtheme");
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    if made.is_ok_and(|s| s.success()) {
        // Must return at once, not wait for a writer.
        let e = uqtheme::read_file(&fifo).unwrap_err();
        plain_refusal("fifo", &e);
    }
}

#[test]
#[ignore = "prints each refusal"]
fn print_refusals() {
    for path in files("bad") {
        let e = uqtheme::read_file(&path).and_then(|t| uqtheme::parse(&t).map(|_| ()));
        println!(
            "{}: {}",
            path.file_name().unwrap().to_string_lossy(),
            e.unwrap_err()
        );
    }
}
