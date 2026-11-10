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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Position;

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
