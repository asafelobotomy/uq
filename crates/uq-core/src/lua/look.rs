//! The look, layout-option and animation parts of `generated.lua`. Every `None` is left out so
//! base.lua's (or Hyprland's) default applies.

use super::lua_string;
use crate::settings::{LayoutOptions, Look, round2};
use std::fmt::Write as _;

/// The bezier curves base.lua used to define: name and the two control points.
pub const CURVES: [(&str, [f64; 4]); 4] = [
    ("easeOutQuint", [0.23, 1.0, 0.32, 1.0]),
    ("linear", [0.0, 0.0, 1.0, 1.0]),
    ("almostLinear", [0.5, 0.5, 0.75, 1.0]),
    ("quick", [0.15, 0.0, 0.1, 1.0]),
];

/// The animation leaves base.lua used to define: leaf, speed at animation_speed 1.0, bezier, style.
pub const ANIMATION_LEAVES: [(&str, f64, &str, Option<&str>); 7] = [
    ("global", 10.0, "default", None),
    ("border", 5.39, "easeOutQuint", None),
    ("windowsIn", 4.1, "easeOutQuint", Some("popin 87%")),
    ("windowsOut", 1.49, "linear", Some("popin 87%")),
    ("fade", 3.03, "quick", None),
    ("layers", 3.81, "easeOutQuint", None),
    ("workspaces", 1.94, "almostLinear", Some("fade")),
];

/// Whole numbers without a `.0`, so the default output matches the old hand-written lines.
fn num(x: f64) -> String {
    format!("{x}")
}

/// `decoration` keys that follow `rounding` (blur and shadow); each table only when set.
pub(super) fn emit_decoration_extras(out: &mut String, look: &Look) {
    let mut blur = Vec::new();
    if let Some(b) = look.blur {
        blur.push(format!("enabled = {b}"));
    }
    if let Some(n) = look.blur_size {
        blur.push(format!("size = {n}"));
    }
    if let Some(n) = look.blur_passes {
        blur.push(format!("passes = {n}"));
    }
    if !blur.is_empty() {
        let _ = writeln!(out, "        blur = {{ {} }},", blur.join(", "));
    }
    if let Some(s) = look.shadow {
        let _ = writeln!(out, "        shadow = {{ enabled = {s} }},");
    }
}

/// The `animations`, `dwindle`, `master` and `scrolling` tables of `hl.config`.
pub(super) fn emit_option_tables(out: &mut String, look: &Look, lo: &LayoutOptions) {
    if let Some(a) = look.animations {
        let _ = writeln!(out, "    animations = {{ enabled = {a} }},");
    }
    let d = &lo.dwindle;
    let mut dwindle = Vec::new();
    if let Some(b) = d.preserve_split {
        dwindle.push(format!("preserve_split = {b}"));
    }
    if let Some(b) = d.smart_split {
        dwindle.push(format!("smart_split = {b}"));
    }
    if let Some(f) = d.force_split {
        dwindle.push(format!("force_split = {}", f.as_int()));
    }
    table(out, "dwindle", &dwindle);
    let m = &lo.master;
    let mut master = Vec::new();
    if let Some(f) = m.mfact {
        master.push(format!("mfact = {f:?}"));
    }
    if let Some(s) = m.new_status {
        master.push(format!("new_status = {}", lua_string(s.as_str())));
    }
    if let Some(o) = m.orientation {
        master.push(format!("orientation = {}", lua_string(o.as_str())));
    }
    table(out, "master", &master);
    let s = &lo.scrolling;
    let mut scrolling = Vec::new();
    if let Some(w) = s.column_width {
        scrolling.push(format!("column_width = {w:?}"));
    }
    if let Some(b) = s.fullscreen_on_one_column {
        scrolling.push(format!("fullscreen_on_one_column = {b}"));
    }
    table(out, "scrolling", &scrolling);
}

fn table(out: &mut String, name: &str, entries: &[String]) {
    if !entries.is_empty() {
        let _ = writeln!(out, "    {name} = {{ {} }},", entries.join(", "));
    }
}

/// `hl.curve` and `hl.animation` calls, with speeds divided by `look.animation_speed`.
pub(super) fn emit_animations(out: &mut String, look: &Look) {
    let scale = look.animation_speed.unwrap_or(1.0);
    let pt = |x: f64, y: f64| format!("{{{}, {}}}", num(x), num(y));
    let first: Vec<String> = CURVES.iter().map(|(_, p)| pt(p[0], p[1])).collect();
    let second: Vec<String> = CURVES.iter().map(|(_, p)| pt(p[2], p[3])).collect();
    let w1 = first.iter().map(String::len).max().unwrap_or(0) + 2;
    let w2 = second.iter().map(String::len).max().unwrap_or(0) + 1;
    let w0 = CURVES.iter().map(|(n, _)| n.len()).max().unwrap_or(0) + 3;
    for (i, (name, _)) in CURVES.iter().enumerate() {
        let _ = writeln!(
            out,
            "hl.curve({:<w0$} {{ type = \"bezier\", points = {{ {:<w1$}{:<w2$}}} }})",
            format!("{},", lua_string(name)),
            format!("{},", first[i]),
            second[i],
        );
    }
    out.push('\n');
    let speeds: Vec<String> = ANIMATION_LEAVES
        .iter()
        .map(|(_, s, _, _)| num(round2(s / scale).max(0.1)))
        .collect();
    let wl = ANIMATION_LEAVES
        .iter()
        .map(|l| l.0.len())
        .max()
        .unwrap_or(0)
        + 3;
    let ws = speeds.iter().map(String::len).max().unwrap_or(0) + 1;
    let wb = ANIMATION_LEAVES
        .iter()
        .map(|l| l.2.len())
        .max()
        .unwrap_or(0)
        + 3;
    for ((leaf, _, bezier, style), speed) in ANIMATION_LEAVES.iter().zip(&speeds) {
        let mut line = format!(
            "hl.animation({{ leaf = {:<wl$} enabled = true, speed = {:<ws$} bezier = ",
            format!("{},", lua_string(leaf)),
            format!("{speed},"),
        );
        match style {
            Some(st) => {
                let _ = write!(
                    line,
                    "{:<wb$} style = {} }})",
                    format!("{},", lua_string(bezier)),
                    lua_string(st)
                );
            }
            None => {
                let _ = write!(line, "{} }})", lua_string(bezier));
            }
        }
        let _ = writeln!(out, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::{StartupRender, emit_generated};
    use crate::settings::{ForceSplit, Look, NewStatus, Orientation, Settings};
    use crate::theme::{Theme, Variant, tests::FIXTURE};
    use std::collections::BTreeMap;

    fn render(s: &Settings) -> String {
        let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
        let out = emit_generated(
            &t,
            &BTreeMap::new(),
            &crate::rules::Rules::default(),
            &crate::keybinds::Keybinds::default(),
            s,
            &StartupRender::default(),
        );
        // These tests are about look and layout options, not the shortcuts block.
        out.split("\n-- Keyboard shortcuts")
            .next()
            .unwrap()
            .to_string()
    }

    /// Cut from base.lua before the move, byte for byte.
    const OLD: &str = r##"hl.curve("easeOutQuint", { type = "bezier", points = { {0.23, 1},  {0.32, 1} } })
hl.curve("linear",       { type = "bezier", points = { {0, 0},     {1, 1}    } })
hl.curve("almostLinear", { type = "bezier", points = { {0.5, 0.5}, {0.75, 1} } })
hl.curve("quick",        { type = "bezier", points = { {0.15, 0},  {0.1, 1}  } })

hl.animation({ leaf = "global",     enabled = true, speed = 10,   bezier = "default" })
hl.animation({ leaf = "border",     enabled = true, speed = 5.39, bezier = "easeOutQuint" })
hl.animation({ leaf = "windowsIn",  enabled = true, speed = 4.1,  bezier = "easeOutQuint", style = "popin 87%" })
hl.animation({ leaf = "windowsOut", enabled = true, speed = 1.49, bezier = "linear",       style = "popin 87%" })
hl.animation({ leaf = "fade",       enabled = true, speed = 3.03, bezier = "quick" })
hl.animation({ leaf = "layers",     enabled = true, speed = 3.81, bezier = "easeOutQuint" })
hl.animation({ leaf = "workspaces", enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })
"##;

    #[test]
    fn animation_leaves_match_old_base_lua() {
        let out = render(&Settings::default());
        let mut at = 0;
        for line in OLD.lines().filter(|l| !l.is_empty()) {
            let i = out[at..].find(&format!("{line}\n")).unwrap_or_else(|| {
                panic!("missing or out of order: {line}\n{out}");
            });
            at += i + line.len();
        }
        assert!(out.contains(OLD), "block differs\n{out}");
        assert!(
            !out.contains("animations ="),
            "nothing unset is emitted\n{out}"
        );
    }

    #[test]
    fn unset_look_uses_theme_tokens() {
        let out = render(&Settings::default());
        for want in [
            "gaps_in = 5,",
            "gaps_out = 12,",
            "border_size = 2,",
            "rounding = 10,",
        ] {
            assert!(out.contains(want), "{want}\n{out}");
        }
        for no in [
            "blur",
            "shadow",
            "layout =",
            "dwindle",
            "master",
            "scrolling",
        ] {
            assert!(!out.contains(no), "{no}\n{out}");
        }
        let s = Settings {
            look: Look {
                gaps_in: Some(0),
                gaps_out: Some(30),
                border_width: Some(4),
                rounding: Some(0),
                ..Look::default()
            },
            ..Settings::default()
        };
        let out = render(&s);
        for want in [
            "gaps_in = 0,",
            "gaps_out = 30,",
            "border_size = 4,",
            "rounding = 0,",
        ] {
            assert!(out.contains(want), "{want}\n{out}");
        }
    }

    #[test]
    fn blur_table_only_with_set_keys() {
        let mut s = Settings::default();
        s.look.blur_passes = Some(3);
        let out = render(&s);
        assert!(out.contains("blur = { passes = 3 },"), "{out}");
        s.look.blur = Some(false);
        s.look.blur_size = Some(8);
        s.look.shadow = Some(true);
        s.look.animations = Some(false);
        let out = render(&s);
        assert!(
            out.contains("blur = { enabled = false, size = 8, passes = 3 },"),
            "{out}"
        );
        assert!(out.contains("shadow = { enabled = true },"), "{out}");
        assert!(out.contains("animations = { enabled = false },"), "{out}");
    }

    #[test]
    fn reduce_motion_turns_animations_off() {
        let mut s = Settings::default();
        assert!(!render(&s).contains("animations = {"));
        s.ui.reduce_motion = true;
        let out = render(&s);
        assert!(out.contains("animations = { enabled = false },"), "{out}");
        // Even when the look turns them on.
        s.look.animations = Some(true);
        let out = render(&s);
        assert!(out.contains("animations = { enabled = false },"), "{out}");
        assert!(!out.contains("animations = { enabled = true },"), "{out}");
    }

    #[test]
    fn speed_scales_leaves() {
        let mut s = Settings::default();
        s.look.animation_speed = Some(2.0);
        let out = render(&s);
        assert!(
            out.contains("leaf = \"windowsIn\",  enabled = true, speed = 2.05,"),
            "{out}"
        );
        assert!(out.contains("speed = 5,    bezier = \"default\""), "{out}");
        s.look.animation_speed = Some(4.0);
        let out = render(&s);
        assert!(out.contains("speed = 0.37,"), "{out}");
        s.look.animation_speed = Some(0.25);
        let out = render(&s);
        assert!(out.contains("speed = 40,"), "{out}");
    }

    #[test]
    fn layout_options_emit_only_set_keys() {
        let mut s = Settings::default();
        s.layout_options.dwindle.force_split = Some(ForceSplit::Right);
        s.layout_options.master.new_status = Some(NewStatus::Master);
        let out = render(&s);
        assert!(out.contains("dwindle = { force_split = 2 },"), "{out}");
        assert!(
            out.contains("master = { new_status = \"master\" },"),
            "{out}"
        );
        assert!(!out.contains("scrolling"), "{out}");
        assert!(!out.contains("preserve_split"), "{out}");
        s.layout_options.master.orientation = Some(Orientation::Top);
        s.layout_options.master.mfact = Some(0.6);
        s.layout_options.default = Some(crate::lua::Layout::Scrolling);
        s.layout_options.scrolling.column_width = Some(0.5);
        s.layout_options.scrolling.fullscreen_on_one_column = Some(true);
        let out = render(&s);
        assert!(out.contains("layout = \"scrolling\","), "{out}");
        assert!(
            out.contains(
                "master = { mfact = 0.6, new_status = \"master\", orientation = \"top\" },"
            ),
            "{out}"
        );
        assert!(
            out.contains("scrolling = { column_width = 0.5, fullscreen_on_one_column = true },"),
            "{out}"
        );
    }
}
