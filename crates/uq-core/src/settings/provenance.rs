//! Where a setting's live value comes from (spec §2): which Hyprland option or gsettings key
//! each section key lands in, what uQ renders for it, and a plain-text scan of `user.lua`.

use crate::state::State;
use crate::theme::Theme;
use serde_json::Value;
mod user_lua;
pub use user_lua::*;
mod options;
pub use options::*;

/// What a `null` (unset) key falls back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullSource {
    /// A theme token (gaps, border, rounding, fonts, cursor, icons).
    Theme,
    /// base.lua's or Hyprland's own default.
    Default,
}

impl NullSource {
    /// The `source` an unset key reports.
    pub fn as_str(self) -> &'static str {
        match self {
            NullSource::Theme => "theme",
            NullSource::Default => "default",
        }
    }
}

/// One section key and where it shows up in the live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionRow {
    /// `<domain>.<json path>`, for example `look.gaps_in` or `layouts.dwindle.preserve_split`.
    pub key: &'static str,
    /// The Settings app's setting id (its registry), for example `gaps-inner`.
    pub setting_id: &'static str,
    pub label: &'static str,
    /// The Hyprland option path `getoption` takes, for example `general:gaps_in`.
    pub hypr: Option<&'static str>,
    /// `(schema, key)` for keys applied through gsettings.
    pub gsettings: Option<(&'static str, &'static str)>,
    pub null_source: NullSource,
}

impl OptionRow {
    /// The section this row belongs to (`look`).
    pub fn domain(&self) -> &'static str {
        self.key.split_once('.').map_or(self.key, |(d, _)| d)
    }

    /// The dotted path within the section's JSON (`gaps_in`, `dwindle.preserve_split`).
    pub fn json_key(&self) -> &'static str {
        self.key.split_once('.').map_or("", |(_, k)| k)
    }

    /// This row's value in the section JSON `section`; `Null` when unset.
    pub fn stored(&self, section: &Value) -> Value {
        self.json_key()
            .split('.')
            .try_fold(section, |v, part| v.get(part))
            .cloned()
            .unwrap_or(Value::Null)
    }
}

/// The rows of one section, in [`OPTIONS`] order.
pub fn options_for(domain: &str) -> impl Iterator<Item = &'static OptionRow> + '_ {
    OPTIONS.iter().filter(move |r| r.domain() == domain)
}

/// The display label of `(domain, json key)` for the rows here; history summaries use
/// [`crate::history::label`], which covers every key.
pub fn label_for(domain: &str, json_key: &str) -> Option<&'static str> {
    options_for(domain)
        .find(|r| r.json_key() == json_key)
        .map(|r| r.label)
}

/// The value uQ renders for `row`, in the form the live session reports it (Hyprland's int for
/// `force_split`, gsettings' `"<font> <size>"`). `None` when uQ leaves the key to base.lua or
/// Hyprland, or has nothing to read back. `theme` is the current theme, without appearance
/// overrides.
pub fn expected(row: &OptionRow, state: &State, theme: &Theme) -> Option<Value> {
    let s = &state.settings;
    let (look, lo) = (&s.look, &s.layout_options);
    let t = theme.with_appearance(&s.appearance);
    let num = |n: u8| Some(Value::from(n));
    let b = |v: Option<bool>| v.map(Value::from);
    let f = |v: Option<f64>| v.map(Value::from);
    let font = |name: &str| Some(Value::from(format!("{name} {}", t.fonts.size)));
    match row.key {
        "look.gaps_in" => num(look.gaps_in.unwrap_or(t.ui.gap_in)),
        "look.gaps_out" => num(look.gaps_out.unwrap_or(t.ui.gap_out)),
        "look.border_width" => num(look.border_width.unwrap_or(t.ui.border_width)),
        "look.rounding" => num(look.rounding.unwrap_or(t.ui.radius)),
        "look.blur" => b(look.blur),
        "look.blur_size" => look.blur_size.map(Value::from),
        "look.blur_passes" => look.blur_passes.map(Value::from),
        "look.shadow" => b(look.shadow),
        // Reduce motion (`ui.reduce_motion`) renders animations off whatever `look` says.
        "look.animations" if s.ui.reduce_motion => Some(Value::Bool(false)),
        "look.animations" => b(look.animations),
        "layouts.default" => lo.default.map(|l| Value::from(l.as_str())),
        "layouts.dwindle.preserve_split" => b(lo.dwindle.preserve_split),
        "layouts.dwindle.smart_split" => b(lo.dwindle.smart_split),
        "layouts.dwindle.force_split" => lo.dwindle.force_split.map(|v| Value::from(v.as_int())),
        "layouts.master.mfact" => f(lo.master.mfact),
        "layouts.master.new_status" => lo.master.new_status.map(|v| Value::from(v.as_str())),
        "layouts.master.orientation" => lo.master.orientation.map(|v| Value::from(v.as_str())),
        "layouts.scrolling.column_width" => f(lo.scrolling.column_width),
        "layouts.scrolling.fullscreen_on_one_column" => b(lo.scrolling.fullscreen_on_one_column),
        "appearance.ui_font" | "appearance.font_size" => font(&t.fonts.ui),
        "appearance.mono_font" => font(&t.fonts.mono),
        "appearance.cursor_theme" => Some(Value::from(t.icons.cursor_theme.clone())),
        "appearance.cursor_size" => num(t.icons.cursor_size),
        "appearance.icon_theme" => Some(Value::from(t.icons.theme.clone())),
        // Off renders nothing, so Hyprland's own default is all there is to read back (F6).
        "compat.sharp_x11" => state.compat.sharp_x11.then_some(Value::Bool(true)),
        "input.keyboard.layouts" => {
            let k = &s.input.keyboard;
            (!k.layouts.is_empty()).then(|| {
                let names: Vec<&str> = k.layouts.iter().map(|l| l.layout.as_str()).collect();
                Value::from(names.join(","))
            })
        }
        "input.keyboard.switch_combo" => s.input.keyboard.switch_combo.clone().map(Value::from),
        "input.keyboard.repeat_rate" => s.input.keyboard.repeat_rate.map(Value::from),
        "input.keyboard.repeat_delay" => s.input.keyboard.repeat_delay.map(Value::from),
        "input.mouse.sensitivity" => Some(Value::from(s.mouse.sensitivity)),
        // Default renders nothing: libinput's per-device default.
        "input.mouse.accel_profile" => match s.mouse.accel_profile {
            crate::settings::AccelProfile::Default => None,
            crate::settings::AccelProfile::Adaptive => Some(Value::from("adaptive")),
            crate::settings::AccelProfile::Flat => Some(Value::from("flat")),
        },
        "input.mouse.natural_scroll" => Some(Value::Bool(s.mouse.natural_scroll)),
        "input.mouse.left_handed" => Some(Value::Bool(s.mouse.left_handed)),
        "input.mouse.scroll_factor" => Some(Value::from(s.mouse.scroll_factor)),
        "input.touchpad.natural_scroll" => b(s.input.touchpad.natural_scroll),
        "input.touchpad.tap_to_click" => b(s.input.touchpad.tap_to_click),
        "input.touchpad.disable_while_typing" => b(s.input.touchpad.disable_while_typing),
        "input.touchpad.scroll_factor" => f(s.input.touchpad.scroll_factor),
        "input.swipe.distance" => s.input.swipe.distance.map(Value::from),
        "input.swipe.invert" => b(s.input.swipe.invert),
        "input.swipe.create_new" => b(s.input.swipe.create_new),
        "input.swipe.cancel_ratio" => f(s.input.swipe.cancel_ratio),
        _ => None,
    }
}

/// Whether a live value (Hyprland's `int`/`float`/`bool`/`str`/`css`, or gsettings text) is the
/// `expected` one. Bools match 0/1, numbers match within 1e-4, and a number matches text whose
/// every word is that number (a CSS gap `"5 5 5 5"`, gsettings `"24"`).
pub fn live_matches(expected: &Value, live: &Value) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-4;
    match (expected, live) {
        (Value::Bool(e), Value::Number(n)) => n.as_f64() == Some(if *e { 1.0 } else { 0.0 }),
        (Value::Number(e), Value::Number(l)) => match (e.as_f64(), l.as_f64()) {
            (Some(a), Some(b)) => close(a, b),
            _ => false,
        },
        (Value::Number(e), Value::String(s)) => {
            let Some(e) = e.as_f64() else { return false };
            let words: Vec<&str> = s
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|w| !w.is_empty())
                .collect();
            !words.is_empty()
                && words
                    .iter()
                    .all(|w| w.parse::<f64>().is_ok_and(|n| close(n, e)))
        }
        (Value::Bool(e), Value::String(s)) => s.trim() == e.to_string(),
        (e, l) => e == l,
    }
}

#[cfg(test)]
mod tests;
