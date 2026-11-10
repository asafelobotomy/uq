//! The `wallpaper` section: per-output (or `*`) image, folder slideshow and colour.

use super::{SettingError, check_known, invalid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, PathBuf};

const MAX_OUTPUTS: usize = 16;
const MAX_PATH_BYTES: usize = 4096;

/// An unknown value (from a newer uqd) loads as `Fill`; the API rejects it via [`Fit::NAMES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Fill,
    Fit,
    Center,
    Tile,
}

impl Fit {
    pub const NAMES: [&str; 4] = ["fill", "fit", "center", "tile"];
}

impl<'de> Deserialize<'de> for Fit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "fit" => Fit::Fit,
            "center" => Fit::Center,
            "tile" => Fit::Tile,
            _ => Fit::Fill,
        })
    }
}

/// Unknown keys are ignored on load and rejected by the API (see [`Wallpaper::check_keys`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<PathBuf>,
    pub fit: Fit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    /// Slideshow interval in seconds, 10 to 86400.
    pub interval: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl Default for WallpaperEntry {
    fn default() -> Self {
        Self {
            image: None,
            fit: Fit::Fill,
            folder: None,
            interval: 300,
            color: None,
        }
    }
}

/// `"*"` applies to every display without its own entry.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Wallpaper {
    pub outputs: BTreeMap<String, WallpaperEntry>,
}

fn valid_output(name: &str) -> bool {
    name == "*"
        || ((1..=32).contains(&name.len())
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')))
}

fn check_path(key: &str, p: &Option<PathBuf>) -> Result<(), SettingError> {
    let Some(p) = p else { return Ok(()) };
    let Some(s) = p.to_str() else {
        return Err(invalid(key, "must be valid UTF-8"));
    };
    if s.len() > MAX_PATH_BYTES {
        return Err(invalid(key, format!("over {MAX_PATH_BYTES} bytes")));
    }
    if s.chars().any(char::is_control) {
        return Err(invalid(key, "must not contain control characters"));
    }
    if s.starts_with('~') || !p.is_absolute() {
        return Err(invalid(key, "must be an absolute path"));
    }
    if p.components().any(|c| c == Component::ParentDir) {
        return Err(invalid(key, "must not contain .."));
    }
    Ok(())
}

fn check_color(key: &str, c: &Option<String>) -> Result<(), SettingError> {
    match c {
        Some(c)
            if !(c.len() == 7
                && c.starts_with('#')
                && c[1..].bytes().all(|b| b.is_ascii_hexdigit())) =>
        {
            Err(invalid(key, "expected #rrggbb"))
        }
        _ => Ok(()),
    }
}

impl WallpaperEntry {
    pub const FIELDS: [&str; 5] = ["image", "fit", "folder", "interval", "color"];
}

impl Wallpaper {
    /// The API's strictness: unknown keys at either level and unknown `fit` values are errors
    /// (loading a file is lenient instead).
    pub fn check_keys(v: &serde_json::Value) -> Result<(), SettingError> {
        check_known("wallpaper", v, &["outputs"])?;
        let Some(outputs) = v.get("outputs").and_then(|o| o.as_object()) else {
            return Ok(());
        };
        for (name, e) in outputs {
            let key = format!("wallpaper.outputs.{name}");
            check_known(&key, e, &WallpaperEntry::FIELDS)?;
            if let Some(fit) = e.get("fit").filter(|f| !f.is_null())
                && !fit.as_str().is_some_and(|f| Fit::NAMES.contains(&f))
            {
                return Err(invalid(
                    &format!("{key}.fit"),
                    "expected fill, fit, center or tile",
                ));
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), SettingError> {
        if self.outputs.len() > MAX_OUTPUTS {
            return Err(invalid(
                "wallpaper.outputs",
                format!("at most {MAX_OUTPUTS} entries"),
            ));
        }
        for (name, e) in &self.outputs {
            if !valid_output(name) {
                return Err(invalid(
                    "wallpaper.outputs",
                    format!("invalid output name {name:?}"),
                ));
            }
            let key = |f: &str| format!("wallpaper.outputs.{name}.{f}");
            check_path(&key("image"), &e.image)?;
            check_path(&key("folder"), &e.folder)?;
            if !(10..=86400).contains(&e.interval) {
                return Err(invalid(&key("interval"), "must be between 10 and 86400"));
            }
            check_color(&key("color"), &e.color)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use serde_json::json;

    #[test]
    fn rejects_bad_paths() {
        let mut s = Settings::default();
        for bad in ["../a.png", "/a/../b.png", "~/a.png", "a.png", "/a\n.png"] {
            assert!(
                s.set_section("wallpaper", json!({"outputs": {"*": {"image": bad}}}))
                    .is_err(),
                "{bad}"
            );
            assert!(
                s.set_section("wallpaper", json!({"outputs": {"*": {"folder": bad}}}))
                    .is_err(),
                "{bad}"
            );
        }
        let long = format!("/{}", "a".repeat(4096));
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"*": {"image": long}}}))
                .is_err()
        );
        s.set_section(
            "wallpaper",
            json!({"outputs": {"*": {"image": "/home/u/a b.png"}}}),
        )
        .unwrap();
    }

    #[test]
    fn rejects_bad_outputs_and_too_many() {
        let mut s = Settings::default();
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"DP 1": {}}}))
                .is_err()
        );
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"": {}}}))
                .is_err()
        );
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"a".repeat(33): {}}}))
                .is_err()
        );
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"*": {"bogus": 1}}}))
                .is_err()
        );
        assert!(
            s.set_section("wallpaper", json!({"outputs": {}, "bogus": 1}))
                .is_err()
        );
        let ok: serde_json::Map<_, _> = (0..16).map(|i| (format!("DP-{i}"), json!({}))).collect();
        s.set_section("wallpaper", json!({"outputs": ok})).unwrap();
        let many: serde_json::Map<_, _> = (0..17).map(|i| (format!("DP-{i}"), json!({}))).collect();
        assert!(
            s.set_section("wallpaper", json!({"outputs": many}))
                .is_err()
        );
    }

    #[test]
    fn interval_and_color() {
        let mut s = Settings::default();
        for bad in [9, 86401] {
            assert!(
                s.set_section("wallpaper", json!({"outputs": {"*": {"interval": bad}}}))
                    .is_err()
            );
        }
        for bad in ["#12345", "123456", "#12345g", "#1234567"] {
            assert!(
                s.set_section("wallpaper", json!({"outputs": {"*": {"color": bad}}}))
                    .is_err(),
                "{bad}"
            );
        }
        assert!(
            s.set_section("wallpaper", json!({"outputs": {"*": {"fit": "stretch"}}}))
                .is_err()
        );
        s.set_section(
            "wallpaper",
            json!({"outputs": {"*": {"interval": 10, "color": "#aBc012", "fit": "tile"}}}),
        )
        .unwrap();
        let e = &s.wallpaper.outputs["*"];
        assert_eq!((e.interval, e.color.as_deref()), (10, Some("#aBc012")));
        s.set_section("wallpaper", json!({"outputs": {"*": {}}}))
            .unwrap();
        assert_eq!(s.wallpaper.outputs["*"], super::WallpaperEntry::default());
    }
}
