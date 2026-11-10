//! The `appearance` section: font, cursor and icon overrides (`None` = use the theme).

use super::{SettingError, invalid};
use serde::{Deserialize, Serialize};

const MAX_FONT_CHARS: usize = 100;

/// Unknown keys are ignored on load (a newer uqd's state.toml still loads) and rejected by
/// [`super::Settings::set_section`] via [`Appearance::FIELDS`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mono_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_size: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_theme: Option<String>,
    pub kvantum: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            ui_font: None,
            mono_font: None,
            font_size: None,
            cursor_theme: None,
            cursor_size: None,
            icon_theme: None,
            kvantum: true,
        }
    }
}

fn font(key: &str, v: &Option<String>) -> Result<(), SettingError> {
    let Some(s) = v else { return Ok(()) };
    let n = s.chars().count();
    if !(1..=MAX_FONT_CHARS).contains(&n) {
        return Err(invalid(
            key,
            format!("must be 1 to {MAX_FONT_CHARS} characters"),
        ));
    }
    if s.chars().any(char::is_control) {
        return Err(invalid(key, "must not contain control characters"));
    }
    Ok(())
}

/// A cursor or icon theme name uqd accepts: `^[A-Za-z0-9._+ -]{1,64}$`, and not `.` or `..`.
pub fn valid_theme_name(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b' ' | b'-'))
        && s != "."
        && s != ".."
}

fn theme_name(key: &str, v: &Option<String>) -> Result<(), SettingError> {
    let Some(s) = v else { return Ok(()) };
    if valid_theme_name(s) {
        Ok(())
    } else {
        Err(invalid(
            key,
            "must match ^[A-Za-z0-9._+ -]{1,64}$ and not be . or ..",
        ))
    }
}

fn ranged(key: &str, v: Option<u8>, lo: u8, hi: u8) -> Result<(), SettingError> {
    match v {
        Some(n) if !(lo..=hi).contains(&n) => {
            Err(invalid(key, format!("must be between {lo} and {hi}")))
        }
        _ => Ok(()),
    }
}

impl Appearance {
    /// Every key the API accepts.
    pub const FIELDS: [&str; 7] = [
        "ui_font",
        "mono_font",
        "font_size",
        "cursor_theme",
        "cursor_size",
        "icon_theme",
        "kvantum",
    ];

    pub fn validate(&self) -> Result<(), SettingError> {
        font("appearance.ui_font", &self.ui_font)?;
        font("appearance.mono_font", &self.mono_font)?;
        ranged("appearance.font_size", self.font_size, 6, 32)?;
        theme_name("appearance.cursor_theme", &self.cursor_theme)?;
        ranged("appearance.cursor_size", self.cursor_size, 16, 96)?;
        theme_name("appearance.icon_theme", &self.icon_theme)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_unset_and_kvantum_on() {
        let a = Appearance::default();
        assert_eq!(
            (
                a.ui_font,
                a.mono_font,
                a.font_size,
                a.cursor_theme,
                a.cursor_size,
                a.icon_theme
            ),
            (None, None, None, None, None, None)
        );
        assert!(a.kvantum);
        let a: Appearance = serde_json::from_value(json!({})).unwrap();
        assert_eq!(a, Appearance::default());
    }

    #[test]
    fn rejects_bad_names() {
        let mut s = Settings::default();
        for bad in ["..", ".", "", "a/b", "caf\u{e9}", "x\ny", &"a".repeat(65)] {
            assert!(
                s.set_section("appearance", json!({"cursor_theme": bad}))
                    .is_err(),
                "{bad:?}"
            );
            assert!(
                s.set_section("appearance", json!({"icon_theme": bad}))
                    .is_err(),
                "{bad:?}"
            );
        }
        assert!(
            s.set_section("appearance", json!({"ui_font": "Inter\nX"}))
                .is_err()
        );
        assert!(s.set_section("appearance", json!({"ui_font": ""})).is_err());
        assert!(
            s.set_section("appearance", json!({"mono_font": "a".repeat(101)}))
                .is_err()
        );
        assert!(s.set_section("appearance", json!({"bogus": 1})).is_err());
        s.set_section(
            "appearance",
            json!({"cursor_theme": "Bibata-Modern_Ice+1.x", "ui_font": "Noto Sans CJK"}),
        )
        .unwrap();
        assert_eq!(
            s.appearance.cursor_theme.as_deref(),
            Some("Bibata-Modern_Ice+1.x")
        );
    }

    #[test]
    fn font_and_cursor_ranges() {
        let mut s = Settings::default();
        for (key, bad) in [
            ("font_size", 5),
            ("font_size", 33),
            ("cursor_size", 15),
            ("cursor_size", 97),
        ] {
            assert!(
                s.set_section("appearance", json!({ key: bad })).is_err(),
                "{key} {bad}"
            );
        }
        assert!(
            s.set_section("appearance", json!({"font_size": 300}))
                .is_err()
        );
        assert!(
            s.set_section("appearance", json!({"font_size": -1}))
                .is_err()
        );
        s.set_section("appearance", json!({"font_size": 6, "cursor_size": 96}))
            .unwrap();
        s.set_section("appearance", json!({"font_size": 32, "cursor_size": 16}))
            .unwrap();
        assert_eq!(
            (s.appearance.font_size, s.appearance.cursor_size),
            (Some(32), Some(16))
        );
    }
}
