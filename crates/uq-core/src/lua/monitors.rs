//! The `hl.monitor` lines of `generated.lua`. Only stored, validated names reach here: output
//! names and mirror targets pass `valid_output_name` and still go through `lua_string`.

use super::lua_string;
use crate::rules::WorkspaceRule;
use crate::settings::plan::{Planned, plan, roles, screens};
use crate::settings::{LiveOutput, Monitors, OutputConfig, valid_output_name};
use std::collections::BTreeSet;

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

/// What the monitors section adds to `generated.lua` beyond `hl.monitor` lines: the workspace
/// plan (merged into the workspace rules) and the default monitor for the cursor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MonitorRender {
    pub planned: Vec<Planned>,
    /// The chosen main display's connector, when it is connected.
    pub default_monitor: Option<String>,
}

/// The plan and the default monitor for `m` on the screens `live` (Hyprland's, with the stored
/// layout laid over them). The default monitor is set only when a main display is chosen and
/// found.
pub fn monitor_render(m: &Monitors, live: &[LiveOutput]) -> MonitorRender {
    let primary = roles(m, live).primary;
    MonitorRender {
        planned: plan(&m.workspaces, &screens(m, live)),
        default_monitor: (!primary.is_empty()).then_some(primary),
    }
}

/// Inside `hl.config({ … })`: `cursor = { default_monitor = "DP-1" },` so the session starts on
/// the main display (Hyprland reads it at startup). Nothing without one.
pub fn emit_cursor(out: &mut String, r: &MonitorRender) {
    if let Some(name) = r
        .default_monitor
        .as_deref()
        .filter(|n| valid_output_name(n))
    {
        out.push_str(&format!(
            "    cursor = {{ default_monitor = {} }},\n",
            lua_string(name)
        ));
    }
}

/// The user's workspace rules with the plan filled in (R14): for each planned workspace, the
/// user's rule for that selector gets `monitor` (and `default`) only where it has none; a
/// workspace with no rule gets one with just those. Added rules follow the user's, in
/// workspace order, so there is still one call per selector.
pub fn merge_planned(rules: &[WorkspaceRule], planned: &[Planned]) -> Vec<WorkspaceRule> {
    let mut out = rules.to_vec();
    // A monitor opens on one workspace: a `default` the person already set on a monitor means
    // the plan adds no second one there.
    let mut has_default: BTreeSet<String> = out
        .iter()
        .filter(|r| r.default == Some(true))
        .filter_map(|r| r.monitor.clone())
        .collect();
    for p in planned.iter().filter(|p| valid_output_name(&p.output)) {
        let sel = p.workspace.to_string();
        match out.iter_mut().find(|r| r.workspace == sel) {
            Some(r) => {
                r.monitor.get_or_insert_with(|| p.output.clone());
                if r.default.is_none()
                    && p.default
                    && r.monitor.as_deref() == Some(p.output.as_str())
                    && has_default.insert(p.output.clone())
                {
                    r.default = Some(true);
                }
            }
            None => out.push(WorkspaceRule {
                workspace: sel,
                monitor: Some(p.output.clone()),
                default: (p.default && has_default.insert(p.output.clone())).then_some(true),
                persistent: None,
                gaps_in: None,
                gaps_out: None,
                border_size: None,
            }),
        }
    }
    out
}

/// `hl.dsp.workspace.move({ workspace = "3", monitor = "DP-1" })`: the dispatcher uqd runs
/// (through `hl.dispatch`) to put a planned workspace back on its screen.
pub fn workspace_move_line(ws: u8, output: &str) -> String {
    format!(
        "hl.dsp.workspace.move({{ workspace = {}, monitor = {} }})",
        lua_string(&ws.to_string()),
        lua_string(output)
    )
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
    use crate::settings::{MonitorId, Position, Strategy, WorkspacePlan};

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

    fn live(name: &str, serial: &str, x: i32) -> LiveOutput {
        LiveOutput {
            name: name.into(),
            make: "Dell".into(),
            model: "U".into(),
            serial: serial.into(),
            width: 1920,
            height: 1080,
            x,
            scale: 1.0,
            ..LiveOutput::default()
        }
    }

    #[test]
    fn default_render_is_empty() {
        let r = MonitorRender::default();
        let mut out = String::new();
        emit_cursor(&mut out, &r);
        assert_eq!(out, "");
        let m = Monitors::default();
        let two = [live("DP-1", "A", 0), live("DP-2", "B", 1920)];
        assert_eq!(monitor_render(&m, &two), r, "nothing set, nothing planned");
    }

    #[test]
    fn cursor_table_shape() {
        let r = MonitorRender {
            default_monitor: Some("DP-1".into()),
            ..MonitorRender::default()
        };
        let mut out = String::new();
        emit_cursor(&mut out, &r);
        assert_eq!(out, "    cursor = { default_monitor = \"DP-1\" },\n");
        // A name that isn't a valid output name never reaches the file.
        let bad = MonitorRender {
            default_monitor: Some("DP-1\"x".into()),
            ..MonitorRender::default()
        };
        let mut out = String::new();
        emit_cursor(&mut out, &bad);
        assert_eq!(out, "");
    }

    #[test]
    fn monitor_render_follows_primary_and_plan() {
        let m = Monitors {
            primary: Some(MonitorId {
                make: "Dell".into(),
                model: "U".into(),
                serial: "B".into(),
            }),
            workspaces: WorkspacePlan {
                strategy: Strategy::Sequential,
                count: 4,
                ..WorkspacePlan::default()
            },
            ..Monitors::default()
        };
        let two = [live("DP-1", "A", 0), live("DP-2", "B", 1920)];
        let r = monitor_render(&m, &two);
        assert_eq!(r.default_monitor.as_deref(), Some("DP-2"));
        let got: Vec<(u8, &str, bool)> = r
            .planned
            .iter()
            .map(|p| (p.workspace, p.output.as_str(), p.default))
            .collect();
        assert_eq!(
            got,
            [
                (1, "DP-1", true),
                (2, "DP-1", false),
                (3, "DP-2", true),
                (4, "DP-2", false)
            ]
        );
        // Not connected: no default monitor.
        assert_eq!(monitor_render(&m, &two[..1]).default_monitor, None);
    }

    #[test]
    fn move_line_escapes_names() {
        assert_eq!(
            workspace_move_line(3, "DP-1"),
            r#"hl.dsp.workspace.move({ workspace = "3", monitor = "DP-1" })"#
        );
        assert_eq!(
            workspace_move_line(3, "a\"b"),
            r#"hl.dsp.workspace.move({ workspace = "3", monitor = "a\"b" })"#
        );
    }

    #[test]
    fn merge_escapes_and_checks_names() {
        let plan = |ws: u8, out: &str| Planned {
            workspace: ws,
            output: out.into(),
            default: true,
        };
        // A name that isn't a valid output name is never planned.
        let merged = merge_planned(&[], &[plan(1, "a\"b"), plan(2, "DP-1")]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].workspace, "2");
        // A user's own default on a monitor means the plan adds none there.
        let user = WorkspaceRule {
            workspace: "2".into(),
            monitor: Some("DP-1".into()),
            default: Some(true),
            persistent: None,
            gaps_in: None,
            gaps_out: None,
            border_size: None,
        };
        let merged = merge_planned(
            &[user],
            &[plan(1, "DP-1"), plan(3, "DP-2"), plan(4, "DP-2")],
        );
        let defaults: Vec<&str> = merged
            .iter()
            .filter(|r| r.default == Some(true))
            .map(|r| r.workspace.as_str())
            .collect();
        assert_eq!(defaults, ["2", "3"], "{merged:?}");
    }
}
