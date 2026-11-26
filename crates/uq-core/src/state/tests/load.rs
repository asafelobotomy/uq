//! Loading: defaults, upgrades and damaged files.

use super::*;

#[test]
fn default_is_graphite_dark() {
    let s = State::default();
    assert_eq!((s.theme.as_str(), s.variant), ("graphite", Variant::Dark));
    assert_eq!(s.schedule, Schedule::Off);
    assert!(s.layouts.is_empty());
}

#[test]
fn missing_state_is_default() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(State::load(&dir.path().join("nope.toml")), State::default());
}

#[test]
fn corrupt_state_is_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, "{{nope").unwrap();
    assert_eq!(State::load(&p), State::default());
    assert!(!p.exists());
    let bad: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|f| {
            f.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("state.toml.bad-")
        })
        .collect();
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert_eq!(std::fs::read(&bad[0]).unwrap(), b"{{nope");
}

#[test]
fn phase1_state_upgrades() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        include_str!("../../../tests/fixtures/phase1-state.toml"),
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.settings, Settings::default());
    assert!(s.launch_counts.is_empty());
    assert_eq!(s.theme, "dracula");
    assert_eq!(s.variant, Variant::Light);
    assert_eq!(s.layouts.get("2"), Some(&Layout::Scrolling));
    assert_eq!(s.layouts.get("special:magic"), Some(&Layout::Master));
    assert!(p.exists(), "a valid Phase 1 file is not renamed");
}

#[test]
fn out_of_order_idle_on_load_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        "launch_counts = { a = 3 }\n[idle]\ndim = 500\nlock = 300\n",
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.settings, Settings::default());
    assert_eq!(s.launch_counts.get("a"), Some(&3), "other state is kept");
    assert!(p.exists(), "only unparsable files are renamed");
}

#[test]
fn state_without_welcome_loads_as_pending() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        include_str!("../../../tests/fixtures/phase1-state.toml"),
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.welcome, Welcome::default());
    assert!(s.welcome.pending());
    assert_eq!(s.theme, "dracula", "the rest of the file loads");
    // A wrong-typed [welcome] costs only itself.
    std::fs::write(&p, "theme = \"rosewood\"\nwelcome = \"done\"\n").unwrap();
    let s = State::load(&p);
    assert_eq!(
        (s.theme.as_str(), s.welcome),
        ("rosewood", Welcome::default())
    );
    assert!(p.exists(), "not moved aside");
}

#[test]
fn old_state_loads_with_default_sections() {
    let st = State::parse("theme = \"graphite\"\nvariant = \"dark\"\n").unwrap();
    assert_eq!(
        st.settings.appearance,
        crate::settings::Appearance::default()
    );
    assert_eq!(st.settings.wallpaper, crate::settings::Wallpaper::default());
}

#[test]
fn unknown_section_keys_and_fit_keep_state() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
         [appearance]\ncursor_size = 32\nfuture_key = 1\n\
         [wallpaper.outputs.\"*\"]\nfit = \"stretch\"\nshiny = true\n\
         [wallpaper]\nlater = \"x\"\n",
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.pinned, ["firefox.desktop"]);
    assert_eq!(s.settings.appearance.cursor_size, Some(32));
    assert_eq!(
        s.settings.wallpaper.outputs["*"].fit,
        crate::settings::Fit::Fill,
        "an unknown fit loads as fill"
    );
    assert!(p.exists(), "a file with unknown keys is not moved aside");
}

#[test]
fn old_state_keeps_workspace_rules() {
    let st = State::parse("theme = \"graphite\"\n[layouts]\n\"2\" = \"scrolling\"\n").unwrap();
    let v = st.section("layouts").unwrap();
    assert_eq!(v["workspaces"]["2"], "scrolling");
    assert_eq!(st.settings.look, crate::settings::Look::default());
    assert_eq!(st.settings.layout_options, LayoutOptions::default());
}

#[test]
fn unknown_look_and_layout_values_keep_state() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
         [look]\ngaps_in = 4\nfuture_key = 1\n\
         [layout_options]\ndefault = \"monocle\"\nnew_key = 2\n\
         [layout_options.dwindle]\nforce_split = \"sideways\"\npreserve_split = true\n\
         [layout_options.master]\norientation = \"diagonal\"\nmfact = 0.6\n\
         [layouts]\n\"2\" = \"scrolling\"\n",
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.pinned, ["firefox.desktop"]);
    assert_eq!(s.settings.look.gaps_in, Some(4));
    let lo = &s.settings.layout_options;
    assert_eq!(
        (lo.default, lo.dwindle.force_split, lo.master.orientation),
        (None, None, None)
    );
    assert_eq!(
        (lo.dwindle.preserve_split, lo.master.mfact),
        (Some(true), Some(0.6))
    );
    assert_eq!(s.layouts.get("2"), Some(&Layout::Scrolling));
    assert!(
        p.exists(),
        "unknown keys or values do not move the file aside"
    );
}

#[test]
fn bad_shell_keeps_state() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    for shell in [
        "shell = 5
",
        "[shell]
islands = 3
providers = [1, \"x\"]
clock_24h = \"no\"\nterminal = 7
",
        "[shell]
providers = [{id = \":1.5\"}, {id = \"calc\"}, {id = \"calc\"}]
terminal = \"kitty\"
[shell.islands]\nwork = \"x\"\nfuture = 1
",
    ] {
        std::fs::write(
            &p,
            format!("theme = \"rosewood\"\npinned = [\"a.desktop\"]\n{shell}"),
        )
        .unwrap();
        let s = State::load(&p);
        assert_eq!(s.theme, "rosewood", "{shell}");
        assert_eq!(s.settings.pinned, ["a.desktop"], "{shell}");
        assert!(p.exists());
        assert!(s.settings.shell.islands.work && s.settings.shell.clock_24h);
        assert_eq!(s.settings.shell.terminal, None);
        assert!(s.settings.shell.providers.len() <= 1);
    }
}

#[test]
fn integration_rules_load_on_their_own() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(
        &p,
        "theme = \"rosewood\"\n[integration_rules]\nenabled = [\"romforge.emulators\", 4, \"bad\"]\n",
    )
    .unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.integration_rules.enabled, ["romforge.emulators"]);
    std::fs::write(&p, "theme = \"rosewood\"\nintegration_rules = 5\n").unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert!(s.integration_rules.enabled.is_empty());
    assert!(p.exists(), "not moved aside");
    // Empty, it isn't written at all.
    State::default().save(&p).unwrap();
    assert!(
        !std::fs::read_to_string(&p)
            .unwrap()
            .contains("integration_rules")
    );
    let mut st = State::default();
    st.set_section(
        "integration_rules",
        serde_json::json!({"enabled": ["romforge.emulators"]}),
    )
    .unwrap();
    assert_eq!(
        st.section("integration_rules").unwrap(),
        serde_json::json!({"enabled": ["romforge.emulators"]})
    );
    assert!(
        st.set_section("integration_rules", serde_json::json!({"enabled": ["x"]}))
            .is_err()
    );
}
