//! uq-consts (`native/uq-consts/consts.toml`) is what uQ hands Bindle and its apps. These tests
//! hold uQ's own code to it, so the shell, uqd and the kits can't drift apart.

use std::path::{Path, PathBuf};
use uq_core::uq_consts::{api, dbus, install, motion, palette, paths, shape};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn consts() -> toml::Table {
    let text = std::fs::read_to_string(repo().join("native/uq-consts/consts.toml")).unwrap();
    toml::from_str(&text).unwrap()
}

/// `readonly property int <name>: ... <number>` style lines: the last integer on the line
/// that declares `name`.
fn qml_number(file: &str, name: &str) -> f64 {
    let text = std::fs::read_to_string(repo().join(file)).unwrap();
    let line = text
        .lines()
        .find(|l| l.contains(&format!(" {name}:")))
        .unwrap_or_else(|| panic!("{file} has no {name}"));
    let num: String = line
        .rsplit(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|w| !w.is_empty() && w.parse::<f64>().is_ok())
        .unwrap()
        .to_string();
    num.parse().unwrap()
}

#[test]
fn the_generated_rust_is_the_source() {
    let c = consts();
    assert_eq!(c["version"].as_str(), Some(uq_core::uq_consts::VERSION));
    assert_eq!(c["motion"]["fast"].as_integer(), Some(motion::FAST.into()));
    assert_eq!(c["dbus"]["desktop_name"].as_str(), Some(dbus::DESKTOP_NAME));
    assert_eq!(c["api"]["uq_api"].as_integer(), Some(api::UQ_API.into()));
}

#[test]
fn the_shell_moves_and_rounds_as_the_apps_do() {
    assert_eq!(
        qml_number("shell/core/Motion.qml", "fast"),
        motion::FAST as f64
    );
    assert_eq!(
        qml_number("shell/core/Motion.qml", "base"),
        motion::BASE as f64
    );
    assert_eq!(
        qml_number("shell/core/Motion.qml", "slow"),
        motion::SLOW as f64
    );
    assert_eq!(
        qml_number("shell/core/Motion.qml", "popScale"),
        motion::POP_SCALE
    );
    let m = std::fs::read_to_string(repo().join("shell/core/Motion.qml")).unwrap();
    assert!(m.contains(&format!("easing: Easing.{}", motion::EASING)));
    assert!(m.contains(&format!("easingExit: Easing.{}", motion::EASING_EXIT)));
    let t = std::fs::read_to_string(repo().join("shell/core/Theme.qml")).unwrap();
    assert!(t.contains(&format!(
        "radiusControl: Math.max(0, radius - {})",
        shape::CONTROL_INSET
    )));
    assert!(t.contains(&format!(
        "radiusInner: Math.max(0, radius - {})",
        shape::INNER_INSET
    )));
    assert_eq!(
        qml_number("shell/core/Theme.qml", "radius"),
        shape::FALLBACK_RADIUS as f64
    );
}

#[test]
fn the_fallback_palette_is_graphite() {
    let reg = uq_core::theme::ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    for (variant, pal) in [
        (uq_core::theme::Variant::Dark, palette::DARK),
        (uq_core::theme::Variant::Light, palette::LIGHT),
    ] {
        let t = reg.get("graphite", variant).unwrap();
        let colors = serde_json::to_value(&t.colors).unwrap();
        for (k, v) in pal {
            assert_eq!(colors[k], *v, "graphite {variant} {k}");
        }
        assert_eq!(colors.as_object().unwrap().len(), palette::KEYS.len());
    }
}

#[test]
fn uq_reads_the_paths_it_hands_out() {
    let get = |k: &str| match k {
        "HOME" => Some("/home/a".to_string()),
        _ => None,
    };
    let p = uq_core::paths::Paths::from_vars(get);
    let state = paths::state_home_from(&|k| get(k).map(Into::into)).unwrap();
    assert_eq!(p.theme_dir, state.join(paths::THEME_DIR));
    assert_eq!(
        uq_core::diag::log::crash_dir_from(&get),
        state.join(paths::CRASHES_DIR)
    );
    let crash = uq_core::diag::crash::Dirs::from_vars(&get);
    assert_eq!(
        crash.quickshell,
        Path::new("/home/a/.cache").join(paths::QUICKSHELL_CRASHES_DIR)
    );
    assert_eq!(p.themes_dir, Path::new(install::THEMES_DIR));
    assert_eq!(p.integrations_dir, Path::new(install::INTEGRATIONS_DIR));
    assert_eq!(
        p.themes_dir.join(uq_core::theme::RENDERED_DIR),
        Path::new(install::RENDERED_THEMES_DIR)
    );
}

#[test]
fn tokens_json_carries_motion() {
    let reg = uq_core::theme::ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.get("graphite", uq_core::theme::Variant::Dark).unwrap();
    let tokens = |reduce| {
        let out = uq_core::render::render_with(
            t,
            &repo().join("templates"),
            None,
            uq_core::render::Motion::new(reduce),
        )
        .unwrap();
        serde_json::from_str::<serde_json::Value>(&out.0[Path::new("tokens.json")]).unwrap()
    };
    assert_eq!(
        tokens(false)["motion"],
        serde_json::json!({"fast": motion::FAST, "base": motion::BASE, "slow": motion::SLOW, "reduce": false})
    );
    assert_eq!(
        tokens(true)["motion"],
        serde_json::json!({"fast": 0, "base": 0, "slow": 0, "reduce": true})
    );
    assert_eq!(tokens(true)["colors"]["bg"], palette::DARK[0].1);
}
