//! Integration manifests: `widget.toml` (Phase 4 spec §2.2) and `page.toml` (Phase 3 spec §2.5,
//! extended by Phase 4 §2.3), their shared `[requires]`, `[dbus]` and `[[command]]` tables, and
//! the folder loader.
//!
//! Validation is strict: unknown keys are errors, ids are `[a-z0-9-]{1,40}` and equal the folder
//! name, a QML file is a bare file name that exists and resolves inside its folder. A manifest
//! declaring a newer `uq_api` major than [`UQ_API`] is refused with "needs a newer uQ" before any
//! other check, so a newer key never reads as a mistake (§2.6).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use zbus::names::{InterfaceName, MemberName, WellKnownName};
use zbus::zvariant::ObjectPath;

/// The integration API this uQ implements (§2.6).
pub const UQ_API: u32 = 1;

/// A manifest file over this size is refused unread.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Where `argv[0]` of a declared command may live (§2.3).
pub const COMMAND_PREFIXES: [&str; 2] = ["/usr/bin", "/usr/lib"];

/// Where `[requires] bins` are looked up (§2.2).
pub const BIN_DIR: &str = "/usr/bin";

/// `timeout_ms` when a command gives none, and the most it may ask for (§2.3).
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
pub const MAX_TIMEOUT_MS: u64 = 600_000;

/// The most a page's `[dbus] timeout_ms` may give one method: long enough for a polkit password
/// prompt. Methods not listed keep [`crate::integration::dbus_json::CALL_DEADLINE`].
pub const MAX_DBUS_TIMEOUT_MS: u64 = 120_000;

const MAX_COMMANDS: usize = 64;
const MAX_ARGV: usize = 64;
const MAX_ARG_BYTES: usize = 4096;
const MAX_DECLARED_ARGS: usize = 16;
const MAX_PATTERN_CHARS: usize = 512;
const MAX_MEMBERS: usize = 64;
const MAX_BINS: usize = 16;
const MAX_SETTINGS: usize = 200;
const MAX_KEYWORDS: usize = 32;
const MAX_CONFIRM_CHARS: usize = 300;

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// The manifest targets a newer integration API (§2.6, §5).
    #[error("needs a newer uQ (uq_api = {0}; this uQ supports {UQ_API})")]
    NeedsNewerUq(i64),
    #[error("{0}")]
    Invalid(String),
}

fn invalid(msg: impl Into<String>) -> ManifestError {
    ManifestError::Invalid(msg.into())
}

// ---------------------------------------------------------------------------------------------
// Small validators, shared with the other integration modules and the hosts.

/// An integration id: `^[a-z0-9-]{1,40}$`.
pub fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A `[[command]]` id or an argument name: `^[a-z0-9_-]{1,40}$`.
pub fn valid_command_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// A `[[setting]]` id: `^[A-Za-z0-9._-]{1,64}$`.
pub fn valid_setting_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A theme icon name, `^[A-Za-z0-9._-]{1,64}$`: no paths or URLs.
pub fn valid_icon_name(name: &str) -> bool {
    valid_setting_id(name)
}

/// 1 to `max` characters, none of them control characters.
pub fn valid_text(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}

fn valid_keywords(words: &[String]) -> bool {
    words.len() <= MAX_KEYWORDS && words.iter().all(|k| valid_text(k, 64))
}

/// A bare file name ending `.qml`: no directories, no `..`, no hidden or empty stem.
pub fn valid_qml_name(name: &str) -> bool {
    name.len() <= 100
        && name.len() > ".qml".len()
        && name.ends_with(".qml")
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '%' | '?' | '#'))
}

/// A program name for `[requires] bins`: `^[A-Za-z0-9._+-]{1,64}$`, not starting with `.`.
pub fn valid_bin_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
}

/// A well-known bus name (not a unique `:1.x` one).
pub fn valid_bus_name(name: &str) -> bool {
    !name.starts_with(':') && WellKnownName::try_from(name).is_ok()
}

/// `path` is absolute, already normalized (no `.`, `..`, `//` or trailing `/`), and strictly
/// inside one of `prefixes` (compared by whole components).
///
/// The check is lexical: symlinks are not resolved, so a link inside a prefix that points
/// outside it passes. The prefixes are root-owned, so only root or a package can place one.
pub fn path_under(path: &str, prefixes: &[PathBuf]) -> bool {
    let p = Path::new(path);
    if !p.is_absolute() || path.contains('\0') {
        return false;
    }
    let normal: PathBuf = p.components().collect();
    if normal.as_os_str() != p.as_os_str()
        || p.components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return false;
    }
    prefixes.iter().any(|pre| p.starts_with(pre) && p != pre)
}

/// The default `argv[0]` prefixes as paths.
pub fn command_prefixes() -> Vec<PathBuf> {
    COMMAND_PREFIXES.iter().map(PathBuf::from).collect()
}

// ---------------------------------------------------------------------------------------------
// Shared tables.

/// `[requires]`: every listed condition must hold for the widget or page to show (§2.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requires {
    /// Shown only while this name has an owner (on the `[dbus]` table's bus; session if none).
    pub bus_name: Option<String>,
    /// Shown only if each of these exists in `/usr/bin`.
    pub bins: Vec<String>,
}

impl Requires {
    /// The listed programs missing from `bin_dir` (normally [`BIN_DIR`]).
    pub fn missing_bins(&self, bin_dir: &Path) -> Vec<String> {
        self.bins
            .iter()
            .filter(|b| !bin_dir.join(b).is_file())
            .cloned()
            .collect()
    }
}

/// Which message bus a `[dbus]` table talks to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bus {
    #[default]
    Session,
    /// A system service, such as a daemon with its own polkit actions.
    System,
}

/// `[dbus]`: the one name, path and interface a widget or page may reach, and the members it
/// may use (§2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbusSpec {
    pub name: String,
    pub path: String,
    pub interface: String,
    pub methods: Vec<String>,
    pub properties: Vec<String>,
    pub signals: Vec<String>,
    /// `bus = "session" | "system"`, session by default.
    pub bus: Bus,
    /// `timeout_ms = { Method = ms }`: a longer deadline for listed methods that may wait for a
    /// password prompt (pages only; at most [`MAX_DBUS_TIMEOUT_MS`]).
    pub timeout_ms: BTreeMap<String, u64>,
}

impl DbusSpec {
    /// How long a call of `method` may take: its `timeout_ms`, else the usual 2 s.
    pub fn deadline(&self, method: &str) -> std::time::Duration {
        self.timeout_ms
            .get(method)
            .map_or(crate::integration::dbus_json::CALL_DEADLINE, |ms| {
                std::time::Duration::from_millis(*ms)
            })
    }
}

/// How a command's stdout is handed back (§2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Output {
    /// Parsed as JSON; a parse failure is an error.
    Json,
    /// As text (invalid UTF-8 replaced).
    Text,
    /// Ignored; `output` is `null`.
    None,
}

/// One declared argument: the caller's value must match `pattern` in full.
#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub name: String,
    /// As written in the manifest (`^…$`).
    pub pattern: String,
    /// The pattern compiled as `\A(?:pattern)\z`, so it can only ever match the whole value.
    pub regex: Regex,
}

impl PartialEq for ArgSpec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.pattern == other.pattern
    }
}
impl Eq for ArgSpec {}

impl ArgSpec {
    /// A declared argument. `pattern` must start with `^` and end with `$`.
    pub fn new(name: &str, pattern: &str) -> Result<ArgSpec, String> {
        if !valid_command_id(name) {
            return Err(format!(
                "argument name {name:?} must match [a-z0-9_-]{{1,40}}"
            ));
        }
        if pattern.chars().count() > MAX_PATTERN_CHARS
            || !pattern.starts_with('^')
            || !pattern.ends_with('$')
            || pattern.ends_with("\\$")
        {
            return Err(format!(
                "argument {name:?}: pattern must be anchored, starting with ^ and ending with $ \
                 (at most {MAX_PATTERN_CHARS} characters)"
            ));
        }
        let regex = regex::RegexBuilder::new(&format!(r"\A(?:{pattern})\z"))
            .size_limit(1 << 20)
            .build()
            .map_err(|e| format!("argument {name:?}: bad pattern: {e}"))?;
        Ok(ArgSpec {
            name: name.into(),
            pattern: pattern.into(),
            regex,
        })
    }
}

/// A `[[command]]` table (§2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: String,
    /// The fixed argv; `argv[0]` is an absolute path under `/usr/bin` or `/usr/lib`.
    pub argv: Vec<String>,
    pub output: Output,
    pub timeout_ms: u64,
    /// The arguments a caller must append, in order.
    pub args: Vec<ArgSpec>,
    /// A fixed question shown before running; at most one `%s` (the first argument).
    pub confirm: Option<String>,
}

impl CommandSpec {
    /// The confirmation text with `%s` replaced by the first argument made safe for one line;
    /// `None` when the command has no `confirm`.
    pub fn confirm_text(&self, args: &[String]) -> Option<String> {
        let text = self.confirm.as_ref()?;
        let first = args.first().map(String::as_str).unwrap_or("");
        Some(text.replacen("%s", &crate::desktop_entry::clean_display(first, 200), 1))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequires {
    bus_name: Option<String>,
    #[serde(default)]
    bins: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDbus {
    name: String,
    path: String,
    interface: String,
    #[serde(default)]
    methods: Vec<String>,
    #[serde(default)]
    properties: Vec<String>,
    #[serde(default)]
    signals: Vec<String>,
    #[serde(default)]
    bus: Bus,
    #[serde(default)]
    timeout_ms: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArg {
    name: String,
    pattern: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCommand {
    id: String,
    argv: Vec<String>,
    output: Output,
    timeout_ms: Option<u64>,
    #[serde(default)]
    args: Vec<RawArg>,
    confirm: Option<String>,
}

fn requires_from(raw: Option<RawRequires>) -> Result<Requires, ManifestError> {
    let Some(raw) = raw else {
        return Ok(Requires::default());
    };
    if let Some(n) = &raw.bus_name
        && !valid_bus_name(n)
    {
        return Err(invalid(format!(
            "requires.bus_name {n:?} is not a valid bus name"
        )));
    }
    if raw.bins.len() > MAX_BINS {
        return Err(invalid(format!("requires.bins: at most {MAX_BINS}")));
    }
    if let Some(b) = raw.bins.iter().find(|b| !valid_bin_name(b)) {
        return Err(invalid(format!(
            "requires.bins: {b:?} must be a program name in /usr/bin, not a path"
        )));
    }
    Ok(Requires {
        bus_name: raw.bus_name,
        bins: raw.bins,
    })
}

fn members(kind: &str, list: Vec<String>) -> Result<Vec<String>, ManifestError> {
    if list.len() > MAX_MEMBERS {
        return Err(invalid(format!("dbus.{kind}: at most {MAX_MEMBERS}")));
    }
    for (i, m) in list.iter().enumerate() {
        if MemberName::try_from(m.as_str()).is_err() {
            return Err(invalid(format!("dbus.{kind}: {m:?} is not a member name")));
        }
        if list[..i].contains(m) {
            return Err(invalid(format!("dbus.{kind}: {m:?} is listed twice")));
        }
    }
    Ok(list)
}

fn dbus_from(raw: Option<RawDbus>) -> Result<Option<DbusSpec>, ManifestError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if !valid_bus_name(&raw.name) {
        return Err(invalid(format!(
            "dbus.name {:?} is not a valid bus name",
            raw.name
        )));
    }
    if ObjectPath::try_from(raw.path.as_str()).is_err() {
        return Err(invalid(format!(
            "dbus.path {:?} is not an object path",
            raw.path
        )));
    }
    if InterfaceName::try_from(raw.interface.as_str()).is_err() {
        return Err(invalid(format!(
            "dbus.interface {:?} is not an interface name",
            raw.interface
        )));
    }
    let methods = members("methods", raw.methods)?;
    for (method, ms) in &raw.timeout_ms {
        if !methods.contains(method) {
            return Err(invalid(format!(
                "dbus.timeout_ms: {method:?} is not listed in dbus.methods"
            )));
        }
        if !(1..=MAX_DBUS_TIMEOUT_MS).contains(ms) {
            return Err(invalid(format!(
                "dbus.timeout_ms: {method:?} must be 1 to {MAX_DBUS_TIMEOUT_MS}, not {ms}"
            )));
        }
    }
    Ok(Some(DbusSpec {
        methods,
        properties: members("properties", raw.properties)?,
        signals: members("signals", raw.signals)?,
        name: raw.name,
        path: raw.path,
        interface: raw.interface,
        bus: raw.bus,
        timeout_ms: raw.timeout_ms,
    }))
}

/// Validate one `[[command]]` table against the allowed `argv[0]` prefixes.
fn command_from(raw: RawCommand, prefixes: &[PathBuf]) -> Result<CommandSpec, String> {
    let id = raw.id;
    if !valid_command_id(&id) {
        return Err(format!("command id {id:?} must match [a-z0-9_-]{{1,40}}"));
    }
    let what = |m: &str| format!("command {id:?}: {m}");
    if raw.argv.is_empty() || raw.argv.len() > MAX_ARGV {
        return Err(what(&format!("argv must have 1 to {MAX_ARGV} entries")));
    }
    if raw
        .argv
        .iter()
        .any(|a| a.len() > MAX_ARG_BYTES || a.contains('\0'))
    {
        return Err(what("an argv entry is too long or holds a NUL"));
    }
    if !path_under(&raw.argv[0], prefixes) {
        return Err(what(&format!(
            "argv[0] {:?} must be an absolute path under {}",
            raw.argv[0],
            prefixes
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" or ")
        )));
    }
    let timeout_ms = raw.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(1..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(what(&format!(
            "timeout_ms must be 1 to {MAX_TIMEOUT_MS}, not {timeout_ms}"
        )));
    }
    if raw.args.len() > MAX_DECLARED_ARGS {
        return Err(what(&format!("at most {MAX_DECLARED_ARGS} args")));
    }
    let mut args: Vec<ArgSpec> = Vec::new();
    for a in raw.args {
        if args.iter().any(|o| o.name == a.name) {
            return Err(what(&format!("argument {:?} is declared twice", a.name)));
        }
        args.push(ArgSpec::new(&a.name, &a.pattern).map_err(|e| what(&e))?);
    }
    if let Some(text) = &raw.confirm {
        if !valid_text(text, MAX_CONFIRM_CHARS) {
            return Err(what(&format!(
                "confirm must be 1-{MAX_CONFIRM_CHARS} characters without control characters"
            )));
        }
        let placeholders = text.matches("%s").count();
        if placeholders > 1 || text.replacen("%s", "", 1).contains('%') {
            return Err(what(
                "confirm may hold one %s (the first argument) and no other %",
            ));
        }
        if placeholders == 1 && args.is_empty() {
            return Err(what("confirm has %s but the command declares no args"));
        }
    }
    Ok(CommandSpec {
        id,
        argv: raw.argv,
        output: raw.output,
        timeout_ms,
        args,
        confirm: raw.confirm,
    })
}

fn commands_from(raw: Vec<RawCommand>) -> Result<Vec<CommandSpec>, ManifestError> {
    if raw.len() > MAX_COMMANDS {
        return Err(invalid(format!(
            "at most {MAX_COMMANDS} [[command]] tables"
        )));
    }
    let prefixes = command_prefixes();
    let mut out: Vec<CommandSpec> = Vec::new();
    for c in raw {
        let c = command_from(c, &prefixes).map_err(invalid)?;
        if out.iter().any(|o| o.id == c.id) {
            return Err(invalid(format!("duplicate command id {:?}", c.id)));
        }
        out.push(c);
    }
    Ok(out)
}

/// Check `uq_api` before anything else. `required`: the key must be present (widgets); pages
/// predate it and default to 1.
fn check_api(file: &str, text: &str, required: bool) -> Result<(), ManifestError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|e| invalid(format!("{file}: {}", e.message().trim())))?;
    match table.get("uq_api") {
        None if required => Err(invalid(format!(
            "{file}: uq_api is missing (add uq_api = {UQ_API})"
        ))),
        None => Ok(()),
        Some(toml::Value::Integer(n)) if *n > i64::from(UQ_API) => {
            Err(ManifestError::NeedsNewerUq(*n))
        }
        Some(toml::Value::Integer(n)) if *n >= 1 => Ok(()),
        Some(other) => Err(invalid(format!(
            "{file}: uq_api must be a whole number from 1, not {other}"
        ))),
    }
}

fn strict<'de, T: Deserialize<'de>>(file: &str, text: &'de str) -> Result<T, ManifestError> {
    toml::from_str(text).map_err(|e| invalid(format!("{file}: {}", e.message().trim())))
}

fn check_name(name: &str) -> Result<(), ManifestError> {
    if valid_text(name, 100) {
        Ok(())
    } else {
        Err(invalid(
            "name must be 1-100 characters without control characters",
        ))
    }
}

fn check_qml(qml: &str) -> Result<(), ManifestError> {
    if valid_qml_name(qml) {
        Ok(())
    } else {
        Err(invalid(format!(
            "qml {qml:?} must be a bare file name ending .qml"
        )))
    }
}

/// `requires.bus_name` is watched on the `[dbus]` table's bus, which must be the same name.
fn check_requires_bus(requires: &Requires, dbus: Option<&DbusSpec>) -> Result<(), ManifestError> {
    match (&requires.bus_name, dbus) {
        (Some(r), Some(d)) if d.bus == Bus::System && *r != d.name => Err(invalid(
            "requires.bus_name on the system bus must be dbus.name",
        )),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------
// widget.toml

/// Where a widget shows (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    /// A 1-cell icon in the Status island.
    Bar,
    /// A tile in the Apps group of quick settings.
    Quick,
}

/// A valid `widget.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetManifest {
    pub id: String,
    pub name: String,
    pub slot: Slot,
    pub qml: String,
    /// 0-100; ties break on id.
    pub order: u8,
    pub uq_api: u32,
    pub requires: Requires,
    pub dbus: Option<DbusSpec>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWidget {
    id: String,
    name: String,
    slot: Slot,
    qml: String,
    order: Option<i64>,
    uq_api: u32,
    requires: Option<RawRequires>,
    dbus: Option<RawDbus>,
    #[serde(default)]
    command: Vec<RawCommand>,
}

/// Parse and validate a `widget.toml` (the folder name is checked by [`load_widget`]).
pub fn parse_widget(text: &str) -> Result<WidgetManifest, ManifestError> {
    const FILE: &str = "widget.toml";
    check_api(FILE, text, true)?;
    let m: RawWidget = strict(FILE, text)?;
    if !valid_id(&m.id) {
        return Err(invalid(format!(
            "id {:?} must match [a-z0-9-]{{1,40}}",
            m.id
        )));
    }
    check_name(&m.name)?;
    check_qml(&m.qml)?;
    let order = match m.order {
        None => 50,
        Some(o @ 0..=100) => o as u8,
        Some(o) => return Err(invalid(format!("order must be 0-100, not {o}"))),
    };
    let requires = requires_from(m.requires)?;
    let dbus = dbus_from(m.dbus)?;
    if dbus.as_ref().is_some_and(|d| !d.timeout_ms.is_empty()) {
        return Err(invalid(
            "dbus.timeout_ms is for settings pages only; a widget's calls end within 2 seconds",
        ));
    }
    check_requires_bus(&requires, dbus.as_ref())?;
    Ok(WidgetManifest {
        commands: commands_from(m.command)?,
        id: m.id,
        name: m.name,
        slot: m.slot,
        qml: m.qml,
        order,
        uq_api: m.uq_api,
        requires,
        dbus,
    })
}

// ---------------------------------------------------------------------------------------------
// page.toml

/// A searchable `[[setting]]` row of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSetting {
    pub id: String,
    pub label: String,
    pub keywords: Vec<String>,
}

/// A valid `page.toml` (Phase 3 §2.5 keys, plus `uq_api`, `[requires]`, `[dbus]` and
/// `[[command]]` from Phase 4 §2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageManifest {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub qml: String,
    /// Phase 3's `dbus_name`: the one bus name the page's `dbus` helper reaches. Set from
    /// `[dbus].name` when only the table is given.
    pub dbus_name: Option<String>,
    pub keywords: Vec<String>,
    pub settings: Vec<PageSetting>,
    pub uq_api: u32,
    pub requires: Requires,
    pub dbus: Option<DbusSpec>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    id: String,
    name: String,
    icon: Option<String>,
    category: Option<String>,
    qml: String,
    dbus_name: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    setting: Vec<RawSetting>,
    uq_api: Option<u32>,
    requires: Option<RawRequires>,
    dbus: Option<RawDbus>,
    #[serde(default)]
    command: Vec<RawCommand>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSetting {
    id: String,
    label: String,
    #[serde(default)]
    keywords: Vec<String>,
}

/// Parse and validate a `page.toml` (the folder name is checked by [`load_page`]).
pub fn parse_page(text: &str) -> Result<PageManifest, ManifestError> {
    const FILE: &str = "page.toml";
    check_api(FILE, text, false)?;
    let m: RawPage = strict(FILE, text)?;
    if !valid_id(&m.id) {
        return Err(invalid(format!(
            "id {:?} must match [a-z0-9-]{{1,40}}",
            m.id
        )));
    }
    if let Some(c) = &m.category
        && c != "apps"
    {
        return Err(invalid(format!("category must be \"apps\", not {c:?}")));
    }
    check_name(&m.name)?;
    if let Some(icon) = &m.icon
        && !valid_icon_name(icon)
    {
        return Err(invalid(format!(
            "icon {icon:?} must be an icon name matching [A-Za-z0-9._-]{{1,64}}"
        )));
    }
    check_qml(&m.qml)?;
    if let Some(n) = &m.dbus_name
        && !valid_bus_name(n)
    {
        return Err(invalid(format!("dbus_name {n:?} is not a valid bus name")));
    }
    if !valid_keywords(&m.keywords) {
        return Err(invalid("keywords: at most 32, each 1-64 characters"));
    }
    if m.setting.len() > MAX_SETTINGS {
        return Err(invalid(format!(
            "at most {MAX_SETTINGS} [[setting]] entries"
        )));
    }
    let mut settings: Vec<PageSetting> = Vec::new();
    for s in m.setting {
        if !valid_setting_id(&s.id) {
            return Err(invalid(format!(
                "setting id {:?} must match [A-Za-z0-9._-]{{1,64}}",
                s.id
            )));
        }
        if settings.iter().any(|o| o.id == s.id) {
            return Err(invalid(format!("duplicate setting id {:?}", s.id)));
        }
        if !valid_text(&s.label, 200) || !valid_keywords(&s.keywords) {
            return Err(invalid(format!(
                "setting {:?}: bad label or keywords",
                s.id
            )));
        }
        settings.push(PageSetting {
            id: s.id,
            label: s.label,
            keywords: s.keywords,
        });
    }
    let requires = requires_from(m.requires)?;
    let dbus = dbus_from(m.dbus)?;
    check_requires_bus(&requires, dbus.as_ref())?;
    let dbus_name = match (m.dbus_name, &dbus) {
        (Some(n), Some(d)) if n != d.name => {
            return Err(invalid(format!(
                "dbus_name {n:?} and dbus.name {:?} differ",
                d.name
            )));
        }
        (n, d) => n.or_else(|| d.as_ref().map(|d| d.name.clone())),
    };
    Ok(PageManifest {
        commands: commands_from(m.command)?,
        id: m.id,
        name: m.name,
        icon: m.icon,
        qml: m.qml,
        dbus_name,
        keywords: m.keywords,
        settings,
        uq_api: m.uq_api.unwrap_or(UQ_API),
        requires,
        dbus,
    })
}

// ---------------------------------------------------------------------------------------------
// Folders

/// A manifest folder read from disk: canonical paths only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded<M> {
    pub manifest: M,
    /// The folder, canonical.
    pub dir: PathBuf,
    /// The QML file, canonical, an existing regular file inside `dir`.
    pub qml_path: PathBuf,
}

/// Read `<dir>/<file>`. `root` is the canonical integration directory. A folder or manifest
/// that is a symlink resolving outside it is refused, as is a manifest over
/// [`MAX_MANIFEST_BYTES`]. Returns the canonical folder and the text.
///
/// This is hygiene, not a sandbox: integration QML can still `import "../x"` or read any file
/// the user can.
pub fn read_folder(dir: &Path, root: &Path, file: &str) -> Result<(PathBuf, String), String> {
    let dir = fs::canonicalize(dir).map_err(|e| format!("{e}"))?;
    if !dir.starts_with(root) {
        return Err("resolves outside the integration directory".into());
    }
    let manifest = fs::canonicalize(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    if !manifest.starts_with(&dir) {
        return Err(format!("{file} resolves outside its folder"));
    }
    let meta = fs::metadata(&manifest).map_err(|e| format!("{file}: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_MANIFEST_BYTES {
        return Err(format!("{file} is not a regular file under 64 KiB"));
    }
    let text = fs::read_to_string(&manifest).map_err(|e| format!("{file}: {e}"))?;
    Ok((dir, text))
}

/// The canonical path of `<dir>/<qml>`, which must exist, be a regular file and resolve inside
/// `dir` (canonical).
pub fn resolve_qml(dir: &Path, qml: &str) -> Result<PathBuf, String> {
    let path = fs::canonicalize(dir.join(qml)).map_err(|e| format!("qml {qml:?}: {e}"))?;
    if !path.starts_with(dir) || !path.is_file() {
        return Err("qml resolves outside its folder or is not a file".into());
    }
    Ok(path)
}

fn load<M>(
    dir: &Path,
    root: &Path,
    file: &str,
    parse: impl FnOnce(&str) -> Result<M, ManifestError>,
    id_qml: impl Fn(&M) -> (&str, &str),
) -> Result<Loaded<M>, ManifestError> {
    let folder_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (cdir, text) = read_folder(dir, root, file).map_err(invalid)?;
    let manifest = parse(&text)?;
    let (id, qml) = id_qml(&manifest);
    if id != folder_name {
        return Err(invalid(format!(
            "id {id:?} must equal the folder name {folder_name:?}"
        )));
    }
    let qml_path = resolve_qml(&cdir, qml).map_err(invalid)?;
    Ok(Loaded {
        manifest,
        dir: cdir,
        qml_path,
    })
}

/// Load `<dir>/widget.toml` and its QML file. `root`: the canonical widgets directory.
pub fn load_widget(dir: &Path, root: &Path) -> Result<Loaded<WidgetManifest>, ManifestError> {
    load(dir, root, "widget.toml", parse_widget, |m| {
        (m.id.as_str(), m.qml.as_str())
    })
}

/// Load `<dir>/page.toml` and its QML file. `root`: the canonical pages directory.
pub fn load_page(dir: &Path, root: &Path) -> Result<Loaded<PageManifest>, ManifestError> {
    load(dir, root, "page.toml", parse_page, |m| {
        (m.id.as_str(), m.qml.as_str())
    })
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
