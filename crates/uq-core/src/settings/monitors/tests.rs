use super::*;
use crate::settings::renders_section;
use crate::state::State;
use serde_json::json;

fn section(v: Value) -> Result<Monitors, SettingError> {
    Monitors::from_section(&v)
}

fn one(entry: Value) -> Result<Monitors, SettingError> {
    section(json!({"outputs": {"DP-1": entry}}))
}

#[test]
fn modes_and_ranges() {
    for ok in [
        json!({"mode": "2560x1440@143.912"}),
        json!({"mode": "preferred"}),
        json!({"mode": "highres"}),
        json!({"mode": "1920x1080"}),
        json!({"scale": 1.333333}),
        json!({"scale": 0.25, "transform": 7, "vrr": 3, "position": {"x": -32768, "y": 32767}}),
    ] {
        assert!(one(ok.clone()).is_ok(), "{ok}");
    }
    for bad in [
        json!({"mode": "2560x1440@"}),
        json!({"mode": "big"}),
        json!({"mode": "1x1"}),
        json!({"mode": "2560x1440@1234"}),
        json!({"mode": "2560x1440@60.1234"}),
        json!({"scale": 0.2}),
        json!({"scale": 4.5}),
        json!({"scale": "huge"}),
        json!({"transform": 8}),
        json!({"vrr": 4}),
        json!({"enabled": 1}),
        json!({"position": {"x": 32768, "y": 0}}),
        json!({"position": {"x": 1.5, "y": 0}}),
        json!({"mirror_of": "DP 2"}),
    ] {
        assert!(one(bad.clone()).is_err(), "{bad}");
    }
    assert!(section(json!({"outputs": {"DP 1": {}}})).is_err());
    let many: Map<String, Value> = (0..17).map(|i| (format!("DP-{i}"), json!({}))).collect();
    assert!(section(json!({"outputs": many})).is_err());
    let sixteen: Map<String, Value> = (0..16).map(|i| (format!("DP-{i}"), json!({}))).collect();
    assert!(section(json!({"outputs": sixteen})).is_ok());
    // NaN can't be written in JSON; the struct rule still refuses it.
    let nan = OutputConfig {
        scale: Some(f64::NAN),
        ..OutputConfig::default()
    };
    assert!(nan.validate("k").is_err());
    let m = one(json!({"scale": 1.23456789})).unwrap();
    assert_eq!(m.outputs["DP-1"].scale, Some(1.234568));
}

#[test]
fn mirror_rules() {
    let ok = |v: Value| section(json!({"outputs": v}));
    assert!(ok(json!({"A": {"mirror_of": "A"}})).is_err());
    assert!(ok(json!({"A": {"mirror_of": "B"}, "B": {"mirror_of": "C"}, "C": {}})).is_err());
    assert!(ok(json!({"A": {"mirror_of": "B"}, "B": {"mirror_of": "A"}})).is_err());
    assert!(ok(json!({"A": {"mirror_of": "Z"}})).is_ok());
    assert!(ok(json!({"A": {"mirror_of": "B"}, "B": {}})).is_ok());
    // On load the broken link is cleared, the rest kept.
    let m: Monitors = serde_json::from_value(
        json!({"outputs": {"A": {"mirror_of": "A"}, "B": {"mirror_of": "C"}, "C": {"mirror_of": "D"}, "D": {"scale": 2.0}}}),
    )
    .unwrap();
    let (m, errs) = m.sanitised();
    assert_eq!(errs.len(), 2);
    assert_eq!(m.outputs["A"].mirror_of, None);
    assert_eq!(m.outputs["B"].mirror_of, None);
    assert_eq!(m.outputs["C"].mirror_of.as_deref(), Some("D"));
    assert_eq!(m.outputs["D"].scale, Some(2.0));
}

#[test]
fn unknown_fields_refused() {
    assert!(section(json!({"outputs": {"DP-1": {"rotation": 1}}})).is_err());
    assert!(section(json!({"primary": "DP-1"})).is_err());
    assert!(section(json!({"outputs": 5})).is_err());
    assert!(section(json!({"outputs": {"DP-1": 5}})).is_err());
    assert_eq!(section(json!({})).unwrap(), Monitors::default());
}

#[test]
fn bad_output_entry_costs_only_itself() {
    let toml = "theme = \"rosewood\"\npinned = [\"firefox.desktop\"]\n\
        [monitors]\nprimary = 3\n\
        [[monitors.profiles]]\nid = \"bogus\"\nname = \"Bad\"\n\
        [monitors.outputs.DP-1]\nscale = \"huge\"\n\
        [monitors.outputs.DP-2]\nmode = \"preferred\"\nfuture = 1\n\
        [monitors.outputs.DP-3]\nscale = 9.0\n\
        [monitors.outputs.DP-4]\nmirror_of = \"DP-4\"\n\
        [mouse]\nsensitivity = 0.4\n";
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    std::fs::write(&p, toml).unwrap();
    let s = State::load(&p);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "kept in place"
    );
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.pinned, ["firefox.desktop"]);
    assert_eq!(s.settings.mouse.sensitivity, 0.4);
    let outs = &s.settings.monitors.outputs;
    assert!(!outs.contains_key("DP-1"), "wrong type dropped");
    assert!(!outs.contains_key("DP-3"), "out of range dropped");
    assert_eq!(outs["DP-2"].mode.as_deref(), Some("preferred"));
    assert_eq!(outs["DP-4"].mirror_of, None, "self-mirror cleared");
    assert_eq!(s.settings.monitors.primary, None, "bad primary dropped");
    assert!(
        s.settings.monitors.profiles.is_empty(),
        "bad arrangement dropped"
    );
    // `monitors = 5` costs only the section.
    std::fs::write(&p, "theme = \"rosewood\"\nmonitors = 5\n").unwrap();
    let s = State::load(&p);
    assert_eq!(s.theme, "rosewood");
    assert_eq!(s.settings.monitors, Monitors::default());
}

#[test]
fn stored_monitors_round_trip_through_toml() {
    let mut s = State::default();
    s.settings.monitors = section(json!({"outputs": {
        "DP-1": {"mode": "2560x1440@143.912", "position": {"x": -1920, "y": 0}, "scale": 1.5,
                 "transform": 1, "enabled": true, "vrr": 2, "description": "Dell U2723QE"},
        "DP-2": {"mirror_of": "DP-1"}}}))
    .unwrap();
    let back: State = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.settings.monitors, s.settings.monitors);
    assert!(State::default().section("monitors").is_ok());
    assert!(!renders_section("monitors"));
}

#[test]
fn description_is_cleaned() {
    let m = one(json!({"description": format!("a\nb\"c{}", "x".repeat(300))})).unwrap();
    let d = m.outputs["DP-1"].description.clone().unwrap();
    assert_eq!(d.chars().count(), 200);
    assert!(d.starts_with("ab\"c") && !d.contains('\n'));
}

#[test]
fn live_output_strings_are_capped() {
    let long = format!("{}\n\"{}", "a".repeat(250), "b".repeat(250));
    let json = serde_json::to_string(&json!([{
        "name": "DP-1", "description": long, "mirrorOf": "none", "make": "M\u{7}",
        "width": 100, "height": 50, "refreshRate": 60.0, "vrr": true,
        "availableModes": (0..100).map(|i| format!("{i}x{i}@60.00Hz")).collect::<Vec<_>>()
    }, {"name": "DP-2", "mirrorOf": "DP-1"}, 7, {"description": "no name"}]))
    .unwrap();
    let live = parse_live(&json);
    assert_eq!(live.len(), 2);
    assert_eq!(live[0].description.chars().count(), 200);
    assert!(live[0].description.contains('"') || live[0].description.starts_with("aaa"));
    assert!(!live[0].description.chars().any(char::is_control));
    assert_eq!(live[0].make, "M");
    assert_eq!(live[0].mirror_of, None);
    assert!(live[0].vrr);
    assert_eq!(live[0].available_modes.len(), 64);
    assert_eq!(live[1].mirror_of.as_deref(), Some("DP-1"));
    assert!(parse_live("not json").is_empty());
    assert!(parse_live("{}").is_empty());
}

#[test]
fn live_fixture_parses() {
    let live = parse_live(include_str!("../../../tests/fixtures/monitors-all.json"));
    let names: Vec<_> = live.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["eDP-1", "DP-1"]);
    assert_eq!(
        (live[1].width, live[1].height, live[1].x),
        (3840, 2160, 1600)
    );
    assert_eq!(live[1].scale, 1.5);
    assert!(live[1].vrr && !live[0].vrr);
    assert_eq!(live[0].available_modes[0], "2560x1600@165.00Hz");
    assert_eq!(live[0].mirror_of, None);
}

#[test]
fn stays_visible_rules() {
    let live = parse_live(include_str!("../../../tests/fixtures/monitors-all.json"));
    let with = |v: Value| section(json!({"outputs": v})).unwrap();
    assert!(stays_visible(&Monitors::default(), &live));
    assert!(!stays_visible(&Monitors::default(), &[]));
    let off = with(json!({"eDP-1": {"enabled": false}, "DP-1": {"enabled": false}}));
    assert!(!stays_visible(&off, &live));
    let mut both = Monitors::default();
    for n in ["eDP-1", "DP-1"] {
        both.outputs.insert(
            n.into(),
            OutputConfig {
                mirror_of: Some("DP-9".into()),
                ..OutputConfig::default()
            },
        );
    }
    assert!(!stays_visible(&both, &live), "all mirroring");
    let mirror_one = with(json!({"DP-1": {"mirror_of": "eDP-1"}}));
    assert!(stays_visible(&mirror_one, &live), "eDP-1 stays on");
    let one_on = with(json!({"eDP-1": {"enabled": false}}));
    assert!(stays_visible(&one_on, &live), "DP-1 has no entry");
}
