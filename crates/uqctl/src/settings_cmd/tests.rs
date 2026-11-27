use super::*;
use crate::{Cli, Cmd};
use clap::Parser;

fn parse(args: &[&str]) -> Cmd {
    let mut v = vec!["uqctl"];
    v.extend_from_slice(args);
    Cli::try_parse_from(v).unwrap().cmd
}

fn hit(name: &str, country: &str, lat: f64, lon: f64) -> GeoHit {
    GeoHit {
        name: name.into(),
        country: country.into(),
        lat,
        lon,
    }
}

#[test]
fn parses_every_new_subcommand() {
    assert!(matches!(
        parse(&["pin", "add", "org.a"]),
        Cmd::Pin { cmd: PinCmd::Add { ref id } } if id == "org.a"
    ));
    assert!(matches!(
        parse(&["pin", "rm", "org.a"]),
        Cmd::Pin {
            cmd: PinCmd::Rm { .. }
        }
    ));
    assert!(matches!(
        parse(&["pin", "list"]),
        Cmd::Pin { cmd: PinCmd::List }
    ));
    assert!(matches!(
        parse(&["weather", "set", "Paris", "--pick", "2"]),
        Cmd::Weather { cmd: WeatherCmd::Set { ref city, pick: 2 } } if city == "Paris"
    ));
    assert!(matches!(
        parse(&["weather", "set", "Paris"]),
        Cmd::Weather {
            cmd: WeatherCmd::Set { pick: 1, .. }
        }
    ));
    assert!(matches!(
        parse(&["weather", "off"]),
        Cmd::Weather {
            cmd: WeatherCmd::Off
        }
    ));
    assert!(matches!(
        parse(&["idle", "set", "locked-screen-off", "off"]),
        Cmd::Idle { cmd: IdleCmd::Set { stage: IdleStage::LockedScreenOff, ref value } } if value == "off"
    ));
    assert!(matches!(
        parse(&["notif", "backend", "shell"]),
        Cmd::Notif { cmd: NotifCmd::Backend { ref name } } if name == "shell"
    ));
    assert!(matches!(
        parse(&["privacy", "public-ip", "on"]),
        Cmd::Privacy {
            key: PrivacyKey::PublicIp,
            state: OnOff::On
        }
    ));
    assert!(matches!(
        parse(&["privacy", "launches", "off"]),
        Cmd::Privacy {
            key: PrivacyKey::Launches,
            state: OnOff::Off
        }
    ));
    assert!(matches!(
        parse(&["launches", "clear"]),
        Cmd::Launches {
            cmd: LaunchesCmd::Clear
        }
    ));
}

#[test]
fn rejects_bad_arguments() {
    for bad in [
        &["idle", "set", "nap", "5"][..],
        &["privacy", "public-ip", "maybe"],
        &["pin", "add"],
        &["weather", "set"],
    ] {
        let mut v = vec!["uqctl"];
        v.extend_from_slice(bad);
        assert!(Cli::try_parse_from(v).is_err(), "{bad:?}");
    }
}

#[test]
fn idle_maps_to_setting_keys() {
    assert_eq!(
        idle_setting(IdleStage::Lock, "off").unwrap(),
        ("idle.lock", Value::Null)
    );
    assert_eq!(
        idle_setting(IdleStage::Dim, "240").unwrap(),
        ("idle.dim", json!(240))
    );
    assert_eq!(
        idle_setting(IdleStage::ScreenOff, "360").unwrap().0,
        "idle.screen_off"
    );
    assert_eq!(
        idle_setting(IdleStage::LockedScreenOff, "30").unwrap().0,
        "idle.locked_screen_off"
    );
    assert_eq!(
        idle_setting(IdleStage::Suspend, "1200").unwrap().0,
        "idle.suspend_battery"
    );
}

#[test]
fn idle_rejects_non_numbers() {
    for bad in ["soon", "-5", "1.5", ""] {
        let err = idle_setting(IdleStage::Lock, bad).unwrap_err();
        assert!(err.contains("seconds"), "{err}");
    }
}

#[test]
fn privacy_and_notif_settings() {
    assert_eq!(
        privacy_setting(PrivacyKey::PublicIp, OnOff::On),
        ("privacy.public_ip_lookup", json!(true))
    );
    assert_eq!(
        privacy_setting(PrivacyKey::Launches, OnOff::Off),
        ("privacy.record_launches", json!(false))
    );
    assert_eq!(
        notif_backend_setting("shell"),
        ("notif.backend", json!("shell"))
    );
}

#[test]
fn parses_mouse_commands() {
    assert!(matches!(
        parse(&["mouse", "show"]),
        Cmd::Mouse {
            cmd: MouseCmd::Show
        }
    ));
    assert_eq!(
        parse_mouse(&["speed", "-0.4"]),
        MouseCmd::Speed { value: -0.4 }
    );
    assert_eq!(
        parse_mouse(&["accel", "flat"]),
        MouseCmd::Accel {
            profile: AccelChoice::Flat
        }
    );
    assert_eq!(
        parse_mouse(&["natural-scroll", "on"]),
        MouseCmd::NaturalScroll { state: OnOff::On }
    );
    assert_eq!(
        parse_mouse(&["left-handed", "off"]),
        MouseCmd::LeftHanded { state: OnOff::Off }
    );
    assert_eq!(
        parse_mouse(&["scroll-factor", "2"]),
        MouseCmd::ScrollFactor { value: 2.0 }
    );
    for bad in [
        &["mouse", "speed", "fast"][..],
        &["mouse", "accel", "custom"],
        &["mouse", "left-handed", "yes"],
        &["mouse", "speed"],
    ] {
        let mut v = vec!["uqctl"];
        v.extend_from_slice(bad);
        assert!(Cli::try_parse_from(v).is_err(), "{bad:?}");
    }
}

fn parse_mouse(args: &[&str]) -> MouseCmd {
    let mut v = vec!["mouse"];
    v.extend_from_slice(args);
    match parse(&v) {
        Cmd::Mouse { cmd } => cmd,
        _ => panic!("not a mouse command: {args:?}"),
    }
}

#[test]
fn mouse_maps_to_setting_keys() {
    assert_eq!(mouse_setting(&MouseCmd::Show), None);
    assert_eq!(
        mouse_setting(&MouseCmd::Speed { value: -0.4 }),
        Some(("mouse.sensitivity", json!(-0.4)))
    );
    assert_eq!(
        mouse_setting(&MouseCmd::Accel {
            profile: AccelChoice::Default
        }),
        Some(("mouse.accel_profile", json!("default")))
    );
    assert_eq!(
        mouse_setting(&MouseCmd::NaturalScroll { state: OnOff::On }),
        Some(("mouse.natural_scroll", json!(true)))
    );
    assert_eq!(
        mouse_setting(&MouseCmd::LeftHanded { state: OnOff::Off }),
        Some(("mouse.left_handed", json!(false)))
    );
    assert_eq!(
        mouse_setting(&MouseCmd::ScrollFactor { value: 1.5 }),
        Some(("mouse.scroll_factor", json!(1.5)))
    );
}

#[test]
fn mouse_show_lines() {
    let v = json!({"mouse": {"sensitivity": -0.5, "accel_profile": "flat",
        "natural_scroll": true, "left_handed": false, "scroll_factor": 1.0}});
    assert_eq!(
        mouse_lines(&v),
        [
            "speed\t-0.5",
            "accel\tflat",
            "natural-scroll\ton",
            "left-handed\toff",
            "scroll-factor\t1"
        ]
    );
    // An older uqd without mouse settings shows the defaults.
    assert_eq!(mouse_lines(&json!({}))[0], "speed\t0");
}

#[test]
fn pin_list_edits() {
    let cur = vec!["a".to_string(), "b".to_string()];
    assert_eq!(pinned_with(&cur, "c").unwrap(), ["a", "b", "c"]);
    assert_eq!(pinned_with(&cur, "a"), None);
    assert_eq!(pinned_without(&cur, "a").unwrap(), ["b"]);
    assert_eq!(pinned_without(&cur, "z"), None);
    assert_eq!(
        pinned_of(&json!({"pinned": ["x", "y"], "idle": {}})),
        ["x", "y"]
    );
    assert!(pinned_of(&json!({})).is_empty());
}

#[test]
fn history_lines_format() {
    let json = json!([
        {"id": "1800000000-0001", "time": 1_800_000_000, "origin": "uqctl", "domain": "look",
         "summary": "gaps_in: 5 → 8", "before": {}, "after": {}},
        {"id": "1799999999-0000", "time": 1_799_999_999, "origin": "settings",
         "domain": "theme", "summary": "theme: graphite → nord", "before": {}, "after": {}}
    ])
    .to_string();
    let lines = history_lines(&json).unwrap();
    assert_eq!(
        lines,
        [
            format!(
                "1800000000-0001  {}  uqctl  gaps_in: 5 → 8",
                local_time(1_800_000_000)
            ),
            format!(
                "1799999999-0000  {}  settings  theme: graphite → nord",
                local_time(1_799_999_999)
            ),
        ]
    );
    assert_eq!(local_time(1_800_000_000).len(), "2027-01-15 08:00:00".len());
    assert!(history_lines("[]").unwrap().is_empty());
    assert!(history_lines("nope").is_err());
    assert_eq!(
        newest_entry_id(&json).unwrap().as_deref(),
        Some("1800000000-0001")
    );
    assert_eq!(newest_entry_id("[]").unwrap(), None);
}

#[test]
fn undo_line_names_the_new_entry() {
    assert_eq!(undo_line("1800000000-0002"), "1800000000-0002");
    assert_eq!(undo_line(""), "nothing changed");
}

#[test]
fn restore_lines_format() {
    let json = json!([
        {"id": "1800000000-abcdef", "name": "Before", "time": 1_800_000_000,
         "kind": {"auto": "before_restore"}},
        {"id": "1700000000-000001", "name": "mine", "time": 1_700_000_000, "kind": "manual"}
    ])
    .to_string();
    let lines = restore_lines(&json).unwrap();
    assert_eq!(
        lines[0],
        format!(
            "1800000000-abcdef  {}  auto:before_restore  Before",
            local_time(1_800_000_000)
        )
    );
    assert!(lines[1].ends_with("  manual  mine"), "{}", lines[1]);
}

#[test]
fn cli_parses_history_undo_and_restore() {
    assert!(matches!(parse(&["history"]), Cmd::History { limit: 20 }));
    assert!(matches!(
        parse(&["history", "--limit", "5"]),
        Cmd::History { limit: 5 }
    ));
    assert!(matches!(
        parse(&["undo"]),
        Cmd::Undo {
            id: None,
            force: false
        }
    ));
    assert!(matches!(
        parse(&["undo", "1-0001", "--force"]),
        Cmd::Undo { id: Some(ref id), force: true } if id == "1-0001"
    ));
    assert_eq!(parse_restore(&["restore", "list"]), RestoreCmd::List);
    assert_eq!(
        parse_restore(&["restore", "save", "before party"]),
        RestoreCmd::Save {
            name: "before party".into()
        }
    );
    assert_eq!(
        parse_restore(&["restore", "delete", "1700000000-abcdef"]),
        RestoreCmd::Delete {
            id: "1700000000-abcdef".into()
        }
    );
    assert_eq!(
        parse_restore(&["restore", "1700000000-abcdef"]),
        RestoreCmd::Restore(vec!["1700000000-abcdef".into()])
    );
}

fn parse_restore(args: &[&str]) -> RestoreCmd {
    match parse(args) {
        Cmd::Restore { cmd } => cmd,
        _ => panic!("not a restore command"),
    }
}

#[test]
fn weather_choice_picks_one_based() {
    let hits = [
        hit("Paris", "France", 48.85, 2.35),
        hit("Paris", "United States", 33.66, -95.55),
    ];
    let (lines, (key, value)) = weather_choice(&hits, 1).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(key, "weather_location");
    assert_eq!(
        value,
        json!({"name": "Paris, France", "lat": 48.85, "lon": 2.35})
    );
    let (_, (_, value)) = weather_choice(&hits, 2).unwrap();
    assert_eq!(value["lat"], 33.66);
    assert!(weather_choice(&hits, 0).is_err());
    assert!(weather_choice(&hits, 3).is_err());
    assert!(weather_choice(&[], 1).unwrap_err().contains("no match"));
}
