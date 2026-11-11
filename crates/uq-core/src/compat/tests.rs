use super::browsers::{BROWSERS, FLAGS_LINE, FLAGS_MARKER, browser_for, flags_text};
use super::hypr::{self, SWITCH_ENV, SwitchKey};
use super::launch::{argv, fmt_scale, gdk_scale, main_scale, override_text, switch_env};
use super::*;
use crate::settings::{LiveOutput, SettingError};
use crate::state::State;
use serde_json::json;

mod launch;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn one_fix(app: serde_json::Value) -> AppFix {
    Compat::from_section(&json!({ "apps": [app] }))
        .expect("a valid fix")
        .apps
        .remove(0)
}

fn reason(e: SettingError) -> (String, String) {
    match e {
        SettingError::Invalid { key, reason } => (key, reason),
        other => panic!("not Invalid: {other:?}"),
    }
}

fn refused(v: serde_json::Value) -> (String, String) {
    reason(Compat::from_section(&v).expect_err("should be refused"))
}

#[test]
fn switches_and_apps_round_trip() {
    let section = json!({
        "sharp_x11": true, "electron_wayland": false, "chromium_wayland": true,
        "qt_wayland": false, "steam_scale": true,
        "apps": [
            {"id": "code.desktop", "platform": "wayland", "scale": null,
             "env": [{"name": "FOO", "value": "1"}], "args": ["--x"], "elsewhere": true},
            {"id": "steam.desktop", "platform": "auto", "scale": 1.25,
             "env": [], "args": [], "elsewhere": false},
        ]
    });
    let c = Compat::from_section(&section).unwrap();
    assert!(c.sharp_x11 && c.chromium_wayland && c.steam_scale);
    assert!(!c.electron_wayland && !c.qt_wayland);
    assert_eq!(c.apps[0].platform, Platform::Wayland);
    assert_eq!(c.apps[1].scale, Some(1.25));
    assert_eq!(c.section(), section);
    assert_eq!(Compat::from_section(&c.section()).unwrap(), c);

    let mut st = State::default();
    st.set_section("compat", section.clone()).unwrap();
    assert_eq!(st.section("compat").unwrap(), section);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.toml");
    st.save(&path).unwrap();
    let loaded = State::load(&path);
    assert_eq!(loaded.compat, c);
    assert_eq!(loaded, st);

    // Missing fields take their defaults; the default section is all off.
    assert_eq!(Compat::from_section(&json!({})).unwrap(), Compat::default());
    for key in SwitchKey::ALL {
        let mut c = Compat::default();
        assert!(!c.switch(key));
        c.set_switch(key, true);
        assert!(c.switch(key));
        assert_eq!(c.section()[key.key()], json!(true));
        assert_eq!(SwitchKey::from_key(key.key()), Some(key));
    }
    assert_eq!(SwitchKey::from_key("nope"), None);
}

#[test]
fn lenient_load_keeps_good_entries() {
    let src = r#"
[compat]
sharp_x11 = true
qt_wayland = "yes"

[[compat.apps]]
id = "code"
platform = "x11"

[[compat.apps]]
id = "../x"
platform = "x11"

[[compat.apps]]
id = "empty.desktop"

[[compat.apps]]
id = "code.desktop"
platform = "wayland"

[[compat.apps]]
id = "steam.desktop"
scale = 1.5
"#;
    let st = State::parse_strict(src).unwrap();
    assert!(st.compat.sharp_x11 && !st.compat.qt_wayland);
    let ids: Vec<_> = st.compat.apps.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["code.desktop", "steam.desktop"]);
    assert_eq!(st.compat.apps[0].platform, Platform::X11);
    let st = State::parse_strict("[compat]\napps = 3\n").unwrap();
    assert_eq!(st.compat, Compat::default());
}

#[test]
fn app_ids_normalised_and_checked() {
    assert_eq!(
        one_fix(json!({"id": "code", "platform": "x11"})).id,
        "code.desktop"
    );
    for bad in ["../x", "a/b.desktop", ".hidden.desktop", "", "..desktop"] {
        let (key, _) = refused(json!({"apps": [{"id": bad, "platform": "x11"}]}));
        assert_eq!(key, "compat.apps[0].id", "{bad:?}");
    }
    let (key, why) = refused(json!({"apps": [
        {"id": "code", "platform": "x11"},
        {"id": "code.desktop", "platform": "wayland"},
    ]}));
    assert_eq!(key, "compat.apps[1].id");
    assert!(why.contains("already"), "{why}");

    let c = Compat::from_section(&json!({"apps": [{"id": "code", "platform": "x11"}]})).unwrap();
    assert!(c.fix_for("code").is_some());
    assert!(c.fix_for("code.desktop").is_some());
    assert!(c.fix_for("other.desktop").is_none());

    let many: Vec<_> = (0..=MAX_APPS)
        .map(|i| json!({"id": format!("a{i}.desktop"), "platform": "x11"}))
        .collect();
    let (_, why) = refused(json!({ "apps": many }));
    assert!(why.contains("100"), "{why}");
}

#[test]
fn app_env_uses_env_rules() {
    let (key, why) = refused(json!({"apps": [{"id": "code.desktop",
        "env": [{"name": "XDG_SESSION_ID", "value": "1"}]}]}));
    assert_eq!(key, "compat.apps[0].env[0]");
    assert_eq!(
        why,
        crate::env::blocked("XDG_SESSION_ID").unwrap(),
        "the env domain's own reason"
    );
    for (name, value) in [("lower", "1"), ("A=B", "1"), ("A", "$HOME"), ("A", "a\nb")] {
        let (key, _) = refused(json!({"apps": [{"id": "code.desktop",
            "env": [{"name": name, "value": value}]}]}));
        assert_eq!(key, "compat.apps[0].env[0]", "{name}={value:?}");
    }
    // A warned name is allowed, as in the env domain.
    one_fix(json!({"id": "code.desktop", "env": [{"name": "LD_PRELOAD", "value": "/x.so"}]}));
    let too_many: Vec<_> = (0..=MAX_APP_ENV)
        .map(|i| json!({"name": format!("V{i}"), "value": ""}))
        .collect();
    let (key, _) = refused(json!({"apps": [{"id": "code.desktop", "env": too_many}]}));
    assert_eq!(key, "compat.apps[0].env");
    let (key, _) = refused(json!({"apps": [{"id": "code.desktop",
        "env": [{"name": "A", "value": "1"}, {"name": "A", "value": "2"}]}]}));
    assert_eq!(key, "compat.apps[0].env[1]");
}

#[test]
fn args_checked() {
    let long = "x".repeat(MAX_ARG_CHARS + 1);
    for bad in [json!([""]), json!(["a\nb"]), json!([long]), json!([1])] {
        let (key, _) = refused(json!({"apps": [{"id": "code.desktop", "args": bad}]}));
        assert!(key.starts_with("compat.apps[0].args"), "{key}");
    }
    let many: Vec<_> = (0..=MAX_ARGS).map(|i| format!("--a{i}")).collect();
    let (key, _) = refused(json!({"apps": [{"id": "code.desktop", "args": many}]}));
    assert_eq!(key, "compat.apps[0].args");
    let quotes = one_fix(json!({"id": "code.desktop", "args": ["--x=\"a b\"; 'c' %F"]}));
    assert_eq!(quotes.args, ["--x=\"a b\"; 'c' %F"]);
    one_fix(json!({"id": "code.desktop", "args": ["é".repeat(MAX_ARG_CHARS)]}));
}

#[test]
fn empty_fix_refused() {
    for empty in [
        json!({"id": "code.desktop"}),
        json!({"id": "code.desktop", "platform": "auto", "scale": null, "env": [], "args": []}),
        json!({"id": "code.desktop", "elsewhere": true}),
    ] {
        let (key, why) = refused(json!({ "apps": [empty] }));
        assert_eq!(key, "compat.apps[0]");
        assert_eq!(why, "This fix doesn't change anything.");
    }
    assert!(AppFix::new("code.desktop").is_empty());
    assert!(!one_fix(json!({"id": "code.desktop", "args": ["--x"]})).is_empty());
}

#[test]
fn scale_steps_checked() {
    for ok in [1.0, 1.25, 1.5, 2.75, 3.0] {
        assert!(valid_scale(ok), "{ok}");
    }
    for bad in [1.1, 0.75, 3.25, 0.0, f64::NAN, f64::INFINITY, 1.3] {
        assert!(!valid_scale(bad), "{bad}");
    }
    let (key, _) = refused(json!({"apps": [{"id": "code.desktop", "scale": 1.1}]}));
    assert_eq!(key, "compat.apps[0].scale");
    assert_eq!(
        one_fix(json!({"id": "code.desktop", "scale": 1.25})).scale,
        Some(1.25)
    );
    assert_eq!(
        one_fix(json!({"id": "code.desktop", "scale": 3.0})).scale,
        Some(3.0)
    );
    let (key, _) = refused(json!({"apps": [{"id": "code.desktop", "platform": "mir"}]}));
    assert_eq!(key, "compat.apps[0].platform");
}

#[test]
fn gdk_scale_rounds_half_down() {
    for (s, g) in [
        (1.0, 1),
        (1.25, 1),
        (1.5, 1),
        (1.75, 2),
        (2.0, 2),
        (2.5, 2),
        (3.0, 3),
        (0.5, 1),
    ] {
        assert_eq!(gdk_scale(s), g, "{s}");
    }
    assert_eq!(fmt_scale(1.25), "1.25");
    assert_eq!(fmt_scale(2.0), "2");
    assert_eq!(fmt_scale(1.5), "1.5");
    assert_eq!(fmt_scale(1.333_333), "1.33");
}

fn out(name: &str, x: i32, y: i32, scale: f64, disabled: bool) -> LiveOutput {
    LiveOutput {
        name: name.into(),
        x,
        y,
        scale,
        disabled,
        ..LiveOutput::default()
    }
}

#[test]
fn main_scale_picks_origin_output() {
    let both = [
        out("DP-1", 1920, 0, 1.0, false),
        out("eDP-1", 0, 0, 1.5, false),
    ];
    assert_eq!(main_scale(&both), Some(1.5));
    let none_at_origin = [
        out("HDMI-A-1", 2000, 0, 2.0, false),
        out("DP-2", 100, 0, 1.25, false),
    ];
    assert_eq!(main_scale(&none_at_origin), Some(1.25));
    let disabled = [
        out("eDP-1", 0, 0, 2.0, true),
        out("DP-1", 1920, 0, 1.75, false),
    ];
    assert_eq!(main_scale(&disabled), Some(1.75));
    // Several at 0,0 (mirrored or overlapping): the first by name, whatever the list order.
    let tied = [
        out("HDMI-A-1", 0, 0, 2.0, false),
        out("eDP-1", 0, 0, 1.25, false),
        out("DP-1", 1920, 0, 1.0, false),
    ];
    assert_eq!(main_scale(&tied), Some(2.0));
    let mut swapped = tied.clone();
    swapped.swap(0, 1);
    assert_eq!(main_scale(&swapped), Some(2.0));
    assert_eq!(main_scale(&[]), None);
    assert_eq!(main_scale(&[out("eDP-1", 0, 0, 2.0, true)]), None);
}

#[test]
fn switch_env_follows_table() {
    let line = |n: &str, v: &str| (n.to_string(), v.to_string());
    let cases = [
        (SwitchKey::SharpX11, Some(line("GDK_SCALE", "1"))),
        (
            SwitchKey::ElectronWayland,
            Some(line("ELECTRON_OZONE_PLATFORM_HINT", "auto")),
        ),
        (SwitchKey::ChromiumWayland, None),
        (
            SwitchKey::QtWayland,
            Some(line("QT_QPA_PLATFORM", "wayland;xcb")),
        ),
        (
            SwitchKey::SteamScale,
            Some(line("STEAM_FORCE_DESKTOPUI_SCALING", "1.5")),
        ),
    ];
    for (key, want) in cases {
        let mut c = Compat::default();
        c.set_switch(key, true);
        assert_eq!(
            switch_env(&c, Some(1.5)),
            want.into_iter().collect::<Vec<_>>(),
            "{key:?}"
        );
    }
    let mut all = Compat::default();
    for key in SwitchKey::ALL {
        all.set_switch(key, true);
    }
    let names = |v: Vec<(String, String)>| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(
        names(switch_env(&all, None)),
        ["ELECTRON_OZONE_PLATFORM_HINT", "QT_QPA_PLATFORM"]
    );
    assert_eq!(switch_env(&all, Some(2.0)).len(), 4);
    assert!(switch_env(&Compat::default(), Some(2.0)).is_empty());
    for (_, name, value) in SWITCH_ENV {
        assert_eq!(
            value.scale_derived(),
            ["GDK_SCALE", "STEAM_FORCE_DESKTOPUI_SCALING"].contains(&name),
            "{name}"
        );
    }
}

#[test]
fn browsers_table_unique() {
    let mut ids = std::collections::HashSet::new();
    let mut files = std::collections::HashSet::new();
    for b in &BROWSERS {
        assert!(!b.name.is_empty() && !b.desktop_ids.is_empty());
        assert!(files.insert(b.flags_file), "{}", b.flags_file);
        assert!(!b.flags_file.contains('/'));
        for id in b.desktop_ids {
            assert!(ids.insert(*id), "{id}");
            assert!(crate::desktop_entry::valid_desktop_id(id) && id.ends_with(".desktop"));
            assert_eq!(browser_for(id).unwrap().name, b.name);
        }
    }
    assert_eq!(
        browser_for("chromium").unwrap().flags_file,
        "chromium-flags.conf"
    );
    assert!(browser_for("firefox.desktop").is_none());
    assert_eq!(flags_text(), format!("{FLAGS_MARKER}\n{FLAGS_LINE}\n"));
    assert!(FLAGS_MARKER.starts_with('#'));
}

#[test]
fn labels_are_plain() {
    for key in SwitchKey::ALL {
        for text in [key.label(), key.description()] {
            let lower = text.to_lowercase();
            for bad in ["xwayland", "env", "ozone", "hl.", "_"] {
                assert!(!lower.contains(bad), "{text:?} holds {bad:?}");
            }
            assert!(!text.contains("Qt_"), "{text}");
        }
        assert!(
            key.description()
                .ends_with("Applies to apps started after the change."),
            "{}",
            key.description()
        );
    }
    assert_eq!(SwitchKey::SharpX11.label(), "Sharp X11 apps");
    assert_eq!(hypr::XWAYLAND_ZERO_SCALING, "xwayland:force_zero_scaling");
}

#[test]
fn refusals_are_plain() {
    let long = "x".repeat(MAX_ARG_CHARS + 1);
    let bad = [
        json!([]),
        json!({"sharp_x11": 1}),
        json!({"other": true}),
        json!({"apps": 3}),
        json!({"apps": [1]}),
        json!({"apps": [{"platform": "x11"}]}),
        json!({"apps": [{"id": "../x", "platform": "x11"}]}),
        json!({"apps": [{"id": "a", "platform": "mir"}]}),
        json!({"apps": [{"id": "a", "scale": 1.1}]}),
        json!({"apps": [{"id": "a", "scale": "big"}]}),
        json!({"apps": [{"id": "a", "env": [{"name": "DISPLAY", "value": "x"}]}]}),
        json!({"apps": [{"id": "a", "env": [{"name": "A"}]}]}),
        json!({"apps": [{"id": "a", "args": [long]}]}),
        json!({"apps": [{"id": "a", "args": ["a\tb"]}]}),
        json!({"apps": [{"id": "a"}]}),
        json!({"apps": [{"id": "a", "args": ["x"], "more": 1}]}),
        json!({"apps": [{"id": "a", "args": ["x"], "elsewhere": "yes"}]}),
    ];
    let mut msgs: Vec<String> = bad.into_iter().map(|v| refused(v).1).collect();
    let fix = one_fix(json!({"id": "code.desktop", "platform": "wayland"}));
    msgs.push(
        argv(Some(&fix), "code", s(&["A=B"]))
            .unwrap_err()
            .to_string(),
    );
    msgs.push(argv(Some(&fix), "code", vec![]).unwrap_err().to_string());
    msgs.push(
        override_text("[Desktop Entry]\nExec=sh -c 'x'\n", "/s", &fix, "code")
            .unwrap_err()
            .to_string(),
    );
    for m in msgs {
        assert!(!m.is_empty());
        for bad in [
            "regex",
            "[A-Z",
            "state.toml",
            "^",
            "Some(",
            "argv",
            "Exec",
            "lossy",
            "serde",
            "compat.",
            "invalid type",
        ] {
            assert!(!m.contains(bad), "{m:?} contains {bad:?}");
        }
    }
}
