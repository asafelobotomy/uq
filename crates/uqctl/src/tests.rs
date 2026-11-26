use super::*;

#[test]
fn fixed_schedule_json() {
    let json = schedule_json(&ScheduleCmd::Fixed {
        light_at: "07:00".into(),
        dark_at: "19:30".into(),
    })
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["mode"], "fixed");
    assert_eq!(v["light_at"], "07:00:00");
    assert_eq!(v["dark_at"], "19:30:00");
}

#[test]
fn sun_and_off_schedule_json() {
    let v: serde_json::Value = serde_json::from_str(
        &schedule_json(&ScheduleCmd::Sun {
            lat: 51.5,
            lon: -0.13,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        (v["mode"].as_str(), v["lat"].as_f64()),
        (Some("sun"), Some(51.5))
    );
    assert_eq!(
        schedule_json(&ScheduleCmd::Off).unwrap(),
        r#"{"mode":"off"}"#
    );
}

#[test]
fn active_workspace_selector_prefers_id() {
    let json = r#"{"id": 3, "name": "web", "monitor": "DP-1", "windows": 2}"#;
    assert_eq!(active_workspace_selector(json).unwrap(), "3");
    let special = r#"{"id": -98, "name": "special:magic"}"#;
    assert_eq!(active_workspace_selector(special).unwrap(), "special:magic");
    let named = r#"{"id": -1337, "name": "scratch"}"#;
    assert_eq!(active_workspace_selector(named).unwrap(), "name:scratch");
    assert!(active_workspace_selector("not json").is_err());
    assert!(active_workspace_selector(r#"{"name": 1}"#).is_err());
}

#[test]
fn cli_parses_setup() {
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "setup"]).unwrap().cmd,
        Cmd::Setup {
            apps: false,
            json: false,
            quiet: false
        }
    ));
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "setup", "--apps"])
            .unwrap()
            .cmd,
        Cmd::Setup {
            apps: true,
            json: false,
            quiet: false
        }
    ));
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "setup", "--quiet"])
            .unwrap()
            .cmd,
        Cmd::Setup { quiet: true, .. }
    ));
    assert!(Cli::try_parse_from(["uqctl", "setup", "--quiet", "--json"]).is_err());
}

#[test]
fn cli_parses_greeter() {
    use greeter_cmd::GreeterCmd;
    for (word, want) in [
        ("enable", GreeterCmd::Enable),
        ("disable", GreeterCmd::Disable),
    ] {
        let cli = Cli::try_parse_from(["uqctl", "greeter", word]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Greeter { cmd } if cmd == want));
    }
    assert!(Cli::try_parse_from(["uqctl", "greeter"]).is_err());
}

#[test]
fn bad_time_is_rejected() {
    let err = schedule_json(&ScheduleCmd::Fixed {
        light_at: "7am".into(),
        dark_at: "19:00".into(),
    })
    .unwrap_err();
    assert!(err.contains("7am"), "{err}");
}

#[test]
fn cli_parses_integrations() {
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "integrations", "list", "--json"])
            .unwrap()
            .cmd,
        Cmd::Integrations {
            cmd: IntegrationsCmd::List {
                json: true,
                root: None
            }
        }
    ));
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "integrations", "check", "uq"])
            .unwrap()
            .cmd,
        Cmd::Integrations {
            cmd: IntegrationsCmd::Check {
                no_programs: false,
                ..
            }
        }
    ));
    assert!(matches!(
        Cli::try_parse_from(["uqctl", "integrations", "check", "--no-programs", "uq"])
            .unwrap()
            .cmd,
        Cmd::Integrations {
            cmd: IntegrationsCmd::Check {
                no_programs: true,
                ..
            }
        }
    ));
}

#[test]
fn cli_parses_theme_set() {
    let cli = Cli::try_parse_from(["uqctl", "theme", "set", "rosewood", "light"]).unwrap();
    assert!(
        matches!(cli.cmd, Cmd::Theme { cmd: ThemeCmd::Set { ref id, ref variant } } if id == "rosewood" && variant == "light")
    );
}
