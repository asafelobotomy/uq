//! A bad value in one domain of `state.toml` costs only that domain (or field): the theme,
//! pins, other workspace rules and every other domain load, and the file is not moved aside.

use super::*;

const BASE: &str = "theme = \"rosewood\"\npinned = [\"firefox.desktop\", \"foot.desktop\"]\n";
const APPEARANCE: &str = "[appearance]\ncursor_size = 32\n";
const RULES: &str = "[layouts]\n\"1\" = \"master\"\n\"3\" = \"scrolling\"\n";

fn load_with(extra: &str) -> (State, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, format!("{BASE}{extra}{APPEARANCE}{RULES}")).unwrap();
    let s = State::load(&p);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        ["state.toml"],
        "{extra}: the file is not moved aside"
    );
    (s, dir)
}

fn assert_kept(s: &State, extra: &str) {
    assert_eq!(s.theme, "rosewood", "{extra}");
    assert_eq!(
        s.settings.pinned,
        ["firefox.desktop", "foot.desktop"],
        "{extra}"
    );
    assert_eq!(s.settings.appearance.cursor_size, Some(32), "{extra}");
    assert_eq!(s.layouts.get("1"), Some(&Layout::Master), "{extra}");
    assert_eq!(s.layouts.get("3"), Some(&Layout::Scrolling), "{extra}");
}

#[test]
fn bad_look_values_keep_state() {
    for extra in [
        "[look]\ngaps_in = 300\ngaps_out = 7\n",
        "[look]\nblur = 1\ngaps_out = 7\n",
        "[look]\nrounding = 41\ngaps_out = 7\n",
    ] {
        let (s, _d) = load_with(extra);
        assert_kept(&s, extra);
        assert_eq!(s.settings.look.rounding, None, "{extra}");
        assert_eq!(s.settings.look.blur, None, "{extra}");
        assert_eq!(s.settings.look.gaps_in, None, "{extra}");
    }
    // Per-field leniency keeps the good neighbour of a wrong-typed value.
    let (s, _d) = load_with("[look]\nblur = 1\ngaps_out = 7\n");
    assert_eq!(s.settings.look.gaps_out, Some(7));
    let (s, _d) = load_with("look = 5\n");
    assert_kept(&s, "look = 5");
}

#[test]
fn bad_layout_options_keep_state() {
    for extra in [
        "[layout_options]\ndwindle = 3\ndefault = \"master\"\n",
        "[layout_options]\ndefault = \"master\"\n[layout_options.master]\nmfact = 0.95\n",
        "layout_options = 3\n",
    ] {
        let (s, _d) = load_with(extra);
        assert_kept(&s, extra);
        assert_eq!(s.settings.layout_options.master.mfact, None, "{extra}");
    }
    let (s, _d) = load_with("[layout_options]\ndwindle = 3\ndefault = \"master\"\n");
    assert_eq!(s.settings.layout_options.default, Some(Layout::Master));
}

#[test]
fn unknown_workspace_layout_drops_only_that_rule() {
    let (s, _d) = load_with("");
    assert_eq!(s.layouts.len(), 2);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        format!("{BASE}{APPEARANCE}[layouts]\n\"1\" = \"master\"\n\"2\" = \"monocle\"\n\"3\" = \"scrolling\"\n"),
    )
    .unwrap();
    let s = State::load(&p);
    assert_kept(&s, "monocle");
    assert_eq!(s.layouts.get("2"), None);
    assert!(p.exists());
}

#[test]
fn bad_shell_value_keeps_state() {
    let extra = "[shell]\nclock_24h = 3\nterminal = \"foot.desktop\"\n";
    let (s, _d) = load_with(extra);
    assert_kept(&s, extra);
    assert!(s.settings.shell.clock_24h);
    assert_eq!(s.settings.shell.terminal.as_deref(), Some("foot.desktop"));
}

#[test]
fn bad_domain_falls_back_alone() {
    // Each of these parses but breaks a rule; only its own domain resets.
    for extra in [
        "[idle]\ndim = 500\nlock = 300\n",
        "[mouse]\nscroll_factor = 50.0\n",
        "[wallpaper.outputs.\"*\"]\nimage = \"relative.png\"\n",
    ] {
        let (s, _d) = load_with(extra);
        assert_kept(&s, extra);
    }
    let (s, _d) =
        load_with("[mouse]\nscroll_factor = 50.0\nleft_handed = true\n[idle]\nlock = 250\n");
    assert_eq!(s.settings.mouse, crate::settings::MouseSettings::default());
    assert_eq!(
        s.settings.idle.lock,
        Some(250),
        "a valid idle table is kept"
    );
}

#[test]
fn wrong_typed_phase2_values_keep_state() {
    for extra in [
        "[idle]\ndim = \"soon\"\n",
        "[notif]\nbackend = 4\n",
        "[privacy]\nrecord_launches = \"no\"\n",
        "weather_location = 3\n",
    ] {
        let (s, _d) = load_with(extra);
        assert_kept(&s, extra);
    }
}

#[test]
fn bad_pin_entries_are_dropped_not_all_pins() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        "pinned = [\"firefox.desktop\", \"bad id\", 4, \"foot.desktop\", \"firefox.desktop\"]\n",
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.settings.pinned, ["firefox.desktop", "foot.desktop"]);
    assert!(p.exists());
}
