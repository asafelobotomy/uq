use super::*;
use crate::settings::renders_section;
use crate::state::State;
use serde_json::json;

fn section(v: Value) -> Result<InputSettings, SettingError> {
    InputSettings::from_section(&v)
}

fn kb(v: Value) -> Result<InputSettings, SettingError> {
    section(json!({ "keyboard": v }))
}

#[test]
fn keyboard_rules() {
    let ok = kb(
        json!({"layouts": [{"layout": "us"}, {"layout": "de", "variant": "nodeadkeys"}],
        "switch_combo": "grp:alt_shift_toggle", "repeat_rate": 100, "repeat_delay": 2000}),
    )
    .unwrap();
    assert_eq!(ok.keyboard.layouts.len(), 2);
    assert_eq!(
        ok.keyboard.layouts[1].variant.as_deref(),
        Some("nodeadkeys")
    );
    assert!(kb(json!({"layouts": []})).is_ok());
    let five: Vec<Value> = ["us", "de", "fr", "es", "it"]
        .iter()
        .map(|l| json!({"layout": l}))
        .collect();
    for bad in [
        json!({"layouts": five}),
        json!({"layouts": [{"layout": "US"}]}),
        json!({"layouts": [{"layout": "u s"}]}),
        json!({"layouts": [{"layout": "u"}]}),
        json!({"layouts": [{"layout": "us", "variant": "a b"}]}),
        json!({"layouts": [{"layout": "us"}, {"layout": "us"}]}),
        json!({"layouts": [{"layout": 5}]}),
        json!({"switch_combo": "ctrl:nocaps"}),
        json!({"repeat_rate": 0}),
        json!({"repeat_rate": 101}),
        json!({"repeat_rate": 300}),
        json!({"repeat_delay": 50}),
        json!({"repeat_delay": 2001}),
    ] {
        assert!(kb(bad.clone()).is_err(), "{bad}");
    }
    // The same layout with different variants is fine.
    assert!(
        kb(json!({"layouts": [{"layout": "us"}, {"layout": "us", "variant": "dvorak"}]})).is_ok()
    );
}

#[test]
fn touchpad_and_device_ranges() {
    let t = section(json!({"touchpad": {"scroll_factor": 1.2345, "tap_to_click": true}})).unwrap();
    assert_eq!(t.touchpad.scroll_factor, Some(1.23));
    for bad in [
        json!({"touchpad": {"scroll_factor": 0.05}}),
        json!({"touchpad": {"scroll_factor": 11}}),
        json!({"touchpad": {"tap_to_click": 1}}),
        json!({"devices": {"m": {"sensitivity": 1.5}}}),
        json!({"devices": {"m": {"accel_profile": "fast"}}}),
        json!({"devices": {"m": {"left_handed": "yes"}}}),
        json!({"devices": {"m": 5}}),
    ] {
        assert!(section(bad.clone()).is_err(), "{bad}");
    }
    let d = section(json!({"devices": {"m": {"sensitivity": -0.456}, "empty": {}}})).unwrap();
    assert_eq!(d.devices["m"].sensitivity, Some(-0.46));
    assert!(!d.devices.contains_key("empty"), "all-None entry dropped");
}

#[test]
fn device_names() {
    let d = section(json!({"devices": {"Logitech G502": {"left_handed": true}}})).unwrap();
    assert!(d.devices.contains_key("Logitech-G502"));
    let entry = json!({"left_handed": true});
    assert!(section(json!({"devices": {"a\nb": entry}})).is_err());
    assert!(section(json!({"devices": {"": entry}})).is_err());
    assert!(section(json!({"devices": {"x".repeat(129): entry}})).is_err());
    assert!(section(json!({"devices": {"x".repeat(128): entry}})).is_ok());
    let many = |n: usize| -> Value {
        json!({"devices": (0..n).map(|i| (format!("d{i}"), entry.clone())).collect::<Map<_, _>>()})
    };
    assert!(section(many(32)).is_ok());
    assert!(section(many(33)).is_err());
    // "a b" and "a-b" are the same device to Hyprland.
    assert!(section(json!({"devices": {"a b": entry, "a-b": entry}})).is_err());
}

#[test]
fn section_carries_mouse() {
    let mut s = Settings::default();
    s.set_section("input", json!({"mouse": {"sensitivity": 0.3}}))
        .unwrap();
    assert_eq!(s.mouse.sensitivity, 0.3);
    let v = s.section("input").unwrap();
    assert_eq!(v["mouse"]["natural_scroll"], json!(false));
    assert_eq!(v["mouse"]["sensitivity"], json!(0.3));
    for part in ["keyboard", "touchpad", "devices"] {
        assert!(v.get(part).is_some(), "{part}");
    }
    // A missing mouse means the defaults, as with any replace.
    s.set_section("input", json!({})).unwrap();
    assert_eq!(s.mouse, crate::settings::MouseSettings::default());
    // A bad mouse value or part leaves everything as it was.
    s.set_section(
        "input",
        json!({"mouse": {"left_handed": true}, "touchpad": {"tap_to_click": true}}),
    )
    .unwrap();
    let before = s.clone();
    for bad in [
        json!({"mouse": {"sensitivity": 2}, "touchpad": {"tap_to_click": false}}),
        json!({"mouse": {"accel_profile": "fast"}}),
        json!({"mouse": 5}),
        json!({"mouse": {"left_handed": false}, "keyboard": {"repeat_rate": 0}}),
    ] {
        assert!(s.set_section("input", bad.clone()).is_err(), "{bad}");
        assert_eq!(s, before, "{bad}");
    }
    assert!(renders_section("input"));
}

#[test]
fn phase2_mouse_values_survive() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, "[mouse]\nsensitivity = -0.5\nnatural_scroll = true\n").unwrap();
    let s = State::load(&p);
    let v = s.section("input").unwrap();
    assert_eq!(v["mouse"]["sensitivity"], json!(-0.5));
    assert_eq!(v["mouse"]["natural_scroll"], json!(true));
    s.save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(table["mouse"]["sensitivity"].as_float(), Some(-0.5));
    assert_eq!(table["mouse"]["natural_scroll"].as_bool(), Some(true));
    assert!(
        table["input"]
            .as_table()
            .is_none_or(|t| !t.contains_key("mouse")),
        "{text}"
    );
    let again = State::load(&p);
    assert_eq!(again.settings.mouse, s.settings.mouse);
    // SetSetting still works on the same table.
    let mut s = again;
    s.settings.apply("mouse.sensitivity", json!(0.2)).unwrap();
    assert_eq!(
        s.section("input").unwrap()["mouse"]["sensitivity"],
        json!(0.2)
    );
}

#[test]
fn unknown_fields_refused() {
    for bad in [
        json!({"bogus": 1}),
        json!({"keyboard": {"bogus": 1}}),
        json!({"keyboard": {"layouts": [{"layout": "us", "bogus": 1}]}}),
        json!({"touchpad": {"bogus": 1}}),
        json!({"devices": {"m": {"bogus": 1}}}),
        json!({"gestures": [{"fingers": 3, "direction": "left", "action": "close", "x": 1}]}),
        json!({"gestures": "all"}),
        json!({"swipe": {"bogus": 1}}),
        json!({"swipe": {"distance": 2001}}),
        json!({"swipe": {"cancel_ratio": 1.5}}),
        json!({"swipe": {"distance": "far"}}),
    ] {
        let mut s = Settings::default();
        assert!(s.set_section("input", bad.clone()).is_err(), "{bad}");
        assert_eq!(s, Settings::default());
    }
    let mut s = Settings::default();
    assert!(
        s.set_section("input", json!({"mouse": {"bogus": 1}}))
            .is_err()
    );
}

#[test]
fn unknown_nested_keys_are_ignored_on_load() {
    let toml = "theme = \"rosewood\"\n\
        [input.keyboard]\nrepeat_rate = 30\nfuture = 1\n\
        [[input.keyboard.layouts]]\nlayout = \"de\"\nfuture = \"x\"\n\
        [input.touchpad]\ntap_to_click = true\nfuture = { a = 1 }\n\
        [input.devices.mouse1]\nleft_handed = true\nfuture = { a = 1 }\n\
        [mouse]\nsensitivity = 0.4\n";
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, toml).unwrap();
    let s = State::load(&p).settings;
    assert_eq!(s.input.keyboard.repeat_rate, Some(30));
    assert_eq!(s.input.keyboard.layouts.len(), 1);
    assert_eq!(s.input.keyboard.layouts[0].layout, "de");
    assert_eq!(s.input.touchpad.tap_to_click, Some(true));
    assert_eq!(s.input.devices["mouse1"].left_handed, Some(true));
    assert_eq!(s.mouse.sensitivity, 0.4);
}

#[test]
fn bad_stored_values_cost_only_themselves() {
    let toml = "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
        [input.keyboard]\nrepeat_rate = 0\nrepeat_delay = 400\nswitch_combo = \"ctrl:nocaps\"\n\
        [[input.keyboard.layouts]]\nlayout = \"US\"\n\
        [[input.keyboard.layouts]]\nlayout = \"fr\"\n\
        [[input.keyboard.layouts]]\nlayout = \"fr\"\n\
        [[input.keyboard.layouts]]\nlayout = 5\n\
        [input.touchpad]\nscroll_factor = 99.0\nnatural_scroll = true\n\
        [input.devices.ok]\nsensitivity = 0.5\n\
        [input.devices.bad]\nsensitivity = \"fast\"\n\
        [input.devices.range]\nsensitivity = 3.0\n\
        [mouse]\nsensitivity = 0.4\n";
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, toml).unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.pinned, ["firefox.desktop"]);
    assert_eq!(s.settings.mouse.sensitivity, 0.4);
    let i = &s.settings.input;
    assert_eq!(i.keyboard.repeat_rate, None);
    assert_eq!(i.keyboard.repeat_delay, Some(400));
    assert_eq!(i.keyboard.switch_combo, None);
    assert_eq!(i.keyboard.layouts.len(), 1, "only one fr survives");
    assert_eq!(i.touchpad.scroll_factor, None);
    assert_eq!(i.touchpad.natural_scroll, Some(true));
    assert_eq!(i.devices.keys().collect::<Vec<_>>(), ["ok"]);
    // `input = 5` costs only the section.
    std::fs::write(&p, "theme = \"rosewood\"\ninput = 5\n").unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.input, InputSettings::default());
}

#[test]
fn stored_input_round_trips_through_toml() {
    let mut s = State::default();
    s.settings
        .set_section(
            "input",
            json!({
                "keyboard": {"layouts": [{"layout": "us"}, {"layout": "de", "variant": "nodeadkeys"}],
                    "switch_combo": "grp:alt_shift_toggle", "repeat_rate": 40, "repeat_delay": 300},
                "mouse": {"sensitivity": 0.25},
                "touchpad": {"natural_scroll": true, "tap_to_click": false, "scroll_factor": 1.5},
                "devices": {"Logitech G502": {"sensitivity": -0.3, "accel_profile": "flat"}}
            }),
        )
        .unwrap();
    let back: State = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.settings.input, s.settings.input);
    assert_eq!(back.settings.mouse, s.settings.mouse);
    assert_eq!(back.settings.validated().0, back.settings);
    assert_eq!(back.section("input").unwrap(), s.section("input").unwrap());
}

#[test]
fn input_section_round_trips() {
    let mut s = State::default();
    s.settings
        .set_section(
            "input",
            json!({
                "keyboard": {"layouts": [{"layout": "us"}], "repeat_rate": 40},
                "mouse": {"sensitivity": 0.25},
                "touchpad": {"natural_scroll": true},
                "devices": {"Logitech G502": {"sensitivity": -0.3}},
                "gestures": [
                    {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
                    {"fingers": 4, "direction": "up", "action": "uq:spotlight", "enabled": false},
                    {"fingers": 3, "direction": "down", "action": "special", "workspace": "magic",
                     "modifier": "SUPER", "enabled": true},
                    {"fingers": 4, "direction": "pinch_in", "action": "uq:quick-settings",
                     "modifier": "SUPER SHIFT", "enabled": true}],
                "swipe": {"distance": 400, "invert": false, "create_new": false,
                          "cancel_ratio": 0.555}
            }),
        )
        .unwrap();
    assert_eq!(s.settings.input.swipe.cancel_ratio, Some(0.56));
    let gestures = s.settings.input.gestures.as_ref().unwrap();
    assert_eq!(gestures.len(), 4);
    let back: State = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.settings.input, s.settings.input);
    assert_eq!(back.settings.validated().0, back.settings);
    let json = s.section("input").unwrap();
    assert_eq!(back.section("input").unwrap(), json);
    // The section JSON is accepted back as written.
    let mut again = Settings::default();
    again.set_section("input", json.clone()).unwrap();
    assert_eq!(again.section("input").unwrap(), json);
    // Never set: neither key is written, and `null` is the same as absent.
    let plain = State::default().section("input").unwrap();
    assert!(
        plain.get("gestures").is_none() && plain.get("swipe").is_none(),
        "{plain}"
    );
    let mut s = Settings::default();
    s.set_section("input", json!({"gestures": null, "swipe": null}))
        .unwrap();
    assert_eq!(s.input.gestures, None);
    // An empty list is "no gestures", not the defaults.
    s.set_section("input", json!({"gestures": []})).unwrap();
    assert_eq!(s.input.gestures, Some(Vec::new()));
    let back: State = toml::from_str(
        &toml::to_string(&State {
            settings: s.clone(),
            ..State::default()
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(back.settings.input.gestures, Some(Vec::new()));
}

#[test]
fn clashing_gestures_are_refused_on_write() {
    let mut s = Settings::default();
    let err = s
        .set_section(
            "input",
            json!({"gestures": [
                {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
                {"fingers": 3, "direction": "left", "action": "close", "enabled": true}]}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("input.gestures[1].direction"), "{err}");
    assert!(err.contains("both use 3 fingers, swipe left"), "{err}");
    assert_eq!(s, Settings::default());
}

#[test]
fn lenient_load_drops_only_bad_gestures() {
    let toml = "theme = \"rosewood\"\n\
        [[input.gestures]]\nfingers = 3\ndirection = \"horizontal\"\naction = \"workspace\"\n\
        [[input.gestures]]\nfingers = 9\ndirection = \"left\"\naction = \"close\"\n\
        [[input.gestures]]\nfingers = 4\ndirection = \"diagonal\"\naction = \"close\"\n\
        [[input.gestures]]\nfingers = 3\ndirection = \"left\"\naction = \"close\"\n\
        [[input.gestures]]\nfingers = 4\ndirection = \"up\"\naction = \"uq:spotlight\"\nfuture = 1\n\
        [input.swipe]\ndistance = 9000\ninvert = false\n";
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, toml).unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    let list = s.settings.input.gestures.unwrap();
    let got: Vec<(u8, crate::gestures::Direction)> =
        list.iter().map(|g| (g.fingers, g.direction)).collect();
    // The bad finger count and the unknown direction go; the later of two clashing gestures
    // (left against the earlier sideways swipe) goes too.
    assert_eq!(
        got,
        [
            (3, crate::gestures::Direction::Horizontal),
            (4, crate::gestures::Direction::Up)
        ]
    );
    assert!(list.iter().all(|g| g.enabled), "enabled defaults to on");
    assert_eq!(
        s.settings.input.swipe.distance, None,
        "out of range cleared"
    );
    assert_eq!(s.settings.input.swipe.invert, Some(false));
    // A gesture list that isn't a list is the defaults.
    std::fs::write(&p, "[input]\ngestures = 5\n").unwrap();
    assert_eq!(State::load(&p).settings.input.gestures, None);
}
