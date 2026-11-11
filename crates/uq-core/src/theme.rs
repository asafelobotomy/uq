//! Theme model: semantic tokens loaded from `<themes_dir>/<id>/<variant>.toml`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::settings::Appearance;

pub mod derive;
pub mod uqtheme;
pub mod user;

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
    #[serde(default)]
    author: Option<String>,
    colors: Colors,
    ui: Ui,
    fonts: Fonts,
    icons: Icons,
    terminal: Terminal,
}

impl Theme {
    /// Parse a theme file. `id` and `variant` come from the file's path, not its contents.
    pub fn from_toml(src: &str, id: &str, variant: Variant) -> Result<Theme, ThemeError> {
        Self::from_toml_with_author(src, id, variant).map(|(t, _)| t)
    }

    /// [`Theme::from_toml`], and the file's optional `author` (which `Theme` doesn't hold, so
    /// `tokens.json` stays as it was).
    pub fn from_toml_with_author(
        src: &str,
        id: &str,
        variant: Variant,
    ) -> Result<(Theme, Option<String>), ThemeError> {
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
        let theme = Theme {
            id: id.to_string(),
            name: f.name,
            variant,
            colors: f.colors,
            ui: f.ui,
            fonts: f.fonts,
            icons: f.icons,
            terminal: f.terminal,
        };
        Ok((theme, f.author))
    }

    /// A copy with the user's appearance overrides applied; each `Some` replaces the theme's value.
    pub fn with_appearance(&self, a: &Appearance) -> Theme {
        let mut t = self.clone();
        if let Some(v) = &a.ui_font {
            t.fonts.ui = v.clone();
        }
        if let Some(v) = &a.mono_font {
            t.fonts.mono = v.clone();
        }
        if let Some(v) = a.font_size {
            t.fonts.size = v;
        }
        if let Some(v) = &a.cursor_theme {
            t.icons.cursor_theme = v.clone();
        }
        if let Some(v) = a.cursor_size {
            t.icons.cursor_size = v;
        }
        if let Some(v) = &a.icon_theme {
            t.icons.theme = v.clone();
        }
        t
    }
}

/// What the theme list shows about one theme. `Theme` itself carries none of this, because it is
/// written whole into `tokens.json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    /// Made by the user (or imported), not shipped with uQ.
    pub user: bool,
    /// The theme is in use but its files are gone; uQ keeps the last good copy until another
    /// theme is picked.
    pub missing: bool,
}

/// All themes found under a directory, each guaranteed to have both variants.
#[derive(Debug, Clone, Default)]
pub struct ThemeRegistry {
    themes: BTreeMap<(String, Variant), Theme>,
    info: BTreeMap<String, ThemeInfo>,
}

impl ThemeRegistry {
    /// Load the system themes. Packages are trusted, so any bad theme fails the load. A folder
    /// named like a user theme is skipped: user themes come from [`user::scan`].
    pub fn load_dir(dir: &Path) -> Result<Self, ThemeError> {
        let io = |e: std::io::Error| ThemeError::Io(format!("{}: {e}", dir.display()));
        let mut themes = BTreeMap::new();
        let mut info = BTreeMap::new();
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
            if id.starts_with(user::PREFIX) {
                tracing::warn!("skipping system theme folder {id}: the user- prefix is reserved");
                continue;
            }
            for variant in Variant::ALL {
                let path = entry.path().join(format!("{variant}.toml"));
                let src = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(ThemeError::MissingVariant { id, variant });
                    }
                    Err(e) => return Err(io(e)),
                };
                let (theme, author) =
                    Theme::from_toml_with_author(&src, &id, variant).map_err(|e| match e {
                        ThemeError::Parse { message, .. } => ThemeError::Parse {
                            path: path.clone(),
                            message,
                        },
                        other => other,
                    })?;
                info.entry(id.clone()).or_insert_with(|| ThemeInfo {
                    id: id.clone(),
                    name: theme.name.clone(),
                    author,
                    user: false,
                    missing: false,
                });
                themes.insert((id.clone(), variant), theme);
            }
        }
        Ok(ThemeRegistry { themes, info })
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

    pub fn info(&self, id: &str) -> Option<&ThemeInfo> {
        self.info.get(id)
    }

    /// System themes first, then the user's, each by name (ignoring case).
    pub fn infos(&self) -> Vec<&ThemeInfo> {
        let mut v: Vec<&ThemeInfo> = self.info.values().collect();
        v.sort_by_cached_key(|i| (i.user, i.name.to_lowercase(), i.id.clone()));
        v
    }

    pub fn is_user(&self, id: &str) -> bool {
        self.info.get(id).is_some_and(|i| i.user)
    }

    /// Swap in a fresh scan of the user's themes. If `keep` names a user theme the scan no
    /// longer has (its folder was removed behind uqd's back), the old copy stays, marked
    /// `missing`, so the active theme can still be rendered.
    pub fn replace_user(&mut self, scan: user::UserScan, keep: Option<&str>) {
        let old_info = self.info.clone();
        let old_themes: Vec<_> = self
            .themes
            .iter()
            .filter(|((id, _), _)| old_info.get(id).is_some_and(|i| i.user))
            .map(|(k, t)| (k.clone(), t.clone()))
            .collect();
        self.themes
            .retain(|(id, _), _| !old_info.get(id).is_some_and(|i| i.user));
        self.info.retain(|_, i| !i.user);
        for u in scan.themes {
            let id = u.dark.id.clone();
            self.info.insert(
                id.clone(),
                ThemeInfo {
                    id: id.clone(),
                    name: u.dark.name.clone(),
                    author: u.author,
                    user: true,
                    missing: false,
                },
            );
            self.themes.insert((id.clone(), Variant::Dark), u.dark);
            self.themes.insert((id, Variant::Light), u.light);
        }
        if let Some(k) = keep
            && !self.info.contains_key(k)
            && let Some(mut i) = old_info.get(k).filter(|i| i.user).cloned()
        {
            i.missing = true;
            self.info.insert(k.to_string(), i);
            for (key, t) in old_themes {
                if key.0 == k {
                    self.themes.insert(key, t);
                }
            }
        }
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
    fn with_appearance_overrides_only_set_fields() {
        let t = Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap();
        assert_eq!(t.with_appearance(&Appearance::default()), t);
        let a = Appearance {
            mono_font: Some("Hack".into()),
            cursor_size: Some(32),
            icon_theme: Some("Adwaita".into()),
            ..Appearance::default()
        };
        let o = t.with_appearance(&a);
        assert_eq!(o.fonts.mono, "Hack");
        assert_eq!(o.fonts.ui, "Inter");
        assert_eq!(o.fonts.size, 11);
        assert_eq!(o.icons.cursor_size, 32);
        assert_eq!(o.icons.cursor_theme, "Bibata-Modern-Classic");
        assert_eq!(o.icons.theme, "Adwaita");
        let all = Appearance {
            ui_font: Some("Cantarell".into()),
            font_size: Some(14),
            cursor_theme: Some("Breeze".into()),
            ..Appearance::default()
        };
        let o = t.with_appearance(&all);
        assert_eq!(
            (
                o.fonts.ui.as_str(),
                o.fonts.size,
                o.icons.cursor_theme.as_str()
            ),
            ("Cantarell", 14, "Breeze")
        );
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

    #[test]
    fn author_is_optional_in_theme_files() {
        let with = format!("author = \"Sam\"\n{FIXTURE}");
        let (t, a) = Theme::from_toml_with_author(&with, "f", Variant::Dark).unwrap();
        assert_eq!(a.as_deref(), Some("Sam"));
        assert_eq!(t.name, "Fixture");
        let (_, a) = Theme::from_toml_with_author(FIXTURE, "f", Variant::Dark).unwrap();
        assert_eq!(a, None);
        // `Theme` itself gains no field: tokens.json is its whole serialisation.
        let json = serde_json::to_value(&t).unwrap();
        assert!(json.get("author").is_none());
    }

    #[test]
    fn load_dir_skips_user_prefixed_system_folders() {
        let dir = tempfile::tempdir().unwrap();
        for id in ["solo", "user-sneaky"] {
            fs::create_dir(dir.path().join(id)).unwrap();
            fs::write(dir.path().join(id).join("dark.toml"), FIXTURE).unwrap();
            fs::write(dir.path().join(id).join("light.toml"), FIXTURE).unwrap();
        }
        let reg = ThemeRegistry::load_dir(dir.path()).unwrap();
        assert_eq!(reg.ids(), vec!["solo"]);
        assert!(reg.info("user-sneaky").is_none());
    }

    fn user_scan(names: &[&str]) -> (tempfile::TempDir, user::UserScan) {
        let dir = tempfile::tempdir().unwrap();
        for n in names {
            let id = format!("user-{}", n.to_lowercase());
            fs::create_dir(dir.path().join(&id)).unwrap();
            for v in ["dark", "light"] {
                let text = if v == "dark" {
                    FIXTURE.replacen("Fixture", n, 1)
                } else {
                    fs::read_to_string(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../themes/gruvbox/light.toml"),
                    )
                    .unwrap()
                    .replacen("Gruvbox", n, 1)
                };
                fs::write(dir.path().join(&id).join(format!("{v}.toml")), text).unwrap();
            }
        }
        let scan = user::scan(dir.path());
        (dir, scan)
    }

    #[test]
    fn registry_infos_mark_user_and_missing() {
        let sys = tempfile::tempdir().unwrap();
        for (id, name) in [("zeta", "Zeta"), ("alpha", "alpha")] {
            fs::create_dir(sys.path().join(id)).unwrap();
            let text = FIXTURE.replacen("Fixture", name, 1);
            fs::write(sys.path().join(id).join("dark.toml"), &text).unwrap();
            fs::write(sys.path().join(id).join("light.toml"), &text).unwrap();
        }
        let mut reg = ThemeRegistry::load_dir(sys.path()).unwrap();
        let (_keep, scan) = user_scan(&["Beta", "Apple"]);
        assert_eq!(scan.themes.len(), 2, "{:?}", scan.skipped);
        reg.replace_user(scan, None);
        let order: Vec<_> = reg.infos().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(order, ["alpha", "zeta", "user-apple", "user-beta"]);
        assert!(reg.is_user("user-beta") && !reg.is_user("zeta"));
        assert!(reg.get("user-beta", Variant::Light).is_some());
        assert!(!reg.info("user-beta").unwrap().missing);
        assert_eq!(reg.ids().len(), 4);

        // The folder vanishes: the active theme's last copy stays, marked missing.
        reg.replace_user(user::UserScan::default(), Some("user-beta"));
        let i = reg.info("user-beta").unwrap();
        assert!(i.missing && i.user);
        assert!(reg.get("user-beta", Variant::Dark).is_some());
        assert!(reg.get("user-beta", Variant::Light).is_some());
        assert!(reg.info("user-apple").is_none());
        assert!(reg.get("user-apple", Variant::Dark).is_none());
        // Keeping a system id or an unknown id changes nothing.
        reg.replace_user(user::UserScan::default(), Some("zeta"));
        assert!(reg.info("user-beta").is_none());
        assert_eq!(reg.ids(), ["alpha", "zeta"]);
        // A scan that has it again clears `missing`.
        let (_k2, scan) = user_scan(&["Beta"]);
        reg.replace_user(scan, Some("user-beta"));
        assert!(!reg.info("user-beta").unwrap().missing);
    }
}
