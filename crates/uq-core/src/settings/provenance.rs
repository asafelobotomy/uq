//! Where a setting's live value comes from (spec §2): which Hyprland option or gsettings key
//! each section key lands in, what uQ renders for it, and a plain-text scan of `user.lua`.

use crate::state::State;
use crate::theme::Theme;
use serde_json::Value;

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

const IFACE: &str = "org.gnome.desktop.interface";

const fn hypr(
    key: &'static str,
    setting_id: &'static str,
    label: &'static str,
    path: &'static str,
    null_source: NullSource,
) -> OptionRow {
    OptionRow {
        key,
        setting_id,
        label,
        hypr: Some(path),
        gsettings: None,
        null_source,
    }
}

const fn gsettings(
    key: &'static str,
    setting_id: &'static str,
    label: &'static str,
    gkey: &'static str,
) -> OptionRow {
    OptionRow {
        key,
        setting_id,
        label,
        hypr: None,
        gsettings: Some((IFACE, gkey)),
        null_source: NullSource::Theme,
    }
}

use NullSource::{Default as Dflt, Theme as Tok};

/// Every key provenance explains. Option paths are the `hl.config` table paths `generated.lua`
/// renders (see `crate::lua`), checked against Hyprland 0.56.2's `ConfigValues.cpp`.
pub static OPTIONS: &[OptionRow] = &[
    hypr(
        "look.gaps_in",
        "gaps-inner",
        "Gaps between windows",
        "general:gaps_in",
        Tok,
    ),
    hypr(
        "look.gaps_out",
        "gaps-outer",
        "Gaps around the screen",
        "general:gaps_out",
        Tok,
    ),
    hypr(
        "look.border_width",
        "border-width",
        "Window border width",
        "general:border_size",
        Tok,
    ),
    hypr(
        "look.rounding",
        "rounding",
        "Corner rounding",
        "decoration:rounding",
        Tok,
    ),
    hypr(
        "look.blur",
        "blur",
        "Blur behind windows",
        "decoration:blur:enabled",
        Dflt,
    ),
    hypr(
        "look.blur_size",
        "blur-size",
        "Blur size",
        "decoration:blur:size",
        Dflt,
    ),
    hypr(
        "look.blur_passes",
        "blur-passes",
        "Blur passes",
        "decoration:blur:passes",
        Dflt,
    ),
    hypr(
        "look.shadow",
        "shadow",
        "Window shadows",
        "decoration:shadow:enabled",
        Dflt,
    ),
    hypr(
        "look.animations",
        "animations",
        "Animations",
        "animations:enabled",
        Dflt,
    ),
    // Scales every `hl.animation` speed; Hyprland has no single option to read back.
    OptionRow {
        key: "look.animation_speed",
        setting_id: "animation-speed",
        label: "Animation speed",
        hypr: None,
        gsettings: None,
        null_source: Dflt,
    },
    hypr(
        "layouts.default",
        "default-layout",
        "Default layout",
        "general:layout",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.preserve_split",
        "dwindle-options",
        "Dwindle: keep the split",
        "dwindle:preserve_split",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.smart_split",
        "dwindle-options",
        "Dwindle: split by cursor position",
        "dwindle:smart_split",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.force_split",
        "dwindle-options",
        "Dwindle: new window side",
        "dwindle:force_split",
        Dflt,
    ),
    hypr(
        "layouts.master.mfact",
        "master-options",
        "Master: master area size",
        "master:mfact",
        Dflt,
    ),
    hypr(
        "layouts.master.new_status",
        "master-options",
        "Master: new windows",
        "master:new_status",
        Dflt,
    ),
    hypr(
        "layouts.master.orientation",
        "master-options",
        "Master: master area position",
        "master:orientation",
        Dflt,
    ),
    hypr(
        "layouts.scrolling.column_width",
        "scrolling-options",
        "Scrolling: column width",
        "scrolling:column_width",
        Dflt,
    ),
    hypr(
        "layouts.scrolling.fullscreen_on_one_column",
        "scrolling-options",
        "Scrolling: one column fills the screen",
        "scrolling:fullscreen_on_one_column",
        Dflt,
    ),
    gsettings(
        "appearance.ui_font",
        "ui-font",
        "Interface font",
        "font-name",
    ),
    gsettings(
        "appearance.mono_font",
        "mono-font",
        "Monospace font",
        "monospace-font-name",
    ),
    gsettings(
        "appearance.font_size",
        "font-size",
        "Font size",
        "font-name",
    ),
    gsettings(
        "appearance.cursor_theme",
        "cursor-theme",
        "Cursor theme",
        "cursor-theme",
    ),
    gsettings(
        "appearance.cursor_size",
        "cursor-size",
        "Cursor size",
        "cursor-size",
    ),
    gsettings(
        "appearance.icon_theme",
        "icon-theme",
        "Icon theme",
        "icon-theme",
    ),
];

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

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `needle` occurs in `line` as a whole word (no identifier character on either side).
fn contains_word(line: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(i) = line[from..].find(needle) {
        let start = from + i;
        let end = start + needle.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            return true;
        }
        from = start + needle.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Tables nested deeper than this are counted but not named.
const MAX_TABLE_DEPTH: usize = 64;

/// The 1-based line of `user.lua` text that seems to set Hyprland option `hypr_path`
/// (`general:gaps_in`, `decoration:blur:enabled`). A case-sensitive, plain-text heuristic, not
/// a parser; the first hit wins. A line hits when it has the path in dotted form
/// (`general.gaps_in`) or colon form (`general:gaps_in`), or has `gaps_in =` inside a
/// `general = {` table (tables are tracked by brace depth, so `blur = { enabled = … }` inside
/// `decoration = {` finds `decoration:blur:enabled`). Lua `--` comments are ignored (a `--[[`
/// block comment only on its first line); strings are not parsed, so `--` inside one also ends
/// the scanned part of that line.
pub fn find_in_user_lua(text: &str, hypr_path: &str) -> Option<u32> {
    let (section, leaf) = hypr_path.rsplit_once(':')?;
    let sections: Vec<&str> = section.split(':').collect();
    let dotted = hypr_path.replace(':', ".");
    // One entry per open brace: the table's name when it is `name = {`.
    let mut stack: Vec<Option<&str>> = Vec::new();
    let mut deep = 0usize;
    for (n, line) in text.lines().enumerate() {
        let lineno = u32::try_from(n + 1).unwrap_or(u32::MAX);
        let line = line.find("--").map_or(line, |i| &line[..i]);
        if contains_word(line, &dotted) || contains_word(line, hypr_path) {
            return Some(lineno);
        }
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            if c.is_ascii_alphabetic() || c == '_' {
                let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
                let ident = &rest[..end];
                rest = &rest[end..];
                let after = rest.trim_start();
                if after.starts_with('=') && !after.starts_with("==") {
                    let value = after[1..].trim_start();
                    if let Some(inner) = value.strip_prefix('{') {
                        if stack.len() < MAX_TABLE_DEPTH {
                            stack.push(Some(ident));
                        } else {
                            deep += 1;
                        }
                        rest = inner;
                    } else {
                        let named: Vec<&str> = stack.iter().flatten().copied().collect();
                        if ident == leaf && named.ends_with(&sections) {
                            return Some(lineno);
                        }
                        rest = value;
                    }
                }
                continue;
            }
            match c {
                '{' if stack.len() < MAX_TABLE_DEPTH => stack.push(None),
                '{' => deep += 1,
                '}' if deep > 0 => deep -= 1,
                '}' => {
                    stack.pop();
                }
                _ => {}
            }
            rest = &rest[c.len_utf8()..];
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_dotted_path() {
        let text = "-- mine\nlocal x = 1\nhl.config({ general.gaps_in = 2 })\n";
        assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(3));
        let text = "x = decoration.blur.enabled\n";
        assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(1));
        assert_eq!(
            find_in_user_lua("general.gaps_inner = 1", "general:gaps_in"),
            None
        );
    }

    #[test]
    fn finds_colon_path() {
        let text = "\n\nhl.keyword(\"general:gaps_out\", 9)\n";
        assert_eq!(find_in_user_lua(text, "general:gaps_out"), Some(3));
        assert_eq!(find_in_user_lua(text, "general:gaps_in"), None);
    }

    #[test]
    fn finds_leaf_inside_section_table() {
        let text = "hl.config({\n    general = {\n        gaps_in = 2,\n    },\n})\n";
        assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(3));
        let text = "hl.config({\n  decoration = {\n    rounding = 4,\n    blur = { size = 3, enabled = false },\n  },\n})\n";
        assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(4));
        assert_eq!(find_in_user_lua(text, "decoration:rounding"), Some(3));
        let one = "hl.config({ general = { layout = \"master\" } })";
        assert_eq!(find_in_user_lua(one, "general:layout"), Some(1));
    }

    #[test]
    fn ignores_leaf_in_other_table() {
        let text = "hl.config({\n  decoration = {\n    gaps_in = 2,\n  },\n  general = { border_size = 1 },\n  gaps_in = 3,\n})\n";
        assert_eq!(find_in_user_lua(text, "general:gaps_in"), None);
        // `blur.enabled` is not `decoration:shadow:enabled`.
        let text = "decoration = {\n  blur = { enabled = true },\n  shadow = {\n    enabled = false,\n  },\n}\n";
        assert_eq!(find_in_user_lua(text, "decoration:shadow:enabled"), Some(4));
        assert_eq!(find_in_user_lua(text, "decoration:blur:enabled"), Some(2));
        assert_eq!(find_in_user_lua("gaps_in == 2", "general:gaps_in"), None);
    }

    #[test]
    fn commented_lines_are_ignored() {
        let text = "-- general.gaps_in = 3 (old)\n-- hl.config({ general = { gaps_in = 1 } })\n\
            general = { -- gaps_in = 4\n  border_size = 1, -- general:gaps_in\n  gaps_in = 2,\n}\n";
        assert_eq!(find_in_user_lua(text, "general:gaps_in"), Some(5));
        let only = "--decoration = { blur = { enabled = false } }\n";
        assert_eq!(find_in_user_lua(only, "decoration:blur:enabled"), None);
    }

    #[test]
    fn huge_and_non_utf8_input_is_safe() {
        // 1M nested braces open and close inside `general`, then `gaps_in` sits outside it.
        let bytes: Vec<u8> = b"general = {\n\xff\xfe gaps\xc3 = 1\n"
            .iter()
            .copied()
            .chain(std::iter::repeat_n(b'{', 1 << 20))
            .chain(std::iter::repeat_n(b'}', 1 << 20))
            .chain(b"}\n  gaps_in = 2\n".iter().copied())
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(find_in_user_lua(&text, "general:gaps_in"), None);
        let more = format!("{text}general = {{ gaps_in = 1 }}\n");
        assert_eq!(find_in_user_lua(&more, "general:gaps_in"), Some(5));
        let wide = "é".repeat(1 << 20) + "general:gaps_in";
        assert_eq!(find_in_user_lua(&wide, "general:gaps_in"), Some(1));
        assert_eq!(find_in_user_lua("anything", "nocolon"), None);
    }

    #[test]
    fn every_option_key_exists_in_state_json() {
        let mut st = State::default();
        st.set_section(
            "look",
            json!({"gaps_in": 1, "gaps_out": 1, "border_width": 1, "rounding": 1, "blur": true,
                "blur_size": 2, "blur_passes": 2, "shadow": true, "animations": true,
                "animation_speed": 1.0}),
        )
        .unwrap();
        st.set_section(
            "layouts",
            json!({"default": "master", "dwindle": {"preserve_split": true, "smart_split": true,
                "force_split": "left"}, "master": {"mfact": 0.5, "new_status": "master",
                "orientation": "top"}, "scrolling": {"column_width": 0.5,
                "fullscreen_on_one_column": true}}),
        )
        .unwrap();
        st.set_section(
            "appearance",
            json!({"ui_font": "A", "mono_font": "B", "font_size": 10, "cursor_theme": "C",
                "cursor_size": 24, "icon_theme": "D"}),
        )
        .unwrap();
        let mut keys = std::collections::HashSet::new();
        for row in OPTIONS {
            assert!(keys.insert(row.key), "duplicate {}", row.key);
            let section = st.section(row.domain()).unwrap();
            assert!(
                !row.stored(&section).is_null(),
                "{} not in {section}",
                row.key
            );
            assert_eq!(
                row.hypr.is_some() as u8 + row.gsettings.is_some() as u8,
                u8::from(row.key != "look.animation_speed"),
                "{} reads back from exactly one place",
                row.key
            );
            assert!(!row.setting_id.is_empty() && !row.label.is_empty());
        }
        for f in crate::settings::Look::FIELDS {
            assert!(keys.contains(format!("look.{f}").as_str()), "look.{f}");
        }
    }

    #[test]
    fn labels_name_rows() {
        assert_eq!(label_for("look", "gaps_in"), Some("Gaps between windows"));
        assert_eq!(
            label_for("layouts", "dwindle.preserve_split"),
            Some("Dwindle: keep the split")
        );
        assert_eq!(label_for("look", "nope"), None);
        assert_eq!(label_for("wallpaper", "gaps_in"), None);
        let line =
            crate::history::summarize("look", &json!({}), &json!({"gaps_in": 3}), &|d, k| {
                label_for(d, k).map(Into::into)
            });
        assert!(
            line.starts_with("Gaps between windows: default → 3"),
            "{line}"
        );
    }

    #[test]
    fn live_values_compare_across_forms() {
        assert!(live_matches(&json!(5), &json!("5 5 5 5")));
        assert!(!live_matches(&json!(5), &json!("5 2 5 5")));
        assert!(!live_matches(&json!(5), &json!("")));
        assert!(live_matches(&json!(5), &json!(5)));
        assert!(live_matches(&json!(0.55), &json!(0.550000)));
        assert!(live_matches(&json!(true), &json!(1)));
        assert!(live_matches(&json!(false), &json!(false)));
        assert!(!live_matches(&json!(true), &json!(0)));
        assert!(live_matches(&json!("dwindle"), &json!("dwindle")));
        assert!(live_matches(&json!(24), &json!("24")));
        assert!(!live_matches(&json!("Inter 11"), &json!("Inter 12")));
    }

    #[test]
    fn expected_uses_overrides_then_theme() {
        let theme = Theme::from_toml(
            crate::theme::tests::FIXTURE,
            "fixture",
            crate::theme::Variant::Dark,
        )
        .unwrap();
        let mut st = State::default();
        let row = |k: &str| OPTIONS.iter().find(|r| r.key == k).unwrap();
        assert_eq!(expected(row("look.gaps_in"), &st, &theme), Some(json!(5)));
        assert_eq!(expected(row("look.blur"), &st, &theme), None);
        assert_eq!(
            expected(row("appearance.ui_font"), &st, &theme),
            Some(json!("Inter 11"))
        );
        st.set_section("look", json!({"gaps_in": 9, "blur": false}))
            .unwrap();
        st.set_section("appearance", json!({"font_size": 14}))
            .unwrap();
        st.set_section("layouts", json!({"dwindle": {"force_split": "right"}}))
            .unwrap();
        assert_eq!(expected(row("look.gaps_in"), &st, &theme), Some(json!(9)));
        assert_eq!(expected(row("look.blur"), &st, &theme), Some(json!(false)));
        assert_eq!(
            expected(row("appearance.font_size"), &st, &theme),
            Some(json!("Inter 14"))
        );
        assert_eq!(
            expected(row("layouts.dwindle.force_split"), &st, &theme),
            Some(json!(2))
        );
    }
}
