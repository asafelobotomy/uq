//! File bodies, rendered from validated values only. Each template is fixed text with the
//! closed values dropped in; no caller string reaches a file without passing [`crate::values`]
//! (or, for monitors, `uq_core`'s strict monitors validation).

use crate::values::{GreeterVariant, PowerAction, ThemeName, UserName};
use uq_core::settings::Monitors;

/// The command `[initial_session]` runs: the uQ session, never a caller string.
pub const SESSION_COMMAND: &str = "/usr/bin/uq-session";

/// `/etc/systemd/logind.conf.d/50-uq.conf`.
pub fn logind(lid: PowerAction, lid_on_ac: PowerAction, power_key: PowerAction) -> String {
    format!(
        "# Written by uq-settings-helper for uQ Settings (Power). Changes made here are replaced\n\
         # the next time the setting is changed. logind reads it at its next reload or reboot.\n\
         [Login]\n\
         HandleLidSwitch={}\n\
         HandleLidSwitchExternalPower={}\n\
         HandlePowerKey={}\n",
        lid.as_str(),
        lid_on_ac.as_str(),
        power_key.as_str()
    )
}

/// `/etc/uq/greeter.toml`.
pub fn greeter(
    theme: &ThemeName,
    variant: GreeterVariant,
    hide_users: bool,
    reduce_motion: bool,
) -> String {
    format!(
        "# Written by uq-settings-helper for uQ Settings (Login screen).\n\
         theme = \"{}\"\n\
         variant = \"{}\"\n\
         hide_users = {hide_users}\n\
         reduce_motion = {reduce_motion}\n",
        theme.as_str(),
        variant.as_str()
    )
}

/// `/etc/uq/greeter-monitors.toml`: the validated layout, without the free-text `description`
/// metadata (it is never rendered, so the root-owned file doesn't carry caller text).
pub fn greeter_monitors(monitors: &Monitors) -> String {
    let mut m = monitors.clone();
    for cfg in m.outputs.values_mut() {
        cfg.description = None;
    }
    let body = toml::to_string(&m).expect("monitors serialise to TOML");
    format!("# Written by uq-settings-helper for uQ Settings (Login screen).\n{body}")
}

/// Why `greetd.toml` couldn't be edited.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GreetdError {
    #[error("/etc/uq/greetd.toml isn't valid TOML, so it was left unchanged")]
    Parse,
}

/// Set (or with `None`, remove) the `[initial_session]` table of an existing greetd config,
/// keeping every other table, key and comment as it was.
pub fn greetd_autologin(existing: &str, user: Option<&UserName>) -> Result<String, GreetdError> {
    let mut doc: toml_edit::DocumentMut = existing.parse().map_err(|_| GreetdError::Parse)?;
    doc.remove("initial_session");
    if let Some(user) = user {
        let mut t = toml_edit::Table::new();
        t.insert("command", toml_edit::value(SESSION_COMMAND));
        t.insert("user", toml_edit::value(user.as_str()));
        doc.insert("initial_session", toml_edit::Item::Table(t));
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn theme(s: &str) -> ThemeName {
        ThemeName::parse(s).unwrap()
    }

    #[test]
    fn logind_golden() {
        assert_eq!(
            logind(
                PowerAction::Suspend,
                PowerAction::Lock,
                PowerAction::Poweroff
            ),
            "# Written by uq-settings-helper for uQ Settings (Power). Changes made here are replaced\n\
             # the next time the setting is changed. logind reads it at its next reload or reboot.\n\
             [Login]\n\
             HandleLidSwitch=suspend\n\
             HandleLidSwitchExternalPower=lock\n\
             HandlePowerKey=poweroff\n"
        );
    }

    #[test]
    fn logind_every_combination_has_exactly_three_settings() {
        for a in PowerAction::ALL {
            for b in PowerAction::ALL {
                for c in PowerAction::ALL {
                    let out = logind(a, b, c);
                    let keys: Vec<&str> = out
                        .lines()
                        .filter(|l| !l.starts_with('#') && l.contains('='))
                        .collect();
                    assert_eq!(
                        keys,
                        [
                            format!("HandleLidSwitch={}", a.as_str()),
                            format!("HandleLidSwitchExternalPower={}", b.as_str()),
                            format!("HandlePowerKey={}", c.as_str()),
                        ]
                    );
                    assert_eq!(out.lines().count(), 6);
                }
            }
        }
    }

    #[test]
    fn greeter_golden() {
        assert_eq!(
            greeter(&theme("dracula"), GreeterVariant::Light, true, false),
            "# Written by uq-settings-helper for uQ Settings (Login screen).\n\
             theme = \"dracula\"\n\
             variant = \"light\"\n\
             hide_users = true\n\
             reduce_motion = false\n"
        );
    }

    #[test]
    fn greeter_output_parses_back() {
        for v in GreeterVariant::ALL {
            for (h, r) in [(false, false), (true, true), (false, true)] {
                let out = greeter(&theme("my-theme-2"), v, h, r);
                let t: toml::Table = out.parse().unwrap();
                assert_eq!(t["theme"].as_str(), Some("my-theme-2"));
                assert_eq!(t["variant"].as_str(), Some(v.as_str()));
                assert_eq!(t["hide_users"].as_bool(), Some(h));
                assert_eq!(t["reduce_motion"].as_bool(), Some(r));
                assert_eq!(t.len(), 4);
            }
        }
    }

    #[test]
    fn greeter_monitors_golden_and_drops_descriptions() {
        let m = Monitors::from_section(&json!({"outputs": {
            "DP-1": {"mode": "2560x1440@144", "position": {"x": 0, "y": 0}, "scale": 1.25,
                     "description": "Dell \"U2720Q\"\nINJECT = 1"},
            "eDP-1": {"enabled": false}
        }}))
        .unwrap();
        let out = greeter_monitors(&m);
        assert_eq!(
            out,
            "# Written by uq-settings-helper for uQ Settings (Login screen).\n\
             [outputs.DP-1]\n\
             mode = \"2560x1440@144\"\n\
             scale = 1.25\n\
             \n\
             [outputs.DP-1.position]\n\
             x = 0\n\
             y = 0\n\
             \n\
             [outputs.eDP-1]\n\
             enabled = false\n"
        );
        assert!(!out.contains("INJECT") && !out.contains("Dell"));
        // It loads back as the same layout (minus the description).
        let back: Monitors = toml::from_str(&out).unwrap();
        let mut want = m.clone();
        want.outputs.get_mut("DP-1").unwrap().description = None;
        assert_eq!(back, want);
    }

    #[test]
    fn greeter_monitors_empty_layout() {
        let out = greeter_monitors(&Monitors::default());
        let back: Monitors = toml::from_str(&out).unwrap();
        assert!(back.outputs.is_empty());
    }

    const GREETD: &str = "\
# uQ's greetd config (shipped by the uq package).
[terminal]
vt = 1

[default_session]
# the greeter
command = \"/usr/lib/uq/uq-greeter-session\"
user = \"greeter\"
";

    #[test]
    fn greetd_autologin_adds_the_table_and_keeps_the_rest() {
        let u = UserName::parse("alice").unwrap();
        let out = greetd_autologin(GREETD, Some(&u)).unwrap();
        assert_eq!(
            out,
            format!(
                "{GREETD}\n[initial_session]\ncommand = \"/usr/bin/uq-session\"\nuser = \"alice\"\n"
            )
        );
        let t: toml::Table = out.parse().unwrap();
        assert_eq!(t["default_session"]["user"].as_str(), Some("greeter"));
        assert_eq!(t["terminal"]["vt"].as_integer(), Some(1));
    }

    #[test]
    fn greetd_autologin_replaces_and_removes() {
        let existing = format!(
            "{GREETD}\n[initial_session]\ncommand = \"/bin/sh -c evil\"\nuser = \"mallory\"\nextra = 1\n"
        );
        let u = UserName::parse("bob").unwrap();
        let out = greetd_autologin(&existing, Some(&u)).unwrap();
        let t: toml::Table = out.parse().unwrap();
        let s = t["initial_session"].as_table().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s["command"].as_str(), Some(SESSION_COMMAND));
        assert_eq!(s["user"].as_str(), Some("bob"));
        assert!(!out.contains("evil") && !out.contains("mallory"));

        let off = greetd_autologin(&out, None).unwrap();
        assert!(!off.contains("initial_session"));
        assert!(off.starts_with(GREETD));
        // Removing when absent is a no-op.
        assert_eq!(greetd_autologin(GREETD, None).unwrap(), GREETD);
    }

    #[test]
    fn greetd_autologin_replaces_dotted_or_inline_forms() {
        for existing in [
            format!("{GREETD}initial_session = {{ command = \"x\", user = \"y\" }}\n"),
            format!("initial_session.user = \"y\"\n{GREETD}"),
        ] {
            let u = UserName::parse("carol").unwrap();
            let out = greetd_autologin(&existing, Some(&u)).unwrap();
            let t: toml::Table = out.parse().unwrap();
            let s = t["initial_session"].as_table().unwrap();
            assert_eq!(s["user"].as_str(), Some("carol"));
            assert_eq!(s["command"].as_str(), Some(SESSION_COMMAND));
            assert_eq!(s.len(), 2);
        }
    }

    #[test]
    fn greetd_autologin_refuses_a_broken_file() {
        let u = UserName::parse("alice").unwrap();
        assert_eq!(
            greetd_autologin("[default_session\nuser=", Some(&u)),
            Err(GreetdError::Parse)
        );
    }
}
