//! The `hl.monitor` lines of `generated.lua`. Only stored, validated names reach here: output
//! names and mirror targets pass `valid_output_name` and still go through `lua_string`.

use super::lua_string;
use crate::settings::{Monitors, OutputConfig};

/// One `hl.monitor({ ... })` line. A disabled output carries nothing else. Otherwise an unset
/// mode, position or scale is written out as `preferred`, `auto`, `auto`: Hyprland would reuse
/// the previous rule's value for an omitted key.
pub fn emit_monitor_line(name: &str, o: &OutputConfig) -> String {
    let mut f = vec![format!("output = {}", lua_string(name))];
    if o.enabled == Some(false) {
        f.push("disabled = true".into());
    } else {
        f.push(format!(
            "mode = {}",
            lua_string(o.mode.as_deref().unwrap_or("preferred"))
        ));
        f.push(match o.position {
            Some(p) => format!("position = {}", lua_string(&format!("{}x{}", p.x, p.y))),
            None => "position = \"auto\"".into(),
        });
        f.push(match o.scale {
            Some(s) => format!("scale = {}", lua_string(&format!("{s:?}"))),
            None => "scale = \"auto\"".into(),
        });
        if let Some(t) = o.transform {
            f.push(format!("transform = {t}"));
        }
        if let Some(m) = &o.mirror_of {
            f.push(format!("mirror = {}", lua_string(m)));
        }
        if let Some(v) = o.vrr {
            f.push(format!("vrr = {v}"));
        }
    }
    format!("hl.monitor({{ {} }})", f.join(", "))
}

/// Every output's line, in name order, each ending in a newline; empty when nothing is stored.
pub fn emit_monitors(m: &Monitors) -> String {
    m.outputs
        .iter()
        .map(|(n, o)| format!("{}\n", emit_monitor_line(n, o)))
        .collect()
}

/// The largest `/etc/uq/greeter-monitors.toml` read (the helper writes a few hundred bytes).
pub const MAX_GREETER_MONITORS: usize = 64 * 1024;

/// The login screen's `hl.monitor` lines (phase 5 spec §2.2) from the text of
/// `/etc/uq/greeter-monitors.toml`: the `monitors` section the helper wrote, as TOML, checked
/// as strictly as uqd's `SetSection("monitors")`. Any problem is an error, and the caller keeps
/// Hyprland's automatic layout. A layout that turns every listed output off is refused too, so
/// a mistake can't leave the login screen dark.
pub fn greeter_monitors_lua(text: &str) -> Result<String, String> {
    if text.len() > MAX_GREETER_MONITORS {
        return Err("the file is too large".into());
    }
    let parsed: toml::Value = toml::from_str(text).map_err(|e| format!("not valid TOML: {e}"))?;
    let json = serde_json::to_value(parsed).map_err(|e| e.to_string())?;
    let m = Monitors::from_section(&json).map_err(|e| e.to_string())?;
    if !m.outputs.is_empty() && m.outputs.values().all(|o| o.enabled == Some(false)) {
        return Err("every output is turned off".into());
    }
    Ok(emit_monitors(&m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Position;

    #[test]
    fn greeter_file_renders_checked_lines() {
        // What uq-settings-helper writes (templates::greeter_monitors).
        let text = "# Written by uq-settings-helper for uQ Settings (Login screen).\n\
                    [outputs.DP-1]\nmode = \"2560x1440@144\"\nscale = 1.25\n\n\
                    [outputs.DP-1.position]\nx = 0\ny = 0\n\n[outputs.eDP-1]\nenabled = false\n";
        assert_eq!(
            greeter_monitors_lua(text).unwrap(),
            "hl.monitor({ output = \"DP-1\", mode = \"2560x1440@144\", position = \"0x0\", \
             scale = \"1.25\" })\nhl.monitor({ output = \"eDP-1\", disabled = true })\n"
        );
        assert_eq!(greeter_monitors_lua("").unwrap(), "");
        assert_eq!(greeter_monitors_lua("[outputs]\n").unwrap(), "");
    }

    #[test]
    fn greeter_file_refuses_anything_odd() {
        for bad in [
            "not toml [",
            "outputs = 3\n",
            "[outputs.DP-1]\nmode = \"huge\"\n",
            "[outputs.DP-1]\nscale = 99.0\n",
            "[outputs.DP-1]\nwhat = 1\n",
            "extra = 1\n",
            // A name that would break out of the Lua string.
            "[outputs.'DP-1\\\")os.exit()--']\nenabled = true\n",
            "[outputs.DP-1]\nmirror_of = \"DP-1\"\n",
            "[outputs.DP-1]\nenabled = false\n[outputs.HDMI-A-1]\nenabled = false\n",
        ] {
            assert!(greeter_monitors_lua(bad).is_err(), "{bad}");
        }
        let big = format!("# {}\n", "x".repeat(MAX_GREETER_MONITORS));
        assert!(greeter_monitors_lua(&big).is_err());
    }

    #[test]
    fn line_shapes() {
        let o = OutputConfig {
            mode: Some("2560x1440@144".into()),
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.25),
            transform: Some(1),
            vrr: Some(1),
            ..Default::default()
        };
        assert_eq!(
            emit_monitor_line("DP-1", &o),
            r#"hl.monitor({ output = "DP-1", mode = "2560x1440@144", position = "0x0", scale = "1.25", transform = 1, vrr = 1 })"#
        );
        let off = OutputConfig {
            enabled: Some(false),
            mode: Some("1920x1080".into()),
            ..Default::default()
        };
        assert_eq!(
            emit_monitor_line("HDMI-A-1", &off),
            r#"hl.monitor({ output = "HDMI-A-1", disabled = true })"#
        );
        assert_eq!(
            emit_monitor_line("eDP-1", &OutputConfig::default()),
            r#"hl.monitor({ output = "eDP-1", mode = "preferred", position = "auto", scale = "auto" })"#
        );
        let mirror = OutputConfig {
            mirror_of: Some("DP-1".into()),
            ..Default::default()
        };
        assert!(
            emit_monitor_line("HDMI-A-1", &mirror).contains(r#"mirror = "DP-1""#),
            "mirror"
        );
    }

    #[test]
    fn monitors_in_name_order() {
        let mut m = Monitors::default();
        m.outputs.insert("HDMI-A-1".into(), OutputConfig::default());
        m.outputs.insert("DP-1".into(), OutputConfig::default());
        let out = emit_monitors(&m);
        assert!(out.find("DP-1").unwrap() < out.find("HDMI-A-1").unwrap());
        assert_eq!(out.lines().count(), 2);
        assert_eq!(emit_monitors(&Monitors::default()), "");
    }
}
