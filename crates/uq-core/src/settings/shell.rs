//! The `shell` section: the bar's layout (`bar`, see [`super::bar`]), the old island switches
//! it replaced (`islands`, read only to derive a layout for older files), the clock format, the
//! launcher's provider order and enablement, the terminal for `Terminal=true` launches, and
//! whether the screen-edge indicators show, how (an icon tab or a slice of the window) and
//! whether the tab has the window's title. The shell reads it
//! through `SettingsChanged`; uqd renders nothing for it.
//!
//! Loading is lenient (wrong-typed or unknown values fall back or are dropped, so a bad
//! `[shell]` never costs the user their theme or pins); the API is strict
//! ([`ShellSettings::from_section`]).

use super::bar::BarLayout;
use super::{SettingError, check_known, invalid, valid_pinned_id};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use zbus::names::WellKnownName;

/// Providers the launcher always has, in their default order. The apps group is not one.
/// The launcher'"'"'s own search groups, in the order they show after Apps: web addresses, the
/// calculator, Run, open windows, Commands, Settings and files (every `files:<type>` group, contents too).
pub const BUILTIN_PROVIDERS: [&str; 7] = [
    "web", "calc", "run", "windows", "commands", "settings", "files",
];

const MAX_PROVIDERS: usize = 64;

/// Load-time leniency: a value of the wrong type becomes `fallback()`.
fn lenient_or<'de, D, T>(d: D, fallback: fn() -> T) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(T::deserialize(Value::deserialize(d)?).unwrap_or_else(|_| fallback()))
}

fn true_() -> bool {
    true
}

fn lenient_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    lenient_or(d, true_)
}

fn lenient_false<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    lenient_or(d, bool::default)
}

fn lenient_islands<'de, D: Deserializer<'de>>(d: D) -> Result<Islands, D::Error> {
    lenient_or(d, Islands::default)
}

fn lenient_terminal<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => Some(s),
        _ => None,
    })
}

/// Entries that aren't `{id, enabled}` tables are dropped; a non-array is an empty list.
fn lenient_providers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ProviderPref>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(a) => a
            .into_iter()
            .filter_map(|v| serde_json::from_value::<ProviderPref>(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

/// How the screen-edge indicators look (`shell/edge/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeStyle {
    /// A tab with the window's app icon, shown when the pointer reaches the edge.
    #[default]
    Tab,
    /// A slice of the window itself at the edge: always there while the current window is
    /// fullscreen and hides it, otherwise shown when the pointer reaches the edge.
    Peek,
}

impl EdgeStyle {
    pub const NAMES: [&str; 2] = ["tab", "peek"];
}

fn lenient_edge_style<'de, D: Deserializer<'de>>(d: D) -> Result<EdgeStyle, D::Error> {
    lenient_or(d, EdgeStyle::default)
}

/// How the bar writes dates (the clock island and its calendar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateFormat {
    /// Day before month ("Sat 21 Nov"), weeks from Monday.
    #[default]
    Uk,
    /// Month before day ("Sat, Nov 21"), weeks from Sunday.
    Us,
}

impl DateFormat {
    pub const NAMES: [&str; 2] = ["uk", "us"];
}

fn lenient_date_format<'de, D: Deserializer<'de>>(d: D) -> Result<DateFormat, D::Error> {
    lenient_or(d, DateFormat::default)
}

/// The unit every temperature the shell shows is in (weather, system monitor). Readings
/// themselves stay Celsius; only what people read changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TempUnit {
    #[default]
    C,
    F,
}

impl TempUnit {
    pub const NAMES: [&str; 2] = ["c", "f"];
}

fn lenient_temp_unit<'de, D: Deserializer<'de>>(d: D) -> Result<TempUnit, D::Error> {
    lenient_or(d, TempUnit::default)
}

/// The five on/off switches the bar had before `shell.bar`. Still read, so a file from then
/// gets an equivalent layout; nothing in uQ writes them any more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Islands {
    #[serde(deserialize_with = "lenient_true")]
    pub system: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub work: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub time: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub notif: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub status: bool,
}

impl Default for Islands {
    fn default() -> Self {
        Self {
            system: true,
            work: true,
            time: true,
            notif: true,
            status: true,
        }
    }
}

impl Islands {
    pub const FIELDS: [&str; 5] = ["system", "work", "time", "notif", "status"];
}

/// One launcher provider's preference. `id` is one of [`BUILTIN_PROVIDERS`] or a D-Bus well-known
/// name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPref {
    pub id: String,
    #[serde(default = "true_", deserialize_with = "lenient_true")]
    pub enabled: bool,
}

impl ProviderPref {
    pub const FIELDS: [&str; 2] = ["id", "enabled"];

    /// `apps` (the built-in group, always first and always on) must never become a valid
    /// provider id; it has no dot, so the well-known-name check rejects it.
    pub fn valid_id(id: &str) -> bool {
        BUILTIN_PROVIDERS.contains(&id)
            || (!id.starts_with(':') && WellKnownName::try_from(id).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellSettings {
    #[serde(deserialize_with = "lenient_islands")]
    pub islands: Islands,
    /// The bar's layout. `None` only between loading a file that lacks it and
    /// [`super::Settings::validated`], which derives it from `islands`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::bar::lenient_bar"
    )]
    pub bar: Option<BarLayout>,
    /// The user's order; ids the launcher knows but this lacks follow in default order.
    #[serde(deserialize_with = "lenient_providers")]
    pub providers: Vec<ProviderPref>,
    /// `true` is `HH:mm`, `false` is the 12-hour clock.
    #[serde(deserialize_with = "lenient_true")]
    pub clock_24h: bool,
    /// UK (day first, the default) or US (month first) dates in the bar.
    #[serde(deserialize_with = "lenient_date_format")]
    pub date_format: DateFormat,
    /// Celsius (the default) or Fahrenheit, wherever the shell shows a temperature.
    #[serde(deserialize_with = "lenient_temp_unit")]
    pub temp_unit: TempUnit,
    /// A desktop id for `Terminal=true` launches; `None` means kyko, else kitty.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_terminal"
    )]
    pub terminal: Option<String>,
    /// The screen-edge tabs for the window or workspace beside the current one
    /// (`shell/edge/`).
    #[serde(deserialize_with = "lenient_true")]
    pub edge_indicators: bool,
    /// Show the window's title on those tabs; `false` (the default) is the icon only.
    #[serde(deserialize_with = "lenient_false")]
    pub edge_titles: bool,
    #[serde(deserialize_with = "lenient_edge_style")]
    pub edge_style: EdgeStyle,
    /// Keep every floating window inside the usable part of its screen: one that opens (or is
    /// moved, or whose screen changes) bigger than that is shrunk to fit, and one that hangs
    /// over an edge is moved back (`shell/windows/`).
    #[serde(deserialize_with = "lenient_true")]
    pub fit_windows: bool,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            islands: Islands::default(),
            bar: Some(BarLayout::default()),
            providers: Vec::new(),
            clock_24h: true,
            date_format: DateFormat::Uk,
            temp_unit: TempUnit::C,
            terminal: None,
            edge_indicators: true,
            edge_titles: false,
            edge_style: EdgeStyle::Tab,
            fit_windows: true,
        }
    }
}

/// A desktop id for launching: `valid_pinned_id`, ending in `.desktop`, a non-empty stem that
/// starts alphanumeric (never `-x`, so it can't be read as an option) and has no `..`.
pub fn valid_terminal(t: &str) -> bool {
    valid_pinned_id(t)
        && t.strip_suffix(".desktop")
            .is_some_and(|stem| stem.starts_with(|c: char| c.is_ascii_alphanumeric()))
        && !t.contains("..")
}

fn expect_bool(key: &str, v: &Value) -> Result<bool, SettingError> {
    v.as_bool()
        .ok_or_else(|| invalid(key, "expected a boolean"))
}

impl ShellSettings {
    /// Every key of the `shell` section.
    pub const FIELDS: [&str; 11] = [
        "islands",
        "bar",
        "providers",
        "clock_24h",
        "date_format",
        "temp_unit",
        "terminal",
        "edge_indicators",
        "edge_titles",
        "edge_style",
        "fit_windows",
    ];

    /// Rules shared by the API and the loader: ids valid, unique, at most 64, terminal valid.
    pub fn validate(&self) -> Result<(), SettingError> {
        if self.providers.len() > MAX_PROVIDERS {
            return Err(invalid(
                "shell.providers",
                format!("at most {MAX_PROVIDERS} entries"),
            ));
        }
        for (i, p) in self.providers.iter().enumerate() {
            if !ProviderPref::valid_id(&p.id) {
                return Err(invalid(
                    "shell.providers",
                    format!("invalid provider id {:?}", p.id),
                ));
            }
            if self.providers[..i].iter().any(|q| q.id == p.id) {
                return Err(invalid(
                    "shell.providers",
                    format!("duplicate provider id {:?}", p.id),
                ));
            }
        }
        if let Some(b) = &self.bar {
            b.validate()?;
        }
        if let Some(t) = &self.terminal
            && !valid_terminal(t)
        {
            return Err(invalid(
                "shell.terminal",
                "expected a desktop id ending in .desktop",
            ));
        }
        Ok(())
    }

    /// Load-time repair: drop invalid, duplicate and surplus providers, and an invalid
    /// terminal, instead of failing.
    pub fn sanitised(mut self) -> Self {
        let mut seen: Vec<String> = Vec::new();
        self.providers.retain(|p| {
            let keep = ProviderPref::valid_id(&p.id) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        self.providers.truncate(MAX_PROVIDERS);
        self.terminal = self.terminal.filter(|t| valid_terminal(t));
        self.bar = self.bar.map(BarLayout::sanitised);
        self
    }

    /// Parse a whole `shell` section strictly: unknown keys and wrong types are errors, and
    /// missing fields take their defaults; a missing `bar` is `None` (the caller keeps its own).
    pub fn from_section(v: &Value) -> Result<Self, SettingError> {
        const KEY: &str = "shell";
        let obj = v
            .as_object()
            .ok_or_else(|| invalid(KEY, "expected an object"))?;
        check_known(KEY, v, &Self::FIELDS)?;
        let mut out = Self {
            bar: None,
            ..Self::default()
        };
        if let Some(b) = obj.get("bar") {
            out.bar = Some(BarLayout::from_value(b)?);
        }
        if let Some(i) = obj.get("islands") {
            let io = i
                .as_object()
                .ok_or_else(|| invalid("shell.islands", "expected an object"))?;
            check_known("shell.islands", i, &Islands::FIELDS)?;
            for (name, slot) in [
                ("system", &mut out.islands.system),
                ("work", &mut out.islands.work),
                ("time", &mut out.islands.time),
                ("notif", &mut out.islands.notif),
                ("status", &mut out.islands.status),
            ] {
                if let Some(b) = io.get(name) {
                    *slot = expect_bool(&format!("shell.islands.{name}"), b)?;
                }
            }
        }
        if let Some(p) = obj.get("providers") {
            let arr = p
                .as_array()
                .ok_or_else(|| invalid("shell.providers", "expected an array"))?;
            for item in arr {
                check_known("shell.providers", item, &ProviderPref::FIELDS)?;
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("shell.providers", "each entry needs a string id"))?;
                let enabled = match item.get("enabled") {
                    None => true,
                    Some(b) => expect_bool("shell.providers.enabled", b)?,
                };
                out.providers.push(ProviderPref {
                    id: id.to_string(),
                    enabled,
                });
            }
        }
        if let Some(c) = obj.get("clock_24h") {
            out.clock_24h = expect_bool("shell.clock_24h", c)?;
        }
        if let Some(e) = obj.get("edge_indicators") {
            out.edge_indicators = expect_bool("shell.edge_indicators", e)?;
        }
        if let Some(e) = obj.get("edge_titles") {
            out.edge_titles = expect_bool("shell.edge_titles", e)?;
        }
        if let Some(f) = obj.get("fit_windows") {
            out.fit_windows = expect_bool("shell.fit_windows", f)?;
        }
        if let Some(f) = obj.get("date_format") {
            out.date_format = serde_json::from_value(f.clone()).map_err(|_| {
                invalid(
                    "shell.date_format",
                    format!("expected one of {:?}", DateFormat::NAMES),
                )
            })?;
        }
        if let Some(u) = obj.get("temp_unit") {
            out.temp_unit = serde_json::from_value(u.clone()).map_err(|_| {
                invalid(
                    "shell.temp_unit",
                    format!("expected one of {:?}", TempUnit::NAMES),
                )
            })?;
        }
        if let Some(e) = obj.get("edge_style") {
            out.edge_style = serde_json::from_value(e.clone()).map_err(|_| {
                invalid(
                    "shell.edge_style",
                    format!("expected one of {:?}", EdgeStyle::NAMES),
                )
            })?;
        }
        match obj.get("terminal") {
            None | Some(Value::Null) => {}
            Some(Value::String(s)) => out.terminal = Some(s.clone()),
            Some(_) => return Err(invalid("shell.terminal", "expected a string or null")),
        }
        out.validate()?;
        Ok(out)
    }
}

/// The launcher's provider ids in display order: ids `prefs` lists first (in `prefs` order),
/// then the rest (builtins in [`BUILTIN_PROVIDERS`] order, then others sorted). Disabled ids
/// are dropped.
pub fn provider_order(prefs: &[ProviderPref], ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in prefs {
        if ids.contains(&p.id) && !out.contains(&p.id) {
            out.push(p.id.clone());
        }
    }
    let mut rest: Vec<&String> = ids.iter().filter(|i| !out.contains(i)).collect();
    rest.sort_by_key(|i| {
        (
            BUILTIN_PROVIDERS
                .iter()
                .position(|b| b == i)
                .unwrap_or(BUILTIN_PROVIDERS.len()),
            (*i).clone(),
        )
    });
    rest.dedup();
    out.extend(rest.into_iter().cloned());
    out.retain(|i| provider_enabled(prefs, i));
    out
}

/// True unless `id` is listed and disabled.
pub fn provider_enabled(prefs: &[ProviderPref], id: &str) -> bool {
    prefs.iter().find(|p| p.id == id).is_none_or(|p| p.enabled)
}

#[cfg(test)]
mod tests;
