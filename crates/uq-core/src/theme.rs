//! Theme model: semantic tokens loaded from `<themes_dir>/<id>/<variant>.toml`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("bad colour {0:?} (expected #rrggbb)")]
    BadColor(String),
    #[error("{path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("theme {id:?} has no {variant} variant")]
    MissingVariant { id: String, variant: Variant },
    #[error("unknown variant {0:?} (expected dark or light)")]
    BadVariant(String),
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Dark,
    Light,
}

impl Variant {
    pub const ALL: [Variant; 2] = [Variant::Dark, Variant::Light];
    pub fn as_str(self) -> &'static str {
        match self {
            Variant::Dark => "dark",
            Variant::Light => "light",
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Variant {
    type Err = ThemeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dark" => Ok(Variant::Dark),
            "light" => Ok(Variant::Light),
            other => Err(ThemeError::BadVariant(other.to_string())),
        }
    }
}

/// An sRGB colour, written `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub [u8; 3]);

impl Color {
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
    /// Hyprland colour form: `rgba(rrggbbaa)`.
    pub fn rgba(&self, alpha: u8) -> String {
        format!(
            "rgba({:02x}{:02x}{:02x}{alpha:02x})",
            self.0[0], self.0[1], self.0[2]
        )
    }
}

impl FromStr for Color {
    type Err = ThemeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || ThemeError::BadColor(s.to_string());
        let h = s.strip_prefix('#').ok_or_else(bad)?;
        if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
        Ok(Color([p(0)?, p(2)?, p(4)?]))
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub overlay: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    pub radius: u8,
    pub gap_in: u8,
    pub gap_out: u8,
    pub border_width: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    pub ui: String,
    pub mono: String,
    pub size: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Icons {
    pub theme: String,
    pub cursor_theme: String,
    pub cursor_size: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    pub ansi: [Color; 16],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub variant: Variant,
    pub colors: Colors,
    pub ui: Ui,
    pub fonts: Fonts,
    pub icons: Icons,
    pub terminal: Terminal,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    colors: Colors,
    ui: Ui,
    fonts: Fonts,
    icons: Icons,
    terminal: Terminal,
}

impl Theme {
    /// Parse a theme file. `id` and `variant` come from the file's path, not its contents.
    pub fn from_toml(src: &str, id: &str, variant: Variant) -> Result<Theme, ThemeError> {
        let f: ThemeFile = toml::from_str(src).map_err(|e| {
            // Surface colour errors as BadColor so callers can match on them.
            let msg = e.message().to_string();
            if msg.starts_with("bad colour") {
                ThemeError::BadColor(msg)
            } else {
                ThemeError::Parse {
                    path: PathBuf::from(format!("{id}/{variant}.toml")),
                    message: e.to_string(),
                }
            }
        })?;
        Ok(Theme {
            id: id.to_string(),
            name: f.name,
            variant,
            colors: f.colors,
            ui: f.ui,
            fonts: f.fonts,
            icons: f.icons,
            terminal: f.terminal,
        })
    }
}

/// All themes found under a directory, each guaranteed to have both variants.
#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry {
    themes: BTreeMap<(String, Variant), Theme>,
}

impl ThemeRegistry {
    pub fn load_dir(dir: &Path) -> Result<Self, ThemeError> {
        let io = |e: std::io::Error| ThemeError::Io(format!("{}: {e}", dir.display()));
        let mut themes = BTreeMap::new();
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(io)?
            .collect::<Result<_, _>>()
            .map_err(io)?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if !entry.file_type().map_err(io)?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            for variant in Variant::ALL {
                let path = entry.path().join(format!("{variant}.toml"));
                let src = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(ThemeError::MissingVariant { id, variant });
                    }
                    Err(e) => return Err(io(e)),
                };
                let theme = Theme::from_toml(&src, &id, variant).map_err(|e| match e {
                    ThemeError::Parse { message, .. } => ThemeError::Parse {
                        path: path.clone(),
                        message,
                    },
                    other => other,
                })?;
                themes.insert((id.clone(), variant), theme);
            }
        }
        Ok(ThemeRegistry { themes })
    }

    pub fn get(&self, id: &str, v: Variant) -> Option<&Theme> {
        self.themes.get(&(id.to_string(), v))
    }

    pub fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.themes.keys().map(|(id, _)| id.as_str()).collect();
        ids.dedup();
        ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &Theme> {
        self.themes.values()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    pub(crate) const FIXTURE: &str = r##"
name = "Fixture"

[colors]
bg = "#282828"
surface = "#32302f"
surface_alt = "#3c3836"
overlay = "#504945"
text = "#ebdbb2"
text_muted = "#bdae93"
accent = "#d79921"
accent_text = "#282828"
border = "#665c54"
success = "#98971a"
warning = "#d79921"
error = "#cc241d"

[ui]
radius = 10
gap_in = 5
gap_out = 12
border_width = 2

[fonts]
ui = "Inter"
mono = "JetBrains Mono"
size = 11

[icons]
theme = "Papirus-Dark"
cursor_theme = "Bibata-Modern-Classic"
cursor_size = 24

[terminal]
ansi = ["#282828","#cc241d","#98971a","#d79921","#458588","#b16286","#689d6a","#a89984",
        "#928374","#fb4934","#b8bb26","#fabd2f","#83a598","#d3869b","#8ec07c","#ebdbb2"]
"##;

    #[test]
    fn parses_minimal_theme() {
        let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
        assert_eq!(t.colors.bg, Color([0x28, 0x28, 0x28]));
        assert_eq!(t.variant, Variant::Dark);
        assert_eq!(t.id, "fixture");
        assert_eq!(t.name, "Fixture");
        assert_eq!(t.terminal.ansi[15], Color([0xeb, 0xdb, 0xb2]));
    }

    #[test]
    fn rejects_bad_hex() {
        let src = FIXTURE.replace("bg = \"#282828\"", "bg = \"#12345\"");
        let err = Theme::from_toml(&src, "fixture", Variant::Dark).unwrap_err();
        assert!(matches!(err, ThemeError::BadColor(_)), "got {err:?}");
    }

    #[test]
    fn rgba_format() {
        assert_eq!(Color([0x33, 0xcc, 0xff]).rgba(0xee), "rgba(33ccffee)");
        assert_eq!(Color([0x33, 0xcc, 0xff]).hex(), "#33ccff");
    }

    #[test]
    fn variant_round_trip() {
        assert_eq!("light".parse::<Variant>().unwrap(), Variant::Light);
        assert_eq!(Variant::Dark.to_string(), "dark");
        assert!("dim".parse::<Variant>().is_err());
    }

    #[test]
    fn registry_rejects_missing_variant() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("solo")).unwrap();
        fs::write(dir.path().join("solo/dark.toml"), FIXTURE).unwrap();
        let err = ThemeRegistry::load_dir(dir.path()).unwrap_err();
        assert!(
            matches!(&err, ThemeError::MissingVariant { id, variant: Variant::Light } if id == "solo"),
            "got {err:?}"
        );
    }

    #[test]
    fn registry_loads_and_gets() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("solo")).unwrap();
        fs::write(dir.path().join("solo/dark.toml"), FIXTURE).unwrap();
        fs::write(dir.path().join("solo/light.toml"), FIXTURE).unwrap();
        let reg = ThemeRegistry::load_dir(dir.path()).unwrap();
        assert_eq!(reg.ids(), vec!["solo"]);
        assert_eq!(
            reg.get("solo", Variant::Light).unwrap().variant,
            Variant::Light
        );
        assert!(reg.get("nope", Variant::Dark).is_none());
    }
}
