//! The `input` table of `hl.config` (mouse, keyboard, touchpad) and the `hl.device` overrides.
//! Every `None` is left out so base.lua's (or Hyprland's) default applies.

use super::lua_string;
use crate::settings::{
    AccelProfile, DeviceOverride, InputSettings, KeyboardSettings, MouseSettings, TouchpadSettings,
};
use std::fmt::Write as _;

/// The whole `input = { ... }` table: mouse lines first (byte for byte as before the keyboard
/// and touchpad options existed), then keyboard, then the touchpad table.
pub(super) fn emit_input(out: &mut String, m: &MouseSettings, i: &InputSettings) {
    let _ = writeln!(out, "    input = {{");
    emit_mouse(out, m);
    emit_keyboard(out, &i.keyboard);
    emit_touchpad(out, &i.touchpad);
    let _ = writeln!(out, "    }},");
}

/// Mouse lines (`uqctl mouse`), inside the `input` table.
fn emit_mouse(out: &mut String, m: &MouseSettings) {
    let _ = writeln!(out, "        sensitivity = {:?},", m.sensitivity);
    match m.accel_profile {
        // Leaving the key out keeps libinput's per-device default.
        AccelProfile::Default => {}
        AccelProfile::Adaptive => {
            let _ = writeln!(out, "        accel_profile = \"adaptive\",");
        }
        AccelProfile::Flat => {
            let _ = writeln!(out, "        accel_profile = \"flat\",");
        }
    }
    let _ = writeln!(out, "        natural_scroll = {},", m.natural_scroll);
    let _ = writeln!(out, "        left_handed = {},", m.left_handed);
    let _ = writeln!(out, "        scroll_factor = {:?},", m.scroll_factor);
}

fn emit_keyboard(out: &mut String, k: &KeyboardSettings) {
    if !k.layouts.is_empty() {
        let layouts: Vec<&str> = k.layouts.iter().map(|l| l.layout.as_str()).collect();
        let variants: Vec<&str> = k
            .layouts
            .iter()
            .map(|l| l.variant.as_deref().unwrap_or(""))
            .collect();
        let _ = writeln!(
            out,
            "        kb_layout = {},",
            lua_string(&layouts.join(","))
        );
        let _ = writeln!(
            out,
            "        kb_variant = {},",
            lua_string(&variants.join(","))
        );
    }
    if let Some(c) = &k.switch_combo {
        let _ = writeln!(out, "        kb_options = {},", lua_string(c));
    }
    if let Some(r) = k.repeat_rate {
        let _ = writeln!(out, "        repeat_rate = {r},");
    }
    if let Some(d) = k.repeat_delay {
        let _ = writeln!(out, "        repeat_delay = {d},");
    }
}

fn emit_touchpad(out: &mut String, t: &TouchpadSettings) {
    let mut f = Vec::new();
    if let Some(b) = t.natural_scroll {
        f.push(format!("natural_scroll = {b}"));
    }
    if let Some(b) = t.tap_to_click {
        f.push(format!("tap_to_click = {b}"));
    }
    if let Some(b) = t.disable_while_typing {
        f.push(format!("disable_while_typing = {b}"));
    }
    if let Some(s) = t.scroll_factor {
        f.push(format!("scroll_factor = {s:?}"));
    }
    if !f.is_empty() {
        let _ = writeln!(out, "        touchpad = {{ {} }},", f.join(", "));
    }
}

fn device_line(name: &str, d: &DeviceOverride) -> String {
    let mut f = vec![format!("name = {}", lua_string(name))];
    if let Some(s) = d.sensitivity {
        f.push(format!("sensitivity = {s:?}"));
    }
    match d.accel_profile {
        None | Some(AccelProfile::Default) => {}
        Some(AccelProfile::Adaptive) => f.push("accel_profile = \"adaptive\"".into()),
        Some(AccelProfile::Flat) => f.push("accel_profile = \"flat\"".into()),
    }
    if let Some(b) = d.natural_scroll {
        f.push(format!("natural_scroll = {b}"));
    }
    if let Some(b) = d.left_handed {
        f.push(format!("left_handed = {b}"));
    }
    if let Some(s) = d.scroll_factor {
        f.push(format!("scroll_factor = {s:?}"));
    }
    format!("hl.device({{ {} }})", f.join(", "))
}

/// One `hl.device` line per stored override, in name order, each ending in a newline.
pub fn emit_devices(i: &InputSettings) -> String {
    i.devices
        .iter()
        .map(|(n, d)| format!("{}\n", device_line(n, d)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::emit_generated;
    use crate::settings::{DeviceOverride, KbLayout, Settings, TouchpadSettings};
    use crate::theme::{Theme, Variant, tests::FIXTURE};
    use std::collections::BTreeMap;

    fn render(s: &Settings) -> String {
        let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
        emit_generated(&t, &BTreeMap::new(), s)
    }

    #[test]
    fn default_input_unchanged() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let reg = crate::theme::ThemeRegistry::load_dir(&root.join("../../themes")).unwrap();
        let theme = reg.get("graphite", Variant::Dark).unwrap();
        let rules = BTreeMap::from([("2".to_string(), crate::lua::Layout::Scrolling)]);
        let out = emit_generated(theme, &rules, &Settings::default());
        let golden =
            std::fs::read_to_string(root.join("tests/golden/generated/default.lua")).unwrap();
        assert_eq!(out, golden);
    }

    #[test]
    fn keyboard_and_touchpad() {
        let mut s = Settings::default();
        assert!(!render(&s).contains("touchpad"));
        s.input.keyboard.layouts = vec![
            KbLayout {
                layout: "us".into(),
                variant: None,
            },
            KbLayout {
                layout: "de".into(),
                variant: Some("nodeadkeys".into()),
            },
        ];
        s.input.keyboard.switch_combo = Some("grp:alt_shift_toggle".into());
        s.input.keyboard.repeat_rate = Some(40);
        s.input.keyboard.repeat_delay = Some(300);
        s.input.touchpad = TouchpadSettings {
            tap_to_click: Some(false),
            ..Default::default()
        };
        let out = render(&s);
        for want in [
            "        kb_layout = \"us,de\",",
            "        kb_variant = \",nodeadkeys\",",
            "        kb_options = \"grp:alt_shift_toggle\",",
            "        repeat_rate = 40,",
            "        repeat_delay = 300,",
            "        touchpad = { tap_to_click = false },",
        ] {
            assert!(out.contains(want), "{want}\n{out}");
        }
        // The mouse lines stay first.
        assert!(out.find("scroll_factor = 1.0").unwrap() < out.find("kb_layout").unwrap());
    }

    #[test]
    fn device_names_are_escaped() {
        let mut s = Settings::default();
        let base_lines = render(&s).lines().count();
        s.input.devices.insert(
            "x\"y\\z".into(),
            DeviceOverride {
                sensitivity: Some(0.5),
                ..Default::default()
            },
        );
        let out = render(&s);
        assert!(
            out.contains(r#"hl.device({ name = "x\"y\\z", sensitivity = 0.5 })"#),
            "{out}"
        );
        // A blank line, the comment header and the one device line, nothing else.
        assert_eq!(out.lines().count(), base_lines + 3, "{out}");
    }
}
