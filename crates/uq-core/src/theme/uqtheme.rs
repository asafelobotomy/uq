//! The `.uqtheme` file: one theme, both variants, as TOML that is data and nothing else.
//!
//! [`parse`] is the one gate every theme file passes (size, syntax, format, the exact set of keys,
//! text, ranges, contrast, and the optional window look). [`to_text`] writes the same shape back
//! deterministically, so an export read in again is equal and a second export is byte-identical.

use super::{Colors, Fonts, Icons, Terminal, Theme, Ui, Variant, user};
use crate::render::RenderedFiles;
use crate::settings::{Look, SettingError, Settings};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub const FORMAT: i64 = 1;
pub const MAX_FILE_BYTES: u64 = 65_536;
pub const EXTENSION: &str = "uqtheme";
pub const HEADER: &str = "# A uQ theme. Import it on the Appearance page of uQ Settings, or with\n# `uqctl theme import <file>`.\n";

/// The most characters of a refused key an error shows.
const MAX_KEY_CHARS: usize = 40;
const TOP_KEYS: [&str; 7] = ["format", "id", "name", "author", "dark", "light", "look"];

/// A theme as the file holds it. `dark.id` and `light.id` are `user-<slug>` and both variants
/// carry `name`.
#[derive(Debug, Clone, PartialEq)]
pub struct UqTheme {
    pub slug: String,
    pub name: String,
    pub author: Option<String>,
    pub dark: Theme,
    pub light: Theme,
    pub look: Option<Map<String, Value>>,
}

/// Text from a file with its control characters left out and cut short, so an error can't carry
/// a terminal escape sequence.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_KEY_CHARS)
        .collect()
}

fn variant_text(variant: &Variant, problem: &user::Problem) -> String {
    if problem.variant().is_some() {
        problem.to_string()
    } else {
        format!("The {variant} version: {problem}")
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImportError {
    #[error("The file is too big to be a theme.")]
    TooBig,
    #[error("Can't read the file: {0}")]
    Unreadable(String),
    #[error("This file isn't a uQ theme.")]
    NotATheme,
    #[error("This theme was made by a newer version of uQ.")]
    NewerFormat(i64),
    #[error("The theme's id can only use a–z, 0–9 and dashes.")]
    BadId,
    #[error("A theme needs a name of 1 to 64 characters, without special characters.")]
    BadName,
    #[error("The author's name can't be used.")]
    BadAuthor,
    #[error("This file has things a uQ theme can't hold: {0}.")]
    Unknown(String),
    #[error("{}", variant_text(.variant, .problem))]
    Variant {
        variant: Variant,
        problem: user::Problem,
    },
    #[error("The window look in this theme can't be used: {0}")]
    BadLook(String),
}

/// Read a theme file the user picked. It may be a link (the user chose it), but it must be a
/// regular file once opened, so a pipe can't stall the read; it is at most [`MAX_FILE_BYTES`]
/// and UTF-8.
pub fn read_file(path: &Path) -> Result<String, ImportError> {
    let unreadable = |e: std::io::Error| ImportError::Unreadable(clean(&e.to_string()));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(crate::autostart::scan::O_NONBLOCK | crate::autostart::scan::O_NOCTTY)
        .open(path)
        .map_err(unreadable)?;
    let meta = file.metadata().map_err(unreadable)?;
    if !meta.is_file() {
        return Err(ImportError::Unreadable("it isn't a file".into()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ImportError::TooBig);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooBig);
    }
    String::from_utf8(bytes).map_err(|_| ImportError::NotATheme)
}

/// One variant's tables, as the file spells them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantBody {
    colors: Colors,
    ui: Ui,
    fonts: Fonts,
    icons: Icons,
    terminal: Terminal,
}

fn text_of(table: &toml::Table, key: &str) -> Option<String> {
    match table.get(key) {
        Some(toml::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn variant_of(table: &toml::Table, v: Variant, id: &str, name: &str) -> Result<Theme, ImportError> {
    let wrong = |problem| ImportError::Variant {
        variant: v,
        problem,
    };
    let Some(toml::Value::Table(t)) = table.get(v.as_str()) else {
        return Err(wrong(if table.contains_key(v.as_str()) {
            user::Problem::BadFile(v)
        } else {
            user::Problem::MissingVariant(v)
        }));
    };
    let body: VariantBody = toml::Value::Table(t.clone())
        .try_into()
        .map_err(|_| wrong(user::Problem::BadFile(v)))?;
    Ok(Theme {
        id: id.to_string(),
        name: name.to_string(),
        variant: v,
        colors: body.colors,
        ui: body.ui,
        fonts: body.fonts,
        icons: body.icons,
        terminal: body.terminal,
    })
}

/// Parse and fully check a `.uqtheme`.
pub fn parse(text: &str) -> Result<UqTheme, ImportError> {
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooBig);
    }
    let table: toml::Table = text.parse().map_err(|_| ImportError::NotATheme)?;
    match table.get("format") {
        Some(toml::Value::Integer(n)) if *n > FORMAT => return Err(ImportError::NewerFormat(*n)),
        Some(toml::Value::Integer(n)) if *n == FORMAT => {}
        _ => return Err(ImportError::NotATheme),
    }
    if let Some(key) = table.keys().find(|k| !TOP_KEYS.contains(&k.as_str())) {
        return Err(ImportError::Unknown(clean(key)));
    }
    let id = text_of(&table, "id").ok_or(ImportError::BadId)?;
    let slug = id.strip_prefix(user::PREFIX).unwrap_or(&id);
    if !user::valid_slug(slug) {
        return Err(ImportError::BadId);
    }
    let slug = slug.to_string();
    let name = text_of(&table, "name")
        .filter(|n| user::valid_display_text(n))
        .ok_or(ImportError::BadName)?;
    let author = match table.get("author") {
        None => None,
        Some(toml::Value::String(a)) if a.is_empty() => None,
        Some(toml::Value::String(a)) if user::valid_display_text(a) => Some(a.clone()),
        Some(_) => return Err(ImportError::BadAuthor),
    };
    let theme_id = format!("{}{slug}", user::PREFIX);
    let dark = variant_of(&table, Variant::Dark, &theme_id, &name)?;
    let light = variant_of(&table, Variant::Light, &theme_id, &name)?;
    for t in [&dark, &light] {
        user::check_variant(t, author.as_deref()).map_err(|problem| ImportError::Variant {
            variant: t.variant,
            problem,
        })?;
    }
    user::check_pair(&dark, &light, author.as_deref()).map_err(|problem| ImportError::Variant {
        variant: Variant::Dark,
        problem,
    })?;
    let look = match table.get("look") {
        None => None,
        Some(v) => check_look(v)?,
    };
    Ok(UqTheme {
        slug,
        name,
        author,
        dark,
        light,
        look,
    })
}

/// The `[look]` table as settings JSON, once uQ's own `look` rules accept it. An empty table is
/// the same as none.
fn check_look(v: &toml::Value) -> Result<Option<Map<String, Value>>, ImportError> {
    let bad = |why: &str| ImportError::BadLook(why.to_string());
    let toml::Value::Table(t) = v else {
        return Err(bad("it should be a list of settings."));
    };
    let mut map = Map::new();
    for (key, value) in t {
        if !Look::FIELDS.contains(&key.as_str()) {
            return Err(ImportError::Unknown(clean(&format!("look.{key}"))));
        }
        let json = match value {
            toml::Value::String(s) => Value::String(s.clone()),
            toml::Value::Integer(n) => Value::from(*n),
            toml::Value::Float(f) => Value::Number(
                serde_json::Number::from_f64(*f).ok_or_else(|| bad("a number can't be used."))?,
            ),
            toml::Value::Boolean(b) => Value::Bool(*b),
            _ => return Err(bad("a setting has the wrong kind of value.")),
        };
        map.insert(key.clone(), json);
    }
    if map.is_empty() {
        return Ok(None);
    }
    Settings::default()
        .set_section("look", Value::Object(map.clone()))
        .map_err(|e| ImportError::BadLook(look_reason(&e)))?;
    Ok(Some(map))
}

fn look_reason(e: &SettingError) -> String {
    match e {
        SettingError::Invalid { key, reason } => {
            match key
                .strip_prefix("look.")
                .and_then(|k| crate::history::label("look", k))
            {
                Some(label) => format!("{label} {}.", clean(reason)),
                None => "a setting has the wrong kind of value.".into(),
            }
        }
        SettingError::UnknownKey(_) => "a setting isn't one uQ has.".into(),
    }
}

#[derive(Serialize)]
struct Body<'a> {
    colors: &'a Colors,
    ui: &'a Ui,
    fonts: &'a Fonts,
    icons: &'a Icons,
    terminal: &'a Terminal,
}

impl<'a> Body<'a> {
    fn of(t: &'a Theme) -> Self {
        Body {
            colors: &t.colors,
            ui: &t.ui,
            fonts: &t.fonts,
            icons: &t.icons,
            terminal: &t.terminal,
        }
    }
}

fn look_to_toml(look: &Map<String, Value>) -> toml::Table {
    look.iter()
        .filter_map(|(k, v)| {
            let v = match v {
                Value::Bool(b) => toml::Value::Boolean(*b),
                Value::Number(n) => match n.as_i64() {
                    Some(i) => toml::Value::Integer(i),
                    None => toml::Value::Float(n.as_f64()?),
                },
                Value::String(s) => toml::Value::String(s.clone()),
                _ => return None,
            };
            Some((k.clone(), v))
        })
        .collect()
}

/// The file's text: [`HEADER`], the scalars, then `[dark.*]`, `[light.*]` and `[look]`.
pub fn to_text(t: &UqTheme) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        format: i64,
        id: &'a str,
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        author: Option<&'a str>,
        dark: Body<'a>,
        light: Body<'a>,
        #[serde(skip_serializing_if = "Option::is_none")]
        look: Option<toml::Table>,
    }
    // `parse` strips one `user-`, so a slug that itself starts with it is written with a second.
    let id = if t.slug.starts_with(user::PREFIX) {
        format!("{}{}", user::PREFIX, t.slug)
    } else {
        t.slug.clone()
    };
    let body = toml::to_string(&Out {
        format: FORMAT,
        id: &id,
        name: &t.name,
        author: t.author.as_deref(),
        dark: Body::of(&t.dark),
        light: Body::of(&t.light),
        look: t.look.as_ref().map(look_to_toml),
    })
    .expect("a theme is plain data");
    format!("{HEADER}{body}")
}

/// A theme ready to export. The slug is the id without `user-`; the variants are renamed to
/// `user-<slug>` and take the dark one's name.
pub fn from_pair(
    dark: &Theme,
    light: &Theme,
    author: Option<&str>,
    look: Option<Map<String, Value>>,
) -> UqTheme {
    let slug = dark
        .id
        .strip_prefix(user::PREFIX)
        .unwrap_or(&dark.id)
        .to_string();
    let id = format!("{}{slug}", user::PREFIX);
    let name = dark.name.clone();
    let fit = |t: &Theme| Theme {
        id: id.clone(),
        name: name.clone(),
        ..t.clone()
    };
    UqTheme {
        dark: fit(dark),
        light: fit(light),
        slug,
        name,
        author: author.map(str::to_string),
        look,
    }
}

/// The two variant files of a user theme folder.
pub fn install_files(t: &UqTheme) -> RenderedFiles {
    let mut files = std::collections::BTreeMap::new();
    for theme in [&t.dark, &t.light] {
        files.insert(
            PathBuf::from(format!("{}.toml", theme.variant)),
            user::variant_toml(theme, t.author.as_deref()),
        );
    }
    RenderedFiles(files)
}

#[cfg(test)]
mod tests;
