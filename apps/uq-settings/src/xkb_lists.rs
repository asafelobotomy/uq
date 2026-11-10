//! The keyboard layouts, variants and layout-switch keys the Input page offers, read from
//! xkeyboard-config's `evdev.lst`. A missing file gives empty lists; the page then takes a plain
//! layout code.

use std::collections::BTreeMap;

use serde::Serialize;

const EVDEV_LST: &str = "/usr/share/X11/xkb/rules/evdev.lst";
const MAX_NAME_CHARS: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct XkbLists {
    /// `(code, name)`, sorted by name.
    pub layouts: Vec<(String, String)>,
    /// Per layout code, its variants as `(code, name)`.
    pub variants: BTreeMap<String, Vec<(String, String)>>,
    /// `grp:` options that switch layouts (`…_toggle` or `…_switch`), as `(code, name)`.
    pub switch_options: Vec<(String, String)>,
}

/// Names come from a system file: no control characters, at most 200 characters.
fn clean(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect()
}

/// The sections of an `evdev.lst`.
pub fn parse(src: &str) -> XkbLists {
    let mut out = XkbLists::default();
    let mut section = "";
    for line in src.lines() {
        if let Some(name) = line.strip_prefix('!') {
            section = match name.trim() {
                "layout" => "layout",
                "variant" => "variant",
                "option" => "option",
                _ => "",
            };
            continue;
        }
        let line = line.trim();
        let Some((code, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let (code, rest) = (code.trim(), rest.trim());
        match section {
            "layout" => out.layouts.push((clean(code), clean(rest))),
            "variant" => {
                if let Some((layout, name)) = rest.split_once(':') {
                    out.variants
                        .entry(clean(layout))
                        .or_default()
                        .push((clean(code), clean(name)));
                }
            }
            "option"
                if code.starts_with("grp:")
                    && (code.ends_with("_toggle")
                        || code.ends_with("_switch")
                        || code == "grp:toggle"
                        || code == "grp:switch") =>
            {
                out.switch_options.push((clean(code), clean(rest)));
            }
            _ => {}
        }
    }
    out.layouts.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// The system's lists; empty when xkeyboard-config isn't installed.
pub fn load() -> XkbLists {
    std::fs::read_to_string(EVDEV_LST)
        .map(|s| parse(&s))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/evdev.lst");

    #[test]
    fn parses_fixture() {
        let l = parse(FIXTURE);
        let codes: Vec<_> = l.layouts.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(codes, ["gb", "us", "fr", "de"], "sorted by name");
        assert_eq!(l.layouts[1].1, "English (US)");
        assert_eq!(
            l.variants["us"][1],
            ("intl".into(), "English (US, intl., with dead keys)".into())
        );
        assert_eq!(
            l.variants["de"],
            [("nodeadkeys".into(), "German (no dead keys)".into())]
        );
        assert!(!l.variants.contains_key("gb"));
    }

    #[test]
    fn switch_options_only_grp_toggles() {
        let l = parse(FIXTURE);
        let codes: Vec<_> = l.switch_options.iter().map(|(c, _)| c.as_str()).collect();
        assert!(codes.contains(&"grp:alt_shift_toggle") && codes.contains(&"grp:win_switch"));
        assert!(codes.iter().all(|c| c.starts_with("grp:")));
        assert!(
            !codes.contains(&"ctrl:nocaps")
                && !codes.contains(&"grp")
                && !codes.contains(&"grp_led")
        );
        assert!(
            codes
                .iter()
                .all(|c| uq_core::settings::valid_switch_combo(c)),
            "{codes:?}"
        );
    }

    #[test]
    fn names_sanitised() {
        let long = "x".repeat(500);
        let src = format!("! layout\n  zz   Bad\u{7}Name\n  yy   {long}\n! nonsense\n  q  w\n");
        let l = parse(&src);
        assert_eq!(l.layouts[0], ("zz".into(), "BadName".into()));
        assert_eq!(l.layouts[1].1.chars().count(), 200);
        assert_eq!(l.layouts.len(), 2);
        assert_eq!(parse(""), XkbLists::default());
    }
}
