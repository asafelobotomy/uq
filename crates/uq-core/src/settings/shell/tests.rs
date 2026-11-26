use super::super::Settings;
use super::*;
use serde_json::json;

fn pref(id: &str, enabled: bool) -> ProviderPref {
    ProviderPref {
        id: id.into(),
        enabled,
    }
}

#[test]
fn defaults() {
    let s = ShellSettings::default();
    assert_eq!(s.islands, Islands::default());
    assert!(
        s.islands.system && s.islands.work && s.islands.time && s.islands.notif && s.islands.status
    );
    assert!(s.clock_24h && s.providers.is_empty() && s.terminal.is_none());
    let loaded: ShellSettings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(loaded.bar, None, "derived later, from the old switches");
    assert_eq!(s.bar, Some(BarLayout::default()));
    assert_eq!(ShellSettings::from_section(&json!({})).unwrap().bar, None);
    assert!(s.edge_indicators && !s.edge_titles);
    assert_eq!(s.edge_style, EdgeStyle::Tab);
    assert!(s.validate().is_ok());
}

#[test]
fn rejects_bad_provider_ids_and_dupes() {
    let mut set = Settings::default();
    for id in [":1.5", "a", "org..x", ""] {
        let v = json!({"providers": [{"id": id}]});
        assert!(set.set_section("shell", v).is_err(), "{id}");
    }
    let dup = json!({"providers": [{"id": "calc"}, {"id": "calc", "enabled": false}]});
    assert!(set.set_section("shell", dup).is_err());
    let many: Vec<_> = (0..65)
        .map(|i| json!({"id": format!("org.x.P{i}")}))
        .collect();
    assert!(
        set.set_section("shell", json!({"providers": many}))
            .is_err()
    );
    assert_eq!(set.shell, ShellSettings::default(), "errors leave it alone");
    let ok = json!({"providers": [{"id": "run"}, {"id": "org.x.Y", "enabled": false}]});
    set.set_section("shell", ok).unwrap();
    assert_eq!(
        set.shell.providers,
        [pref("run", true), pref("org.x.Y", false)]
    );
    for bad in [
        json!({"nope": 1}),
        json!({"islands": {"nope": true}}),
        json!({"islands": {"system": "yes"}}),
        json!({"islands": 3}),
        json!({"providers": "calc"}),
        json!({"providers": [{"id": "calc", "x": 1}]}),
        json!({"providers": [{"enabled": true}]}),
        json!({"providers": [{"id": "calc", "enabled": 1}]}),
        json!({"clock_24h": "no"}),
        json!({"terminal": 5}),
        json!({"edge_indicators": "off"}),
        json!({"edge_titles": 1}),
        json!({"edge_style": "slice"}),
        json!({"edge_style": true}),
        json!([]),
    ] {
        assert!(set.set_section("shell", bad.clone()).is_err(), "{bad}");
    }
}

#[test]
fn apps_is_not_a_provider() {
    let mut set = Settings::default();
    assert!(
        set.set_section("shell", json!({"providers": [{"id": "apps"}]}))
            .is_err()
    );
    let s: ShellSettings =
        serde_json::from_value(json!({"providers": [{"id": "apps"}, {"id": "run"}]})).unwrap();
    assert_eq!(s.sanitised().providers, [pref("run", true)]);
}

#[test]
fn terminal_must_be_desktop_id() {
    let mut set = Settings::default();
    for bad in [
        "kitty",
        "a b.desktop",
        "../x.desktop",
        "",
        ".desktop.x",
        "-h.desktop",
        "--version.desktop",
        "..x.desktop",
        ".desktop",
        "a..b.desktop",
    ] {
        assert!(
            set.set_section("shell", json!({"terminal": bad})).is_err(),
            "{bad}"
        );
    }
    set.set_section("shell", json!({"terminal": "foot.desktop"}))
        .unwrap();
    assert_eq!(set.shell.terminal.as_deref(), Some("foot.desktop"));
    set.set_section("shell", json!({"terminal": null})).unwrap();
    assert_eq!(set.shell.terminal, None);
    let v = set.section("shell").unwrap();
    assert_eq!(v["clock_24h"], true);
    assert_eq!(v["islands"]["status"], true);
}

#[test]
fn fit_windows_switch() {
    let mut set = Settings::default();
    assert!(set.shell.fit_windows, "on for a new user");
    set.set_section("shell", json!({"fit_windows": false}))
        .unwrap();
    assert!(!set.shell.fit_windows);
    assert_eq!(set.section("shell").unwrap()["fit_windows"], false);
    assert!(
        set.set_section("shell", json!({"fit_windows": "no"}))
            .is_err()
    );
    let s: ShellSettings = serde_json::from_value(json!({"fit_windows": 3})).unwrap();
    assert!(s.fit_windows, "a wrong type keeps the default");
}

#[test]
fn date_format_and_temp_unit() {
    let mut set = Settings::default();
    assert_eq!(set.shell.date_format, DateFormat::Uk, "UK for a new user");
    assert_eq!(set.shell.temp_unit, TempUnit::C, "Celsius for a new user");
    set.set_section("shell", json!({"date_format": "us", "temp_unit": "f"}))
        .unwrap();
    assert_eq!(set.shell.date_format, DateFormat::Us);
    assert_eq!(set.shell.temp_unit, TempUnit::F);
    let v = set.section("shell").unwrap();
    assert_eq!(
        (v["date_format"].as_str(), v["temp_unit"].as_str()),
        (Some("us"), Some("f"))
    );
    for bad in [
        json!({"date_format": "iso"}),
        json!({"temp_unit": "k"}),
        json!({"temp_unit": 1}),
    ] {
        assert!(set.set_section("shell", bad.clone()).is_err(), "{bad}");
    }
    set.set_section("shell", json!({})).unwrap();
    assert_eq!(
        (set.shell.date_format, set.shell.temp_unit),
        (DateFormat::Uk, TempUnit::C)
    );
    let s: ShellSettings =
        serde_json::from_value(json!({"date_format": "eu", "temp_unit": 3})).unwrap();
    assert_eq!(
        (s.date_format, s.temp_unit),
        (DateFormat::Uk, TempUnit::C),
        "lenient load"
    );
}

#[test]
fn edge_indicators_switch() {
    let mut set = Settings::default();
    set.set_section("shell", json!({"edge_indicators": false}))
        .unwrap();
    assert!(!set.shell.edge_indicators);
    assert_eq!(set.section("shell").unwrap()["edge_indicators"], false);
    set.set_section("shell", json!({})).unwrap();
    assert!(set.shell.edge_indicators, "missing means on");
    set.set_section("shell", json!({"edge_titles": true}))
        .unwrap();
    assert!(set.shell.edge_titles);
    assert_eq!(set.section("shell").unwrap()["edge_titles"], true);
    set.set_section("shell", json!({})).unwrap();
    assert!(!set.shell.edge_titles, "missing means icon only");
    set.set_section("shell", json!({"edge_style": "peek"}))
        .unwrap();
    assert_eq!(set.shell.edge_style, EdgeStyle::Peek);
    assert_eq!(set.section("shell").unwrap()["edge_style"], "peek");
    set.set_section("shell", json!({})).unwrap();
    assert_eq!(set.shell.edge_style, EdgeStyle::Tab);
    let s: ShellSettings = serde_json::from_value(json!({"edge_indicators": false})).unwrap();
    assert!(!s.edge_indicators);
}

#[test]
fn provider_order_and_enabled() {
    let prefs = [pref("run", true), pref("org.x.Y", false)];
    let ids: Vec<String> = ["calc", "run", "org.x.Y", "org.a.B"]
        .map(String::from)
        .to_vec();
    assert_eq!(provider_order(&prefs, &ids), ["run", "calc", "org.a.B"]);
    assert!(provider_enabled(&prefs, "run"));
    assert!(!provider_enabled(&prefs, "org.x.Y"));
    assert!(provider_enabled(&prefs, "unlisted"));
    // Unlisted ids: builtins first in builtin order, then others sorted.
    let ids: Vec<String> = ["org.z.Z", "run", "org.a.B", "calc"]
        .map(String::from)
        .to_vec();
    assert_eq!(
        provider_order(&[], &ids),
        ["calc", "run", "org.a.B", "org.z.Z"]
    );
}

#[test]
fn load_is_lenient() {
    let s: ShellSettings = serde_json::from_value(json!({
        "islands": {"system": "x", "work": false, "future": 1},
        "providers": [{"id": "calc"}, 5, {"id": ":1.2"}, {"id": "calc"}, {"id": "org.x.Y", "enabled": "?"}],
        "clock_24h": 3, "terminal": "kitty"
    }))
    .unwrap();
    let s = s.sanitised();
    assert!(s.islands.system && !s.islands.work);
    assert_eq!(s.providers, [pref("calc", true), pref("org.x.Y", true)]);
    assert!(s.clock_24h);
    assert_eq!(s.terminal, None);
    let s: ShellSettings = serde_json::from_value(json!({"edge_indicators": 0})).unwrap();
    assert!(s.edge_indicators, "a wrong type keeps the default");
    let s: ShellSettings = serde_json::from_value(json!({"edge_titles": "yes"})).unwrap();
    assert!(!s.edge_titles, "a wrong type keeps the default");
    let s: ShellSettings = serde_json::from_value(json!({"edge_style": "slice"})).unwrap();
    assert_eq!(
        s.edge_style,
        EdgeStyle::Tab,
        "an unknown style keeps the default"
    );
    let s: ShellSettings =
        serde_json::from_value(json!({"islands": 3, "providers": 4, "terminal": 1})).unwrap();
    assert_eq!(
        s,
        ShellSettings {
            bar: None,
            ..ShellSettings::default()
        }
    );
}
