//! The bar's layout (`shell.bar`): which islands show, in which zone and order, at what size,
//! and the options of the islands that have any (stock symbols, news feeds, clock, world clock).
//!
//! Loading is lenient (unknown islands, duplicates, surplus entries and bad options are dropped);
//! the API is strict ([`BarLayout::from_value`]). A file from before `shell.bar` existed gets
//! its layout from the old `shell.islands` switches ([`BarLayout::from_islands`]).

use super::{SettingError, check_known, invalid};
use serde::{Deserialize, Serialize};
use serde_json::Value;
mod layout;
pub use layout::*;
mod edit;
pub use edit::*;
mod options;
pub use options::*;

/// Every island the bar knows, in the order Settings offers them, with its display name.
pub const ISLANDS: [(&str, &str); 17] = [
    ("system", "App menu, workspaces and dock"),
    ("work", "Open windows"),
    ("clock", "Clock and calendar"),
    ("weather", "Weather"),
    ("notif", "Notifications"),
    ("status", "Status (network, sound, battery)"),
    ("tray", "Tray icons of other apps"),
    ("media", "Now playing"),
    ("stocks", "Stock ticker"),
    ("news", "News headlines"),
    ("sysmon", "System monitor"),
    ("worldclock", "World clock"),
    ("keyboard", "Keyboard layout"),
    ("security", "Security (oysterAV)"),
    ("crumbs", "Storage cleanup (crumBS)"),
    ("clip", "Clipboard history (uqlip)"),
    ("separator", "Separator"),
];

/// The id that may appear any number of times.
pub const SEPARATOR: &str = "separator";
/// Entries across all three zones.
pub const MAX_ITEMS: usize = 24;
pub const MAX_SYMBOLS: usize = 12;
pub const MAX_FEEDS: usize = 8;
pub const MAX_ZONES: usize = 4;
const MAX_URL: usize = 512;
/// A Custom island's width in Icon sizes: one more than Large, up to this.
pub const MIN_CUSTOM: u32 = 7;
pub const MAX_CUSTOM: u32 = 24;
/// An island's text size, in percent of the theme's, in steps of [`TEXT_SIZE_STEP`].
pub const MIN_TEXT_SIZE: u16 = 70;
pub const MAX_TEXT_SIZE: u16 = 150;
pub const TEXT_SIZE_STEP: u16 = 10;
const MAX_FONT_NAME: usize = 64;

pub fn known_island(id: &str) -> bool {
    ISLANDS.iter().any(|(i, _)| *i == id)
}

pub fn island_name(id: &str) -> Option<&'static str> {
    ISLANDS.iter().find(|(i, _)| *i == id).map(|(_, n)| *n)
}

/// How big an island is, in multiples of the Icon size (a square island as tall as the bar):
/// Icon 1x, Small 2x, Medium 4x, Large 6x, and Custom: wider than Large, in whole Icon sizes
/// ([`BarItem::units`], [`MIN_CUSTOM`] to [`MAX_CUSTOM`]). Each island decides what it shows at
/// each size; a Custom island shows what it shows at Large, with more room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Icon,
    /// `compact` before there were four sizes.
    #[serde(alias = "compact")]
    Small,
    Medium,
    /// `wide` before there were four sizes.
    #[serde(alias = "wide")]
    Large,
    /// Its width is the item's `units`.
    Custom,
}

impl Size {
    /// Width in Icon sizes; for Custom, the narrowest it can be (the item's `units` say more).
    pub fn units(self) -> u32 {
        match self {
            Size::Icon => 1,
            Size::Small => 2,
            Size::Medium => 4,
            Size::Large => 6,
            Size::Custom => MIN_CUSTOM,
        }
    }
}

/// The size an island has until the user picks one.
pub fn default_size(id: &str) -> Size {
    match id {
        "system" | "work" | "status" | "news" => Size::Large,
        "clock" | "media" | "stocks" | "worldclock" | "tray" => Size::Medium,
        "keyboard" | "security" | "clip" => Size::Icon,
        _ => Size::Small,
    }
}

/// `size` as stored: absent (or the old `normal`) is the island's default.
fn size_field<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Size>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(s) if s == "normal" => Ok(None),
        Some(s) => {
            Size::deserialize(serde::de::value::StrDeserializer::<D::Error>::new(&s)).map(Some)
        }
    }
}

/// How an island's text looks, in place of the theme's: the font, bold, italic, all capitals
/// and the size. The default (nothing set) is the theme's text; only the island's face changes,
/// not its popup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextStyle {
    /// A font family name, `monospace` for the theme's monospaced font, or `None` for the
    /// theme's interface font.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    /// All capitals.
    #[serde(skip_serializing_if = "is_false")]
    pub caps: bool,
    /// Percent of the theme's text size ([`MIN_TEXT_SIZE`]..=[`MAX_TEXT_SIZE`], in steps of
    /// [`TEXT_SIZE_STEP`]).
    #[serde(skip_serializing_if = "is_hundred")]
    pub size: u16,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_hundred(n: &u16) -> bool {
    *n == 100
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: None,
            bold: false,
            italic: false,
            caps: false,
            size: 100,
        }
    }
}

impl TextStyle {
    pub const FIELDS: [&str; 5] = ["font", "bold", "italic", "caps", "size"];

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// A font name: 1 to 64 characters, no control characters, not only spaces.
    pub fn valid_font(f: &str) -> bool {
        !f.trim().is_empty()
            && f.chars().count() <= MAX_FONT_NAME
            && !f.chars().any(char::is_control)
    }

    fn check(&self) -> Result<(), String> {
        if let Some(f) = &self.font
            && !Self::valid_font(f)
        {
            return Err(format!("invalid font name {f:?}"));
        }
        if !(MIN_TEXT_SIZE..=MAX_TEXT_SIZE).contains(&self.size)
            || !self.size.is_multiple_of(TEXT_SIZE_STEP)
        {
            return Err(format!(
                "text size {} is not {MIN_TEXT_SIZE} to {MAX_TEXT_SIZE} in steps of {TEXT_SIZE_STEP}",
                self.size
            ));
        }
        Ok(())
    }

    /// Load-time repair: a bad font is dropped, the size rounded to a step and clamped.
    fn repaired(mut self) -> Self {
        if self.font.as_deref().is_some_and(|f| !Self::valid_font(f)) {
            self.font = None;
        }
        let step = (self.size + TEXT_SIZE_STEP / 2) / TEXT_SIZE_STEP * TEXT_SIZE_STEP;
        self.size = step.clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
        self
    }
}

/// One island in a zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarItem {
    pub id: String,
    /// `None` is the island's [`default_size`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "size_field"
    )]
    pub size: Option<Size>,
    /// A Custom island's width in Icon sizes ([`MIN_CUSTOM`]..=[`MAX_CUSTOM`]); only with
    /// `size = "custom"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<u32>,
    /// Its text's look; `None` is the theme's. Not for separators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextStyle>,
    /// It grows to fill the bar's empty space beside its zone; `None` is the island's default
    /// ([`default_fill`]: only Open windows). Not for separators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
}

/// Whether an island fills the empty space beside its zone when its item doesn't say.
pub fn default_fill(id: &str) -> bool {
    id == "work"
}

impl BarItem {
    pub const FIELDS: [&str; 5] = ["id", "size", "units", "text", "fill"];

    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            size: None,
            units: None,
            text: None,
            fill: None,
        }
    }

    fn check_text(&self) -> Result<(), String> {
        match &self.text {
            Some(_) if self.id == SEPARATOR => Err("a separator has no text".into()),
            Some(t) => t.check(),
            None => Ok(()),
        }
    }

    fn check_fill(&self) -> Result<(), String> {
        match self.fill {
            Some(_) if self.id == SEPARATOR => Err("a separator doesn't fill".into()),
            _ => Ok(()),
        }
    }

    /// Whether it fills the empty space beside its zone.
    pub fn fills(&self) -> bool {
        self.id != SEPARATOR && self.fill.unwrap_or_else(|| default_fill(&self.id))
    }

    /// The size it is drawn at, before the bar steps it down to fit.
    pub fn size(&self) -> Size {
        self.size.unwrap_or_else(|| default_size(&self.id))
    }

    /// Its width in Icon sizes, before the bar steps it down to fit.
    pub fn width_units(&self) -> u32 {
        match self.size() {
            Size::Custom => self.units.unwrap_or(MIN_CUSTOM),
            s => s.units(),
        }
    }

    fn check_units(&self) -> Result<(), String> {
        match (self.size(), self.units) {
            (Size::Custom, Some(u)) if (MIN_CUSTOM..=MAX_CUSTOM).contains(&u) => Ok(()),
            (Size::Custom, Some(u)) => Err(format!(
                "custom width {u} is outside {MIN_CUSTOM} to {MAX_CUSTOM}"
            )),
            (Size::Custom, None) => Err("a custom size needs units".into()),
            (_, Some(_)) => Err("units are only for a custom size".into()),
            (_, None) => Ok(()),
        }
    }

    /// Load-time repair of the size: a Custom width is clamped (missing, it is the narrowest
    /// Custom), stray units are dropped, and a separator has no size.
    fn repair_units(&mut self) {
        if self.id == SEPARATOR {
            self.size = None;
            self.units = None;
            self.text = None;
            self.fill = None;
            return;
        }
        // The island's own default is stored as "not set".
        if self.fill == Some(default_fill(&self.id)) {
            self.fill = None;
        }
        // A text that is the theme's own is kept: on an island it opts out of the all-islands text.
        self.text = self.text.take().map(TextStyle::repaired);
        if self.size() == Size::Custom {
            self.units = Some(
                self.units
                    .unwrap_or(MIN_CUSTOM)
                    .clamp(MIN_CUSTOM, MAX_CUSTOM),
            );
        } else {
            self.units = None;
        }
    }
}

/// The three zones of the bar, left to right; `center` is truly centred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarLayout {
    pub start: Vec<BarItem>,
    pub center: Vec<BarItem>,
    pub end: Vec<BarItem>,
    pub options: BarOptions,
}

impl Default for BarLayout {
    /// A new user's bar: today's islands, weather left out until a location is chosen, and the
    /// other apps' tray icons in their own island before Status, and the Security island (shown only
    /// with oysterAV installed) before those.
    fn default() -> Self {
        let mut b = BarLayout::from_islands(&super::Islands::default(), false);
        let at = b
            .end
            .iter()
            .position(|i| i.id == "status")
            .unwrap_or(b.end.len());
        b.end.insert(at, BarItem::new("tray"));
        // The Security island hides itself unless oysterAV is installed.
        b.end.insert(at, BarItem::new("security"));
        b
    }
}

/// A text style from a file, field by field: a field of the wrong type is its default.
fn lenient_text(v: &Value) -> Option<TextStyle> {
    let o = v.as_object()?;
    let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
    Some(TextStyle {
        font: o.get("font").and_then(Value::as_str).map(String::from),
        bold: flag("bold"),
        italic: flag("italic"),
        caps: flag("caps"),
        size: o
            .get("size")
            .and_then(Value::as_u64)
            .map_or(100, |n| n.min(u64::from(u16::MAX)) as u16),
    })
}

/// Lenient load of `shell.bar`: anything that isn't a table loads as `None` (so the old
/// switches decide); a table loads field by field, then is sanitised.
pub(super) fn lenient_bar<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<BarLayout>, D::Error> {
    let v = Value::deserialize(d)?;
    let Value::Object(obj) = v else {
        return Ok(None);
    };
    let items = |zone: &str| -> Vec<BarItem> {
        match obj.get(zone) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| {
                    let id = v.get("id")?.as_str()?.to_string();
                    // An unknown size (or the old `normal`) is the island's default.
                    let size = v
                        .get("size")
                        .and_then(|s| serde_json::from_value::<Size>(s.clone()).ok());
                    let units = v
                        .get("units")
                        .and_then(Value::as_u64)
                        .map(|u| u.min(u64::from(MAX_CUSTOM)) as u32);
                    let text = v.get("text").and_then(lenient_text);
                    let fill = v.get("fill").and_then(Value::as_bool);
                    Some(BarItem {
                        id,
                        size,
                        units,
                        text,
                        fill,
                    })
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let opt = obj.get("options");
    let part = |name: &str| opt.and_then(|o| o.get(name)).cloned();
    fn lenient<T: serde::de::DeserializeOwned + Default>(v: Option<Value>) -> T {
        v.and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }
    let strings = |v: Option<Value>, field: &str| -> Option<Vec<String>> {
        match v?.get(field)? {
            Value::Array(a) => Some(
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect(),
            ),
            _ => None,
        }
    };
    let options = BarOptions {
        all: lenient(part("all")),
        clock: lenient(part("clock")),
        stocks: StocksOptions {
            symbols: strings(part("stocks"), "symbols")
                .unwrap_or_else(|| StocksOptions::default().symbols),
        },
        news: NewsOptions {
            feeds: strings(part("news"), "feeds").unwrap_or_default(),
        },
        worldclock: WorldClockOptions {
            zones: strings(part("worldclock"), "zones").unwrap_or_default(),
        },
        work: lenient(part("work")),
        sysmon: lenient(part("sysmon")),
        crumbs: lenient(part("crumbs")),
        security: lenient(part("security")),
    };
    Ok(Some(
        BarLayout {
            start: items("start"),
            center: items("center"),
            end: items("end"),
            options,
        }
        .sanitised(),
    ))
}

#[cfg(test)]
mod tests;
