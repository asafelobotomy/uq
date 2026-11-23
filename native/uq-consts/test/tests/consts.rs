//! Tests for the generated Rust uq-consts (edition 2021, as the oldest app that vendors it).
use std::ffi::OsString;
use std::path::PathBuf;
use uq_consts_test::*;

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |k| {
        pairs
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| OsString::from(*v))
    }
}

#[test]
fn an_absolute_xdg_state_home_wins() {
    let get = env(&[("HOME", "/home/a"), ("XDG_STATE_HOME", "/st")]);
    assert_eq!(paths::state_home_from(&get), Some(PathBuf::from("/st")));
}

#[test]
fn a_relative_or_empty_xdg_state_home_is_ignored() {
    for v in ["", "rel/state"] {
        let pairs: &'static [(&str, &str)] =
            Box::leak(Box::new([("HOME", "/home/a"), ("XDG_STATE_HOME", v)]));
        assert_eq!(
            paths::state_home_from(&env(pairs)),
            Some(PathBuf::from("/home/a/.local/state"))
        );
    }
}

#[test]
fn no_home_is_no_path() {
    assert_eq!(paths::state_home_from(&env(&[])), None);
    assert_eq!(paths::cache_home_from(&env(&[])), None);
}

#[test]
fn the_cache_home_has_its_own_fallback() {
    let get = env(&[("HOME", "/home/a")]);
    assert_eq!(
        paths::cache_home_from(&get),
        Some(PathBuf::from("/home/a/.cache"))
    );
}

#[test]
fn paths_sit_under_the_uq_folder() {
    for p in [paths::THEME_DIR, paths::TOKENS, paths::CRASHES_DIR] {
        assert!(p.starts_with(paths::UQ_DIR), "{p}");
    }
    for p in [
        install::PAGES_DIR,
        install::WIDGETS_DIR,
        install::LAUNCHER_DIR,
        install::ACTIONS_DIR,
        install::INTEGRATIONS_DIR,
        install::DIAG_DIR,
        install::THEMES_DIR,
    ] {
        assert!(p.starts_with(install::DATA_DIR), "{p}");
    }
}

#[test]
fn both_palettes_have_every_key_in_order() {
    for pal in [palette::DARK, palette::LIGHT] {
        let keys: Vec<&str> = pal.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, palette::KEYS);
        assert!(pal.iter().all(|(_, c)| c.len() == 7 && c.starts_with('#')));
    }
}

#[test]
fn the_version_is_the_header() {
    let header = include_str!("../../uq_consts.rs").lines().next().unwrap();
    assert_eq!(header, format!("//! uq-consts {VERSION}"));
}

#[test]
fn motion_is_ordered() {
    const { assert!(motion::FAST < motion::BASE && motion::BASE < motion::SLOW) };
    const { assert!(motion::POP_SCALE > 0.0 && motion::POP_SCALE <= 1.0) };
}
