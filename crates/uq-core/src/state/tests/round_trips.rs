//! Round trips of each section.

use super::*;

#[test]
fn disabled_idle_stage_and_settings_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let mut s = State::default();
    s.settings.idle.dim = None;
    s.settings.weather_location = Some(crate::settings::Location {
        name: "London".into(),
        lat: 51.51,
        lon: -0.13,
    });
    s.settings.pinned = vec!["firefox.desktop".into()];
    s.launch_counts.insert("firefox.desktop".into(), 7);
    s.save(&p).unwrap();
    assert_eq!(State::load(&p), s);
}

#[test]
fn state_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("uq/state.toml");
    let s = State {
        theme: "rosewood".into(),
        variant: Variant::Light,
        schedule: Schedule::Sun {
            lat: 51.5,
            lon: -0.13,
        },
        layouts: BTreeMap::from([
            ("2".into(), Layout::Scrolling),
            ("special:magic".into(), Layout::Master),
        ]),
        rules: Rules::default(),
        keybinds: Keybinds::default(),
        env: Env {
            vars: vec![crate::env::Var {
                name: "GDK_BACKEND".into(),
                value: "x11".into(),
            }],
        },
        compat: Compat {
            sharp_x11: true,
            apps: vec![crate::compat::AppFix {
                scale: Some(1.5),
                ..crate::compat::AppFix::new("code.desktop")
            }],
            ..Compat::default()
        },
        autostart: Autostart {
            files: vec![crate::autostart::FileChoice {
                id: "nm-applet.desktop".into(),
                enabled: Some(false),
                delay: None,
                minimize: None,
            }],
            added: vec![crate::autostart::Added::Command {
                id: "a-0b12".into(),
                name: "Backup check".into(),
                argv: vec!["backup-check".into(), "--quiet".into()],
                enabled: true,
                delay: Some(10),
                minimize: false,
            }],
            ..Autostart::default()
        },
        integration_rules: IntegrationRules {
            enabled: vec!["romforge.emulators".into()],
        },
        schedule_mark: None,
        welcome: Welcome {
            done: true,
            version: 1,
        },
        settings: Settings::default(),
        launch_counts: BTreeMap::new(),
    };
    s.save(&p).unwrap();
    assert_eq!(State::load(&p), s);
}

#[test]
fn welcome_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let s = State {
        welcome: Welcome {
            done: true,
            version: crate::welcome::CURRENT_WELCOME_VERSION,
        },
        ..State::default()
    };
    s.save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(text.contains("[welcome]"), "{text}");
    assert_eq!(State::load(&p), s);
}

#[test]
fn null_overrides_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.toml");
    let mut st = State::default();
    st.settings
        .set_section(
            "appearance",
            serde_json::json!({"ui_font": null, "cursor_size": 32}),
        )
        .unwrap();
    st.settings
        .set_section(
            "wallpaper",
            serde_json::json!({"outputs": {"DP-1": {"image": null, "color": "#101010"}}}),
        )
        .unwrap();
    st.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("ui_font") && !text.contains("image"),
        "{text}"
    );
    assert_eq!(State::load(&path), st);
}

#[test]
fn wallpaper_state_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let mut st = State::default();
    st.settings
        .set_section(
            "wallpaper",
            serde_json::json!({"outputs": {
                "*": {"image": "/w/a.png", "fit": "center"},
                "DP-1": {"folder": "/w/slides", "interval": 60, "fit": "tile"},
            }}),
        )
        .unwrap();
    st.save(&p).unwrap();
    assert_eq!(State::load(&p), st);
}

#[test]
fn layouts_section_normalises_and_replaces_rules() {
    let mut st = State::default();
    st.layouts.insert("2".into(), Layout::Scrolling);
    st.set_section(
        "layouts",
        serde_json::json!({"default": "master", "workspaces": {"web": "master"},
            "master": {"mfact": 0.62}}),
    )
    .unwrap();
    assert_eq!(
        st.layouts,
        BTreeMap::from([("name:web".into(), Layout::Master)])
    );
    let v = st.section("layouts").unwrap();
    assert_eq!(v["workspaces"], serde_json::json!({"name:web": "master"}));
    assert_eq!(v["default"], "master");
    assert_eq!(v["master"]["mfact"], 0.62);
    assert_eq!(v["dwindle"], serde_json::json!({}));
    // A missing `workspaces` means none.
    st.set_section("layouts", serde_json::json!({})).unwrap();
    assert!(st.layouts.is_empty());
    assert_eq!(st.settings.layout_options, LayoutOptions::default());
}

#[test]
fn layouts_section_rejects_bad_input() {
    let mut st = State::default();
    st.set_section(
        "layouts",
        serde_json::json!({"workspaces": {"2": "dwindle"}}),
    )
    .unwrap();
    let before = st.clone();
    let many: serde_json::Map<_, _> = (1..=65)
        .map(|i| (i.to_string(), serde_json::json!("dwindle")))
        .collect();
    for bad in [
        serde_json::json!({"workspaces": many}),
        serde_json::json!({"workspaces": {"web": "monocle"}}),
        serde_json::json!({"workspaces": {"a\nb": "dwindle"}}),
        serde_json::json!({"workspaces": {"name:": "dwindle"}, "extra": 1}),
        serde_json::json!({"workspaces": {"a".repeat(65): "dwindle"}}),
        serde_json::json!({"workspaces": {"web": "master", "name:web": "dwindle"}}),
        serde_json::json!({"master": {"mfact": 0.95}}),
    ] {
        assert!(st.set_section("layouts", bad.clone()).is_err(), "{bad}");
        assert_eq!(st, before);
    }
    let mut many = serde_json::Map::new();
    for i in 1..=64 {
        many.insert(i.to_string(), serde_json::json!("dwindle"));
    }
    st.set_section("layouts", serde_json::json!({"workspaces": many}))
        .unwrap();
    assert_eq!(st.layouts.len(), 64);
    assert!(st.set_section("bogus", serde_json::json!({})).is_err());
}

#[test]
fn shell_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let mut st = State::default();
    st.set_section(
        "shell",
        serde_json::json!({"islands": {"time": false}, "clock_24h": false,
            "terminal": "foot.desktop",
            "providers": [{"id": "run"}, {"id": "org.x.Y", "enabled": false}]}),
    )
    .unwrap();
    st.save(&p).unwrap();
    assert_eq!(State::load(&p), st);
}

#[test]
fn look_and_layout_options_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let mut st = State::default();
    st.set_section(
        "look",
        serde_json::json!({"gaps_in": 3, "blur": false, "animation_speed": 1.5}),
    )
    .unwrap();
    st.set_section(
        "layouts",
        serde_json::json!({"default": "scrolling", "workspaces": {"2": "master"},
            "dwindle": {"force_split": "right"}, "master": {"new_status": "slave", "orientation": "top"},
            "scrolling": {"column_width": 0.7, "fullscreen_on_one_column": true}}),
    )
    .unwrap();
    st.save(&p).unwrap();
    assert_eq!(State::load(&p), st);
}
