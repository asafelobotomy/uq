//! The bar's layout (`shell.bar`): which islands show, in which zone and order, at what size,
//! and the options of the islands that have any (stock symbols, news feeds, clock, world clock).
//!
//! Loading is lenient (unknown islands, duplicates, surplus entries and bad options are dropped);
//! the API is strict ([`BarLayout::from_value`]). A file from before `shell.bar` existed gets
//! its layout from the old `shell.islands` switches ([`BarLayout::from_islands`]).

use super::{SettingError, check_known, invalid};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every island the bar knows, in the order Settings offers them, with its display name.
pub const ISLANDS: [(&str, &str); 13] = [
    ("system", "App menu, workspaces and dock"),
    ("work", "Open windows"),
    ("clock", "Clock and calendar"),
    ("weather", "Weather"),
    ("notif", "Notifications"),
    ("status", "Status (network, sound, battery)"),
    ("media", "Now playing"),
    ("stocks", "Stock ticker"),
    ("news", "News headlines"),
    ("sysmon", "System monitor"),
    ("worldclock", "World clock"),
    ("keyboard", "Keyboard layout"),
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

pub fn known_island(id: &str) -> bool {
    ISLANDS.iter().any(|(i, _)| *i == id)
}

pub fn island_name(id: &str) -> Option<&'static str> {
    ISLANDS.iter().find(|(i, _)| *i == id).map(|(_, n)| *n)
}

/// How big an island is, in multiples of the Icon size (a square island as tall as the bar):
/// Icon 1x, Small 2x, Medium 4x, Large 6x. Each island decides what it shows at each size.
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
}

impl Size {
    /// Width in Icon sizes.
    pub fn units(self) -> u32 {
        match self {
            Size::Icon => 1,
            Size::Small => 2,
            Size::Medium => 4,
            Size::Large => 6,
        }
    }
}

/// The size an island has until the user picks one.
pub fn default_size(id: &str) -> Size {
    match id {
        "system" | "work" | "status" | "news" => Size::Large,
        "clock" | "media" | "stocks" | "worldclock" => Size::Medium,
        "keyboard" => Size::Icon,
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
}

impl BarItem {
    pub const FIELDS: [&str; 2] = ["id", "size"];

    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            size: None,
        }
    }

    /// The size it is drawn at, before the bar steps it down to fit.
    pub fn size(&self) -> Size {
        self.size.unwrap_or_else(|| default_size(&self.id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateStyle {
    None,
    #[default]
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockOptions {
    pub seconds: bool,
    pub date: DateStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StocksOptions {
    /// Ticker symbols as the quote source spells them (`AAPL`, `^GSPC`, `BTC-USD`, `VOD.L`).
    pub symbols: Vec<String>,
}

impl Default for StocksOptions {
    fn default() -> Self {
        Self {
            symbols: vec!["^GSPC".into(), "AAPL".into(), "MSFT".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewsOptions {
    /// RSS or Atom feed URLs, https only. Empty until the user picks some.
    pub feeds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldClockOptions {
    /// IANA time zone names (`America/New_York`).
    pub zones: Vec<String>,
}

/// How the Open windows island orders its pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowSort {
    /// When each window opened (oldest first unless `descending`).
    #[default]
    Opened,
    /// The last notification from each window's app.
    Activity,
    /// Pinned apps in pin order, then the rest by when they opened.
    Pin,
    /// Processor and memory use of each window's process.
    Usage,
    /// The app's name, then the title.
    Name,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkOptions {
    pub sort: WindowSort,
    /// Reverse the order (newest, most active, busiest or Z first). Ignored for `pin`.
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarOptions {
    pub clock: ClockOptions,
    pub stocks: StocksOptions,
    pub news: NewsOptions,
    pub worldclock: WorldClockOptions,
    pub work: WorkOptions,
}

impl BarOptions {
    pub const FIELDS: [&str; 5] = ["clock", "stocks", "news", "worldclock", "work"];
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
    /// A new user's bar: today's islands, weather left out until a location is chosen.
    fn default() -> Self {
        BarLayout::from_islands(&super::Islands::default(), false)
    }
}

/// `^[A-Z0-9^.=-]{1,16}$` after upper-casing; must hold a letter or digit.
pub fn clean_symbol(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_uppercase();
    let ok = (1..=16).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'^' | b'.' | b'=' | b'-'))
        && s.bytes().any(|b| b.is_ascii_alphanumeric());
    ok.then_some(s)
}

/// An `https://` URL with a host, no whitespace or control characters, at most 512 bytes.
pub fn valid_feed_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    u.len() <= MAX_URL
        && !host.is_empty()
        && !host.contains('@')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'))
        && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// An IANA zone name: `Area/Location[/Sub]` or `UTC`.
pub fn valid_zone(z: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 32
            && p.starts_with(|c: char| c.is_ascii_alphabetic())
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+'))
    };
    z == "UTC" || {
        let parts: Vec<&str> = z.split('/').collect();
        (2..=3).contains(&parts.len()) && parts.iter().all(|p| part(p))
    }
}

impl BarLayout {
    pub const FIELDS: [&str; 4] = ["start", "center", "end", "options"];
    pub const ZONES: [&str; 3] = ["start", "center", "end"];

    /// The layout a `shell.islands` table stood for: System and Work on the left; weather
    /// (when a location is set), the clock, notifications and status on the right.
    pub fn from_islands(i: &super::Islands, weather: bool) -> Self {
        let pick = |list: &[(&str, bool)]| -> Vec<BarItem> {
            list.iter()
                .filter(|(_, on)| *on)
                .map(|(id, _)| BarItem::new(id))
                .collect()
        };
        BarLayout {
            start: pick(&[("system", i.system), ("work", i.work)]),
            center: Vec::new(),
            end: pick(&[
                ("weather", weather),
                ("clock", i.time),
                ("notif", i.notif),
                ("status", i.status),
            ]),
            options: BarOptions::default(),
        }
    }

    pub fn zone(&self, name: &str) -> Option<&Vec<BarItem>> {
        match name {
            "start" => Some(&self.start),
            "center" => Some(&self.center),
            "end" => Some(&self.end),
            _ => None,
        }
    }

    pub fn zone_mut(&mut self, name: &str) -> Option<&mut Vec<BarItem>> {
        match name {
            "start" => Some(&mut self.start),
            "center" => Some(&mut self.center),
            "end" => Some(&mut self.end),
            _ => None,
        }
    }

    pub fn items(&self) -> impl Iterator<Item = &BarItem> {
        self.start.iter().chain(&self.center).chain(&self.end)
    }

    /// True when island `id` is placed in any zone.
    pub fn has(&self, id: &str) -> bool {
        self.items().any(|i| i.id == id)
    }

    /// Rules shared by the API and the loader.
    pub fn validate(&self) -> Result<(), SettingError> {
        const KEY: &str = "shell.bar";
        let mut seen: Vec<&str> = Vec::new();
        let mut n = 0;
        for item in self.items() {
            n += 1;
            if !known_island(&item.id) {
                return Err(invalid(KEY, format!("unknown island {:?}", item.id)));
            }
            if item.id != SEPARATOR {
                if seen.contains(&item.id.as_str()) {
                    return Err(invalid(
                        KEY,
                        format!("island {:?} is placed twice", item.id),
                    ));
                }
                seen.push(&item.id);
            }
        }
        if n > MAX_ITEMS {
            return Err(invalid(KEY, format!("at most {MAX_ITEMS} islands")));
        }
        let o = &self.options;
        check_list(
            "shell.bar.options.stocks.symbols",
            &o.stocks.symbols,
            MAX_SYMBOLS,
            |s| clean_symbol(s).as_deref() == Some(s),
        )?;
        check_list(
            "shell.bar.options.news.feeds",
            &o.news.feeds,
            MAX_FEEDS,
            valid_feed_url,
        )?;
        check_list(
            "shell.bar.options.worldclock.zones",
            &o.worldclock.zones,
            MAX_ZONES,
            valid_zone,
        )?;
        Ok(())
    }

    /// Load-time repair: drop unknown, duplicate and surplus entries and bad option values.
    pub fn sanitised(mut self) -> Self {
        let mut seen: Vec<String> = Vec::new();
        let mut n = 0;
        for zone in [&mut self.start, &mut self.center, &mut self.end] {
            zone.retain(|i| {
                let keep = known_island(&i.id)
                    && (i.id == SEPARATOR || !seen.contains(&i.id))
                    && n < MAX_ITEMS;
                if keep {
                    n += 1;
                    seen.push(i.id.clone());
                }
                keep
            });
        }
        let o = &mut self.options;
        let mut syms: Vec<String> = Vec::new();
        for s in o.stocks.symbols.iter().filter_map(|s| clean_symbol(s)) {
            if !syms.contains(&s) {
                syms.push(s);
            }
        }
        syms.truncate(MAX_SYMBOLS);
        o.stocks.symbols = syms;
        dedup_valid(&mut o.news.feeds, MAX_FEEDS, valid_feed_url);
        dedup_valid(&mut o.worldclock.zones, MAX_ZONES, valid_zone);
        self
    }

    /// Parse `shell.bar` strictly: unknown keys and wrong types are errors, missing fields
    /// take their defaults (an absent zone is empty; absent options are the defaults).
    pub fn from_value(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "shell.bar";
        if !v.is_object() {
            return Err(invalid(KEY, "expected an object"));
        }
        check_known(KEY, v, &Self::FIELDS)?;
        let mut out = BarLayout {
            start: Vec::new(),
            center: Vec::new(),
            end: Vec::new(),
            options: BarOptions::default(),
        };
        for zone in Self::ZONES {
            let Some(z) = v.get(zone) else { continue };
            let key = format!("{KEY}.{zone}");
            let arr = z
                .as_array()
                .ok_or_else(|| invalid(&key, "expected an array"))?;
            for item in arr {
                check_known(&key, item, &BarItem::FIELDS)?;
                let it: BarItem = serde_json::from_value(item.clone())
                    .map_err(|e| invalid(&key, e.to_string()))?;
                out.zone_mut(zone).expect("known zone").push(it);
            }
        }
        if let Some(o) = v.get("options") {
            let key = "shell.bar.options";
            check_known(key, o, &BarOptions::FIELDS)?;
            out.options =
                serde_json::from_value(o.clone()).map_err(|e| invalid(key, e.to_string()))?;
        }
        out.validate()?;
        Ok(out)
    }
}

/// One edit of the bar as Settings asks for it, applied to the layout as uqd has it now. Each
/// names the island it means (`id` at `zone`/`index`), so an edit made on an older view is
/// refused instead of changing another island.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BarEdit {
    /// Move to `to_index` of `to_zone` (counted after taking it out).
    Move {
        id: String,
        zone: String,
        index: usize,
        to_zone: String,
        to_index: usize,
    },
    /// Add at `index` of `zone`, or at its end.
    Add {
        id: String,
        zone: String,
        #[serde(default)]
        index: Option<usize>,
    },
    Remove {
        id: String,
        zone: String,
        index: usize,
    },
    Size {
        id: String,
        zone: String,
        index: usize,
        size: Size,
    },
    /// Replace `options.<island>`.
    Options { island: String, value: Value },
    /// Back to a new user's bar (options kept).
    Reset,
}

fn stale() -> SettingError {
    invalid("shell.bar", "the bar changed meanwhile; try again")
}

impl BarLayout {
    fn check_at(&self, id: &str, zone: &str, index: usize) -> Result<(), SettingError> {
        let z = self
            .zone(zone)
            .ok_or_else(|| invalid("shell.bar", format!("unknown zone {zone:?}")))?;
        match z.get(index) {
            Some(it) if it.id == id => Ok(()),
            _ => Err(stale()),
        }
    }

    /// Apply `edit`; on error `self` is unchanged.
    pub fn apply(&mut self, edit: &BarEdit) -> Result<(), SettingError> {
        let mut next = self.clone();
        match edit {
            BarEdit::Move {
                id,
                zone,
                index,
                to_zone,
                to_index,
            } => {
                next.check_at(id, zone, *index)?;
                if next.zone(to_zone).is_none() {
                    return Err(invalid("shell.bar", format!("unknown zone {to_zone:?}")));
                }
                let item = next.zone_mut(zone).expect("checked").remove(*index);
                let to = next.zone_mut(to_zone).expect("checked");
                let at = (*to_index).min(to.len());
                to.insert(at, item);
            }
            BarEdit::Add { id, zone, index } => {
                let z = next
                    .zone_mut(zone)
                    .ok_or_else(|| invalid("shell.bar", format!("unknown zone {zone:?}")))?;
                let at = index.unwrap_or(z.len()).min(z.len());
                z.insert(at, BarItem::new(id));
            }
            BarEdit::Remove { id, zone, index } => {
                next.check_at(id, zone, *index)?;
                next.zone_mut(zone).expect("checked").remove(*index);
            }
            BarEdit::Size {
                id,
                zone,
                index,
                size,
            } => {
                next.check_at(id, zone, *index)?;
                next.zone_mut(zone).expect("checked")[*index].size =
                    (*size != default_size(id)).then_some(*size);
            }
            BarEdit::Options { island, value } => {
                let mut opts = serde_json::to_value(&next.options).expect("options serialise");
                if !BarOptions::FIELDS.contains(&island.as_str()) {
                    return Err(invalid(
                        "shell.bar.options",
                        format!("{island:?} has no options"),
                    ));
                }
                opts[island.as_str()] = value.clone();
                next.options = serde_json::from_value(opts)
                    .map_err(|e| invalid("shell.bar.options", e.to_string()))?;
            }
            BarEdit::Reset => {
                let options = std::mem::take(&mut next.options);
                next = BarLayout {
                    options,
                    ..BarLayout::default()
                };
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}

fn check_list(
    key: &str,
    list: &[String],
    max: usize,
    ok: impl Fn(&str) -> bool,
) -> Result<(), SettingError> {
    if list.len() > max {
        return Err(invalid(key, format!("at most {max} entries")));
    }
    for (i, s) in list.iter().enumerate() {
        if !ok(s) {
            return Err(invalid(key, format!("invalid entry {s:?}")));
        }
        if list[..i].contains(s) {
            return Err(invalid(key, format!("duplicate entry {s:?}")));
        }
    }
    Ok(())
}

fn dedup_valid(list: &mut Vec<String>, max: usize, ok: impl Fn(&str) -> bool) {
    let mut out: Vec<String> = Vec::new();
    for s in list.drain(..) {
        if ok(&s) && !out.contains(&s) {
            out.push(s);
        }
    }
    out.truncate(max);
    *list = out;
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
                    Some(BarItem { id, size })
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
mod tests {
    use super::super::{Islands, Settings};
    use super::*;
    use serde_json::json;

    fn ids(z: &[BarItem]) -> Vec<&str> {
        z.iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn default_is_todays_bar() {
        let b = BarLayout::default();
        assert_eq!(ids(&b.start), ["system", "work"]);
        assert!(b.center.is_empty());
        assert_eq!(ids(&b.end), ["clock", "notif", "status"]);
        assert!(b.validate().is_ok());
        assert_eq!(b.options.stocks.symbols, ["^GSPC", "AAPL", "MSFT"]);
    }

    #[test]
    fn from_islands_keeps_switches_and_weather() {
        let i = Islands {
            work: false,
            notif: false,
            ..Islands::default()
        };
        let b = BarLayout::from_islands(&i, true);
        assert_eq!(ids(&b.start), ["system"]);
        assert_eq!(ids(&b.end), ["weather", "clock", "status"]);
    }

    #[test]
    fn strict_rejects() {
        for bad in [
            json!([]),
            json!({"nope": []}),
            json!({"start": "system"}),
            json!({"start": [{"id": "nope"}]}),
            json!({"start": [{"id": "clock"}], "end": [{"id": "clock"}]}),
            json!({"start": [{"id": "clock", "size": "huge"}]}),
            json!({"start": [{"id": "clock", "x": 1}]}),
            json!({"start": [{"size": "wide"}]}),
            json!({"options": {"nope": {}}}),
            json!({"options": {"clock": {"seconds": "yes"}}}),
            json!({"options": {"clock": {"date": "medium"}}}),
            json!({"options": {"stocks": {"symbols": ["aapl"]}}}),
            json!({"options": {"stocks": {"symbols": ["AAPL", "AAPL"]}}}),
            json!({"options": {"stocks": {"symbols": ["A B"]}}}),
            json!({"options": {"news": {"feeds": ["http://x.org/rss"]}}}),
            json!({"options": {"news": {"feeds": ["https://"]}}}),
            json!({"options": {"news": {"feeds": ["https://u@x.org/"]}}}),
            json!({"options": {"news": {"feeds": ["https://x.org/a b"]}}}),
            json!({"options": {"worldclock": {"zones": ["../etc/passwd"]}}}),
            json!({"options": {"worldclock": {"zones": ["Nowhere"]}}}),
        ] {
            assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
        }
        let many: Vec<_> = (0..25).map(|_| json!({"id": "separator"})).collect();
        assert!(BarLayout::from_value(&json!({"start": many})).is_err());
        let syms: Vec<_> = (0..13).map(|i| format!("S{i}")).collect();
        assert!(BarLayout::from_value(&json!({"options": {"stocks": {"symbols": syms}}})).is_err());
    }

    #[test]
    fn strict_accepts() {
        let b = BarLayout::from_value(&json!({
            "start": [{"id": "work"}, {"id": "separator"}, {"id": "separator"}],
            "center": [{"id": "clock", "size": "wide"}],
            "end": [{"id": "stocks", "size": "compact"}],
            "options": {
                "clock": {"seconds": true, "date": "none"},
                "stocks": {"symbols": ["BTC-USD", "^FTSE", "VOD.L"]},
                "news": {"feeds": ["https://feeds.bbci.co.uk/news/world/rss.xml"]},
                "worldclock": {"zones": ["America/New_York", "UTC", "America/Argentina/Salta"]}
            }
        }))
        .unwrap();
        assert_eq!(b.center[0].size, Some(Size::Large), "wide is now large");
        assert_eq!(b.end[0].size, Some(Size::Small), "compact is now small");
        assert!(b.has("stocks") && !b.has("news"));
        assert_eq!(b.options.clock.date, DateStyle::None);
        // Round trip; normal sizes are not written.
        let v = serde_json::to_value(&b).unwrap();
        assert!(v["start"][0].get("size").is_none());
        assert_eq!(BarLayout::from_value(&v).unwrap(), b);
        // Absent options keep the defaults.
        let b = BarLayout::from_value(&json!({"start": []})).unwrap();
        assert_eq!(b.options, BarOptions::default());
    }

    #[test]
    fn lenient_load_repairs() {
        #[derive(Deserialize)]
        struct W {
            #[serde(deserialize_with = "lenient_bar")]
            bar: Option<BarLayout>,
        }
        let w: W = serde_json::from_value(json!({"bar": {
            "start": [{"id": "work"}, {"id": "future"}, 3, {"id": "work"}, {"id": "clock", "size": "huge"}],
            "end": "x",
            "options": {"stocks": {"symbols": ["aapl", "AAPL", "bad sym"]}, "clock": 4,
                        "news": {"feeds": ["http://x", "https://ok.org/feed"]},
                        "worldclock": {"zones": ["Europe/Paris", 5]}}
        }}))
        .unwrap();
        let b = w.bar.unwrap();
        assert_eq!(ids(&b.start), ["work", "clock"]);
        assert_eq!(b.start[1].size, None);
        assert!(b.end.is_empty());
        assert_eq!(b.options.stocks.symbols, ["AAPL"]);
        assert_eq!(b.options.clock, ClockOptions::default());
        assert_eq!(b.options.news.feeds, ["https://ok.org/feed"]);
        assert_eq!(b.options.worldclock.zones, ["Europe/Paris"]);
        let w: W = serde_json::from_value(json!({"bar": 3})).unwrap();
        assert!(w.bar.is_none());
    }

    #[test]
    fn settings_migrate_and_keep_bar() {
        // An old file: no bar, work off, weather on.
        let mut s: Settings = serde_json::from_value(json!({
            "shell": {"islands": {"work": false}},
            "weather_location": {"name": "Oslo", "lat": 59.91, "lon": 10.75}
        }))
        .unwrap();
        assert!(s.shell.bar.is_none());
        s = s.validated().0;
        let b = s.shell.bar.clone().unwrap();
        assert_eq!(ids(&b.start), ["system"]);
        assert_eq!(ids(&b.end), ["weather", "clock", "notif", "status"]);
        // A shell section written without `bar` keeps the layout.
        s.set_section("shell", json!({"clock_24h": false})).unwrap();
        assert_eq!(s.shell.bar.as_ref(), Some(&b));
        // And one with it replaces it.
        s.set_section("shell", json!({"bar": {"center": [{"id": "clock"}]}}))
            .unwrap();
        assert_eq!(ids(&s.shell.bar.as_ref().unwrap().center), ["clock"]);
        assert!(s.shell.bar.as_ref().unwrap().start.is_empty());
        assert_eq!(
            s.section("shell").unwrap()["bar"]["center"][0]["id"],
            "clock"
        );
        // A fresh install has a bar from the start.
        assert_eq!(Settings::default().shell.bar, Some(BarLayout::default()));
    }

    #[test]
    fn edits() {
        let mut b = BarLayout::default();
        let e = |v: serde_json::Value| -> BarEdit { serde_json::from_value(v).unwrap() };
        b.apply(&e(
            json!({"op": "move", "id": "clock", "zone": "end", "index": 0,
            "to_zone": "center", "to_index": 0}),
        ))
        .unwrap();
        assert_eq!(ids(&b.center), ["clock"]);
        assert_eq!(ids(&b.end), ["notif", "status"]);
        // Stale: notif is no longer at end[0]... it is; status at end[0] isn't.
        let before = b.clone();
        assert!(
            b.apply(&e(
                json!({"op": "remove", "id": "status", "zone": "end", "index": 0})
            ))
            .is_err()
        );
        assert_eq!(b, before, "a refused edit changes nothing");
        b.apply(&e(
            json!({"op": "add", "id": "stocks", "zone": "start", "index": 1}),
        ))
        .unwrap();
        assert_eq!(ids(&b.start), ["system", "stocks", "work"]);
        assert!(
            b.apply(&e(json!({"op": "add", "id": "stocks", "zone": "end"})))
                .is_err(),
            "twice"
        );
        b.apply(&e(json!({"op": "add", "id": "separator", "zone": "end"})))
            .unwrap();
        assert_eq!(ids(&b.end), ["notif", "status", "separator"]);
        b.apply(&e(
            json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "wide"}),
        ))
        .unwrap();
        assert_eq!(b.start[1].size, Some(Size::Large));
        // Setting an island back to its own default stores nothing.
        b.apply(&e(
            json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "medium"}),
        ))
        .unwrap();
        assert_eq!(b.start[1].size, None);
        assert!(
            serde_json::from_value::<BarEdit>(
                json!({"op": "size", "id": "stocks", "zone": "start", "index": 1, "size": "normal"})
            )
            .is_err()
        );
        b.apply(&e(
            json!({"op": "options", "island": "stocks", "value": {"symbols": ["TSLA"]}}),
        ))
        .unwrap();
        assert_eq!(b.options.stocks.symbols, ["TSLA"]);
        for bad in [
            json!({"op": "options", "island": "stocks", "value": {"symbols": ["bad sym"]}}),
            json!({"op": "options", "island": "status", "value": {}}),
            json!({"op": "options", "island": "clock", "value": {"nope": 1}}),
            json!({"op": "add", "id": "nope", "zone": "end"}),
            json!({"op": "add", "id": "news", "zone": "middle"}),
            json!({"op": "move", "id": "notif", "zone": "end", "index": 0, "to_zone": "x", "to_index": 0}),
        ] {
            assert!(b.apply(&e(bad.clone())).is_err(), "{bad}");
        }
        assert!(serde_json::from_value::<BarEdit>(json!({"op": "fly"})).is_err());
        b.apply(&BarEdit::Reset).unwrap();
        assert_eq!(ids(&b.end), ["clock", "notif", "status"]);
        assert_eq!(b.options.stocks.symbols, ["TSLA"], "reset keeps options");
    }

    #[test]
    fn sizes_and_defaults() {
        let parse = |v| serde_json::from_value::<BarItem>(v).unwrap();
        assert_eq!(parse(json!({"id": "clock", "size": "normal"})).size, None);
        assert_eq!(
            parse(json!({"id": "clock", "size": "icon"})).size,
            Some(Size::Icon)
        );
        assert_eq!(parse(json!({"id": "clock"})).size(), Size::Medium);
        assert_eq!(parse(json!({"id": "keyboard"})).size(), Size::Icon);
        assert_eq!(parse(json!({"id": "work"})).size(), Size::Large);
        assert!(serde_json::from_value::<BarItem>(json!({"id": "clock", "size": "huge"})).is_err());
        assert_eq!(
            [Size::Icon, Size::Small, Size::Medium, Size::Large].map(Size::units),
            [1, 2, 4, 6]
        );
        let v = serde_json::to_value(BarItem {
            id: "news".into(),
            size: Some(Size::Icon),
        })
        .unwrap();
        assert_eq!(v, json!({"id": "news", "size": "icon"}));
        assert_eq!(island_name("system"), Some("App menu, workspaces and dock"));
    }

    #[test]
    fn work_sort_options() {
        let b = BarLayout::from_value(
            &json!({"options": {"work": {"sort": "usage", "descending": true}}}),
        )
        .unwrap();
        assert_eq!(b.options.work.sort, WindowSort::Usage);
        assert!(b.options.work.descending);
        assert_eq!(BarOptions::default().work.sort, WindowSort::Opened);
        for bad in [
            json!({"options": {"work": {"sort": "random"}}}),
            json!({"options": {"work": {"descending": "yes"}}}),
            json!({"options": {"work": {"nope": 1}}}),
        ] {
            assert!(BarLayout::from_value(&bad).is_err(), "{bad}");
        }
        #[derive(Deserialize)]
        struct W {
            #[serde(deserialize_with = "lenient_bar")]
            bar: Option<BarLayout>,
        }
        let w: W =
            serde_json::from_value(json!({"bar": {"options": {"work": {"sort": "random"}}}}))
                .unwrap();
        assert_eq!(w.bar.unwrap().options.work, WorkOptions::default());
        let mut b = BarLayout::default();
        b.apply(
            &serde_json::from_value(
                json!({"op": "options", "island": "work", "value": {"sort": "name"}}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(b.options.work.sort, WindowSort::Name);
    }

    #[test]
    fn helpers() {
        assert_eq!(clean_symbol(" brk.b "), Some("BRK.B".into()));
        assert_eq!(clean_symbol("^"), None);
        assert_eq!(clean_symbol("TOOLONGSYMBOL12345"), None);
        assert!(valid_feed_url("https://news.ycombinator.com/rss"));
        assert!(valid_feed_url("https://x.org:8443/feed?x=1"));
        assert!(!valid_feed_url("https://x.org\n/feed"));
        assert!(valid_zone("UTC") && valid_zone("Etc/GMT+5"));
        assert!(!valid_zone("Europe/") && !valid_zone("/Paris"));
        assert_eq!(island_name("stocks"), Some("Stock ticker"));
        assert_eq!(island_name("time"), None);
    }
}
