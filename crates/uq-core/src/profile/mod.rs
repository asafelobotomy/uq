//! Settings profiles: a named overlay of a subset of uqd's settings, as a TOML file (spec §4).
//!
//! A profile holds set values only. `values` maps a domain to a JSON object tree that is never
//! empty. The same text is the stored file and the export, so `parse` is the one gate every file
//! passes: size, TOML, `format`, id, name, icon, domains, the power mode and every value.

mod active;
mod overlay;
mod rules;
mod startup;
mod store;

pub use active::{clear_active, load_active, save_active};
pub use overlay::{
    ActiveProfile, Kept, KeyMemo, PowerMemo, apply, changed_since, from_journal, holds, next_after,
    turn_off,
};
pub use rules::{
    check_tree, enabled_of, mark_rules, missing_rules, missing_rules_notice, rule_keys, rule_label,
    rule_toggles, set_enabled,
};
pub use startup::{
    TOO_MANY_VARS, check_autostart_tree, check_compat_tree, check_env_tree, mark_startup,
    missing_startup, missing_startup_notice, startup_keys, startup_keys_of, startup_label,
    startup_notice,
};
pub use store::ProfileStore;

use crate::history;
use crate::settings::{SettingError, valid_theme_name};
use crate::state::State;
use crate::theme::Variant;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

pub const FORMAT: i64 = 1;
pub const MAX_FILE_BYTES: u64 = 65_536;
pub const MAX_KEYS: usize = 500;
pub const MAX_PROFILES: usize = 50;
pub const MAX_NAME_CHARS: usize = 100;
pub const POWER_MODES: [&str; 3] = ["power-saver", "balanced", "performance"];
pub const HEADER: &str = "# A uQ settings profile. Import it on the Profiles page of uQ Settings, or with\n# `uqctl profile import <file>`.\n";

/// How a profile domain is read and written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainKind {
    /// A settings section (`State::section` / `State::set_section`).
    Section,
    /// The `appearance` section plus the leaves `theme` and `variant` (`State::theme`/`variant`).
    Appearance,
    /// `power.profile`, set through the system's power service.
    Power,
    /// The on/off switch of existing window and panel rules, by id (`rules.rs`).
    Rules,
    /// Environment variables by name: set to a value, or removed with `false` (`startup.rs`).
    Env,
    /// The Apps and scaling switches and each fix's platform and size (`startup.rs`).
    Compat,
    /// An existing startup entry's on/off and delay, by id (`startup.rs`).
    Autostart,
    /// In the format, refused until a later version of uQ can apply it.
    Reserved,
}

pub const PROFILE_DOMAINS: [(&str, DomainKind); 11] = [
    ("look", DomainKind::Section),
    ("layouts", DomainKind::Section),
    ("shell", DomainKind::Section),
    ("wallpaper", DomainKind::Section),
    ("appearance", DomainKind::Appearance),
    ("power", DomainKind::Power),
    ("keybinds", DomainKind::Reserved),
    ("rules", DomainKind::Rules),
    ("env", DomainKind::Env),
    ("compat", DomainKind::Compat),
    ("autostart", DomainKind::Autostart),
];

fn kind_of(domain: &str) -> Option<DomainKind> {
    PROFILE_DOMAINS
        .iter()
        .find(|(d, _)| *d == domain)
        .map(|(_, k)| *k)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub domain: String,
    pub path: Vec<String>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProfileError {
    #[error("The file is too big to be a profile.")]
    TooBig,
    #[error("Can't read the file: {0}")]
    Unreadable(String),
    #[error("This file isn't a uQ profile.")]
    NotAProfile,
    #[error("This profile was made by a newer version of uQ.")]
    NewerFormat(i64),
    #[error("The profile's id can only use a–z, 0–9 and dashes.")]
    BadId,
    #[error("A profile needs a name of 1 to 100 characters.")]
    BadName,
    #[error("The profile's icon name isn't valid.")]
    BadIcon,
    #[error("This profile has settings uQ doesn't know: {}.", plain(.0))]
    UnknownDomain(String),
    #[error(
        "This profile turns keyboard shortcuts on or off, which this version of uQ can't do yet."
    )]
    NeedsNewer(String),
    #[error("{}: {}", plain(&invalid_label(.key)), plain(.reason))]
    Invalid { key: String, reason: String },
    #[error("A profile needs at least one setting.")]
    Empty,
    #[error("A profile can hold up to 500 settings.")]
    TooManyKeys,
    #[error("{}", TOO_MANY_VARS)]
    TooManyVars,
}

/// Text from an imported file with its control characters left out, so an error can't carry a
/// terminal escape sequence to `uqctl`'s output.
fn plain(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// The plain label of a dotted `domain.path` key, for the text of an `Invalid` error.
fn invalid_label(key: &str) -> String {
    let Some((domain, rest)) = key.split_once('.') else {
        return key.to_string();
    };
    // Ids hold dots, so the path is cut from the ends: `apps.<id>.<field>`, `<id>.<field>`.
    let s = |v: &str| v.to_string();
    let path = match (domain, rest.strip_prefix("apps.")) {
        ("compat", Some(fix)) => match fix.rsplit_once('.') {
            Some((id, f)) if id.ends_with(".desktop") => vec![s("apps"), s(id), s(f)],
            _ => vec![s("apps")],
        },
        ("autostart", _) => match rest.rsplit_once('.') {
            Some((id, f)) if startup::startup_id_ok(id) => vec![s(id), s(f)],
            _ => vec![s(rest)],
        },
        _ => vec![s(rest)],
    };
    key_label(domain, &path)
}

fn invalid(key: impl Into<String>, reason: impl Into<String>) -> ProfileError {
    ProfileError::Invalid {
        key: key.into(),
        reason: reason.into(),
    }
}

impl Profile {
    /// Parse and fully validate profile text.
    pub fn parse(text: &str) -> Result<Profile, ProfileError> {
        if text.len() as u64 > MAX_FILE_BYTES {
            return Err(ProfileError::TooBig);
        }
        let table: toml::Table = text.parse().map_err(|_| ProfileError::NotAProfile)?;
        match table.get("format") {
            Some(toml::Value::Integer(n)) if *n > FORMAT => {
                return Err(ProfileError::NewerFormat(*n));
            }
            Some(toml::Value::Integer(n)) if *n == FORMAT => {}
            _ => return Err(ProfileError::NotAProfile),
        }
        let text_of = |key: &str| match table.get(key) {
            Some(toml::Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let id = text_of("id").ok_or(ProfileError::BadId)?;
        let name = text_of("name").ok_or(ProfileError::BadName)?;
        let icon = match table.get("icon") {
            None => None,
            Some(toml::Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(ProfileError::BadIcon),
        };
        let mut values = BTreeMap::new();
        for (key, v) in &table {
            if matches!(key.as_str(), "format" | "id" | "name" | "icon") {
                continue;
            }
            let json = toml_to_json(v).ok_or_else(|| invalid(key.clone(), "can't be used"))?;
            if json.as_object().is_some_and(Map::is_empty) {
                continue;
            }
            values.insert(key.clone(), json);
        }
        let profile = Profile {
            id,
            name,
            icon,
            values,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// The file text: `HEADER`, `format`, `id`, `name`, `icon`, then the domains in order.
    pub fn to_toml(&self) -> String {
        let mut out = String::from(HEADER);
        let mut scalar = |key: &str, v: toml::Value| {
            let mut t = toml::Table::new();
            t.insert(key.into(), v);
            out.push_str(&toml::to_string(&t).expect("a scalar serialises"));
        };
        scalar("format", toml::Value::Integer(FORMAT));
        scalar("id", toml::Value::String(self.id.clone()));
        scalar("name", toml::Value::String(self.name.clone()));
        if let Some(icon) = &self.icon {
            scalar("icon", toml::Value::String(icon.clone()));
        }
        for (domain, _) in PROFILE_DOMAINS {
            let Some(tree) = self.values.get(domain).and_then(json_to_toml) else {
                continue;
            };
            let mut t = toml::Table::new();
            t.insert(domain.into(), tree);
            out.push('\n');
            out.push_str(&toml::to_string(&t).expect("a table serialises"));
        }
        out
    }

    /// Everything `parse` checks after reading the text.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if !valid_id(&self.id) {
            return Err(ProfileError::BadId);
        }
        if !valid_name(&self.name) {
            return Err(ProfileError::BadName);
        }
        if self.icon.as_deref().is_some_and(|i| !valid_icon(i)) {
            return Err(ProfileError::BadIcon);
        }
        for domain in self.values.keys() {
            match kind_of(domain) {
                None => return Err(ProfileError::UnknownDomain(domain.clone())),
                Some(DomainKind::Reserved) => return Err(ProfileError::NeedsNewer(domain.clone())),
                Some(_) => {}
            }
        }
        if let Some(power) = self.values.get("power") {
            let ok = power.as_object().is_some_and(|m| {
                m.len() == 1
                    && m.get("profile")
                        .and_then(Value::as_str)
                        .is_some_and(|s| POWER_MODES.contains(&s))
            });
            if !ok {
                return Err(invalid(
                    "power.profile",
                    "must be Power saver, Balanced or Performance",
                ));
            }
        }
        for (domain, v) in &self.values {
            if !v.is_object() {
                return Err(invalid(domain.clone(), "must be a table of settings"));
            }
        }
        let count = self.keys().len();
        if count == 0 {
            return Err(ProfileError::Empty);
        }
        if count > MAX_KEYS {
            return Err(ProfileError::TooManyKeys);
        }
        if let Some(tree) = self.values.get("rules") {
            // Not written to a default `State`, which has no rules to switch.
            check_tree(tree)?;
        }
        // Nor these: a default `State` has no startup entries, and a variable's limit depends
        // on the computer it's applied on.
        for (domain, tree) in &self.values {
            match kind_of(domain) {
                Some(DomainKind::Env) => check_env_tree(tree)?,
                Some(DomainKind::Compat) => check_compat_tree(tree)?,
                Some(DomainKind::Autostart) => check_autostart_tree(tree)?,
                _ => {}
            }
        }
        for (domain, kind) in PROFILE_DOMAINS {
            if !matches!(kind, DomainKind::Section | DomainKind::Appearance) {
                continue;
            }
            let Some(tree) = self.values.get(domain) else {
                continue;
            };
            let all = leaves(tree);
            let mut state = State::default();
            let to_write: Vec<_> = all
                .iter()
                .map(|(p, v)| (p.clone(), Some(v.clone())))
                .collect();
            if let Err(e) = write_leaves(&mut state, domain, &to_write) {
                return Err(blame(domain, &all, e));
            }
            for (path, _) in &all {
                if read_leaf(&state, domain, path).is_none() {
                    return Err(invalid(
                        format!("{domain}.{}", path.join(".")),
                        "not a setting uQ knows",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every key, in `PROFILE_DOMAINS` order.
    pub fn keys(&self) -> Vec<Key> {
        PROFILE_DOMAINS
            .iter()
            .filter_map(|(d, _)| self.values.get(*d).map(|v| (*d, v)))
            .flat_map(|(domain, tree)| {
                leaves(tree).into_iter().map(move |(path, value)| Key {
                    domain: domain.to_string(),
                    path,
                    value,
                })
            })
            .collect()
    }

    pub fn with_key(&mut self, key: Key) {
        let tree = self
            .values
            .entry(key.domain)
            .or_insert_with(|| Value::Object(Map::new()));
        history::put_path(tree, &key.path, Some(key.value));
    }

    /// Remove a key; the domain goes with its last key. True when the key was there.
    pub fn without_key(&mut self, domain: &str, path: &[String]) -> bool {
        let Some(tree) = self.values.get_mut(domain) else {
            return false;
        };
        let had = history::get_path(tree, path).is_some_and(|v| !v.is_object());
        history::put_path(tree, path, None);
        if tree.as_object().is_none_or(Map::is_empty) {
            self.values.remove(domain);
        }
        had
    }

    pub fn power(&self) -> Option<&str> {
        self.values.get("power")?.get("profile")?.as_str()
    }
}

/// Name the leaf a domain-wide write refused: the first one that fails alone on a fresh state.
fn blame(domain: &str, all: &[(Vec<String>, Value)], e: SettingError) -> ProfileError {
    for (path, v) in all {
        if let Err(e) = write_leaves(
            &mut State::default(),
            domain,
            &[(path.clone(), Some(v.clone()))],
        ) {
            return invalid(format!("{domain}.{}", path.join(".")), reason_of(&e));
        }
    }
    invalid(domain, reason_of(&e))
}

fn reason_of(e: &SettingError) -> String {
    match e {
        SettingError::Invalid { reason, .. } => reason.clone(),
        SettingError::UnknownKey(_) => "not a setting uQ knows".into(),
    }
}

fn valid_name(name: &str) -> bool {
    let n = name.chars().count();
    (1..=MAX_NAME_CHARS).contains(&n) && !name.chars().any(char::is_control)
}

fn valid_icon(icon: &str) -> bool {
    (1..=64).contains(&icon.len())
        && icon
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `^[a-z0-9][a-z0-9-]{0,31}$`
pub fn valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
}

/// A file-name-safe id from a profile name, different from every id in `taken`.
pub fn id_for_name(name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    slug.truncate(32);
    let mut base = slug.trim_end_matches('-').to_string();
    if base.is_empty() {
        base = "profile".into();
    }
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| {
            let suffix = format!("-{n}");
            let keep = 32 - suffix.len();
            let stem: String = base.chars().take(keep).collect();
            format!("{}{suffix}", stem.trim_end_matches('-'))
        })
        .find(|id| !taken.contains(id))
        .expect("an endless range has a free id")
}

/// Object leaves by path segment: arrays and scalars are leaves, an empty object is none.
pub fn leaves(v: &Value) -> Vec<(Vec<String>, Value)> {
    let mut out = Vec::new();
    history::segment_leaves(v, &mut Vec::new(), &mut out);
    out
}

/// The value of one leaf as `domain` holds it in `state`; `None` when it's unset or unknown.
pub fn read_leaf(state: &State, domain: &str, path: &[String]) -> Option<Value> {
    let v = match (kind_of(domain)?, path) {
        (DomainKind::Appearance, [leaf]) if leaf == "theme" => Value::String(state.theme.clone()),
        (DomainKind::Appearance, [leaf]) if leaf == "variant" => {
            Value::String(state.variant.as_str().into())
        }
        (DomainKind::Section | DomainKind::Appearance, _) => {
            history::get_path(&state.section(domain).ok()?, path)?.clone()
        }
        (DomainKind::Rules, [id]) => Value::Bool(enabled_of(state, id)?),
        (DomainKind::Env | DomainKind::Compat | DomainKind::Autostart, _) => {
            startup::read_leaf(state, domain, path)?
        }
        _ => return None,
    };
    (!v.is_null() && !v.is_object()).then_some(v)
}

/// Write every leaf of one domain through a single `State::set_section` (`None` removes it).
/// On an error `state` is unchanged.
pub fn write_leaves(
    state: &mut State,
    domain: &str,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let kind = kind_of(domain).ok_or_else(|| SettingError::UnknownKey(domain.to_string()))?;
    if kind == DomainKind::Rules {
        return write_rule_leaves(state, leaves);
    }
    if matches!(
        kind,
        DomainKind::Env | DomainKind::Compat | DomainKind::Autostart
    ) {
        return startup::write_leaves(state, domain, leaves);
    }
    let (mut theme, mut variant) = (None, None);
    let mut section_leaves = Vec::new();
    for (path, value) in leaves {
        let special = kind == DomainKind::Appearance
            && matches!(path.as_slice(), [p] if p == "theme" || p == "variant");
        if !special {
            section_leaves.push((path, value));
            continue;
        }
        let key = format!("{domain}.{}", path[0]);
        let text = value
            .as_ref()
            .and_then(Value::as_str)
            .ok_or_else(|| SettingError::Invalid {
                key: key.clone(),
                reason: "must be set to a name".into(),
            })?;
        if path[0] == "theme" {
            if !valid_theme_name(text) {
                return Err(SettingError::Invalid {
                    key,
                    reason: "isn't a valid theme name".into(),
                });
            }
            theme = Some(text.to_string());
        } else {
            variant = Some(match text {
                "dark" => Variant::Dark,
                "light" => Variant::Light,
                _ => {
                    return Err(SettingError::Invalid {
                        key,
                        reason: "must be dark or light".into(),
                    });
                }
            });
        }
    }
    if !matches!(kind, DomainKind::Section | DomainKind::Appearance) {
        return Err(SettingError::UnknownKey(domain.to_string()));
    }
    if !section_leaves.is_empty() {
        let mut section = state.section(domain)?;
        for (path, value) in section_leaves {
            history::put_path(&mut section, path, value.clone());
        }
        state.set_section(domain, section)?;
    }
    if let Some(t) = theme {
        state.theme = t;
    }
    if let Some(v) = variant {
        state.variant = v;
    }
    Ok(())
}

/// Set rule switches. A non-bool, a removal, or an id with no rule is refused (callers leave
/// out ids that aren't here); on an error `state` is unchanged.
fn write_rule_leaves(
    state: &mut State,
    leaves: &[(Vec<String>, Option<Value>)],
) -> Result<(), SettingError> {
    let mut next = state.clone();
    for (path, value) in leaves {
        let key = format!("rules.{}", plain(&path.join(".")));
        let refuse = |reason: &str| SettingError::Invalid {
            key: key.clone(),
            reason: reason.into(),
        };
        let ([id], Some(Value::Bool(on))) = (path.as_slice(), value) else {
            return Err(refuse(rules::ONLY_ON_OFF));
        };
        if !set_enabled(&mut next, id, *on) {
            return Err(refuse("isn't a rule on this computer"));
        }
    }
    *state = next;
    Ok(())
}

/// `("look", ["gaps_in"])` → "Gaps between windows". A rule switch is "A rule" here: naming it
/// needs the rules ([`rule_label`]).
pub fn key_label(domain: &str, path: &[String]) -> String {
    let dotted = path.join(".");
    match (domain, dotted.as_str()) {
        ("rules", _) if path.len() == 1 => Some("A rule".to_string()),
        ("appearance", "theme" | "variant") => history::label("theme", &dotted),
        ("power", "profile") => Some("Power mode".to_string()),
        ("env" | "compat" | "autostart", _) => Some(startup::fallback_label(domain, path)),
        _ => history::label(domain, &dotted),
    }
    .unwrap_or(dotted)
}

pub fn power_label(mode: &str) -> &str {
    match mode {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

/// A value as a person reads it.
pub fn value_text(domain: &str, path: &[String], v: &Value) -> String {
    if let Some(text) = startup::startup_value_text(domain, path, v) {
        return text;
    }
    match v {
        Value::String(s) if domain == "power" && path == ["profile"] => power_label(s).to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(true) => "On".into(),
        Value::Bool(false) => "Off".into(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => format!("{} items", a.len()),
        other => other.to_string(),
    }
}

pub fn power_notice(reason: &str) -> String {
    format!("Power mode wasn't changed: {reason}.")
}

/// The text of a profile file, read at most `MAX_FILE_BYTES` deep.
pub fn read_file(path: &Path) -> Result<String, ProfileError> {
    let unreadable = |e: std::io::Error| ProfileError::Unreadable(e.to_string());
    let file = std::fs::File::open(path).map_err(unreadable)?;
    if file.metadata().map_err(unreadable)?.len() > MAX_FILE_BYTES {
        return Err(ProfileError::TooBig);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ProfileError::TooBig);
    }
    String::from_utf8(bytes).map_err(|_| ProfileError::NotAProfile)
}

fn toml_to_json(v: &toml::Value) -> Option<Value> {
    Some(match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(n) => Value::from(*n),
        toml::Value::Float(f) => Value::Number(serde_json::Number::from_f64(*f)?),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(_) => return None,
        toml::Value::Array(a) => Value::Array(a.iter().map(toml_to_json).collect::<Option<_>>()?),
        toml::Value::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| Some((k.clone(), toml_to_json(v)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

fn json_to_toml(v: &Value) -> Option<toml::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64()?),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(a) => toml::Value::Array(a.iter().filter_map(json_to_toml).collect()),
        Value::Object(m) => toml::Value::Table(
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), json_to_toml(v)?)))
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GAMING: &str = r##"format = 1
id = "gaming"
name = "Gaming"
icon = "gamepad"

[look]
gaps_in = 0
gaps_out = 0
blur = false

[shell.islands]
time = false

[appearance]
variant = "dark"

[power]
profile = "performance"
"##;

    fn err(text: &str) -> ProfileError {
        Profile::parse(text).unwrap_err()
    }

    fn with(body: &str) -> String {
        format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
    }

    #[test]
    fn parse_spec_example() {
        let p = Profile::parse(GAMING).unwrap();
        assert_eq!(p.id, "gaming");
        assert_eq!(p.name, "Gaming");
        assert_eq!(p.icon.as_deref(), Some("gamepad"));
        assert_eq!(p.keys().len(), 6);
        assert_eq!(p.power(), Some("performance"));
        assert_eq!(p.values["look"]["gaps_in"], json!(0));
    }

    #[test]
    fn to_toml_round_trips() {
        let mut p = Profile::parse(GAMING).unwrap();
        p.with_key(Key {
            domain: "wallpaper".into(),
            path: ["outputs", "HDMI-A.1", "image"].map(String::from).to_vec(),
            value: json!("/a.png"),
        });
        p.with_key(Key {
            domain: "look".into(),
            path: vec!["animation_speed".into()],
            value: json!(2.0),
        });
        let text = p.to_toml();
        assert!(text.starts_with(HEADER), "{text}");
        assert!(text.contains("format = 1"), "{text}");
        assert!(text.contains("HDMI-A.1"), "{text}");
        assert_eq!(Profile::parse(&text), Ok(p));
    }

    #[test]
    fn refuses_missing_or_newer_format() {
        assert_eq!(
            err("id = \"x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
            ProfileError::NotAProfile
        );
        assert_eq!(
            err("format = 2\nid = \"x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
            ProfileError::NewerFormat(2)
        );
        assert_eq!(err("this is [not toml"), ProfileError::NotAProfile);
    }

    #[test]
    fn refuses_unknown_domain() {
        for d in ["monitors", "input", "settings"] {
            let e = err(&with(&format!("[look]\ngaps_in = 1\n[{d}]\na = 1\n")));
            assert_eq!(e, ProfileError::UnknownDomain(d.into()), "{d}");
        }
    }

    #[test]
    fn refuses_reserved_domains_for_now() {
        assert_eq!(
            err(&with("[keybinds]\n\"uq.spotlight\" = false\n")),
            ProfileError::NeedsNewer("keybinds".into())
        );
        assert!(
            ProfileError::NeedsNewer("keybinds".into())
                .to_string()
                .contains("keyboard shortcuts")
        );
    }

    #[test]
    fn refuses_invalid_values() {
        let e = err(&with("[look]\ngaps_in = 999\n"));
        assert!(
            matches!(&e, ProfileError::Invalid { key, .. } if key == "look.gaps_in"),
            "{e:?}"
        );
        assert!(e.to_string().starts_with("Gaps between windows: "), "{e}");
        assert!(matches!(
            err(&with("[look]\nbogus = 1\n")),
            ProfileError::Invalid { .. }
        ));
        let e = err(&with("[power]\nprofile = \"turbo\"\n"));
        assert!(
            matches!(&e, ProfileError::Invalid { key, .. } if key == "power.profile"),
            "{e:?}"
        );
        let e = err(&with("[appearance]\nvariant = \"dim\"\n"));
        assert!(
            matches!(&e, ProfileError::Invalid { key, .. } if key == "appearance.variant"),
            "{e:?}"
        );
        assert!(matches!(
            err(&with("[power]\nprofile = \"balanced\"\nother = 1\n")),
            ProfileError::Invalid { .. }
        ));
    }

    #[test]
    fn refuses_hostile_files() {
        let big = "# padding padding padding\n".repeat(65 * 1024 / 25 + 10);
        assert!(big.len() > 65_536);
        assert_eq!(err(&big), ProfileError::TooBig);
        assert_eq!(
            err("format = 1\nid = \"../x\"\nname = \"X\"\n[look]\ngaps_in = 1\n"),
            ProfileError::BadId
        );
        let long = "n".repeat(101);
        for name in [long.as_str(), "bell\u{7}", ""] {
            let t = format!(
                "format = 1\nid = \"x\"\nname = {}\n[look]\ngaps_in = 1\n",
                toml::Value::String(name.into())
            );
            assert_eq!(err(&t), ProfileError::BadName, "{name:?}");
        }
        assert_eq!(
            err(&with("icon = \"a/b\"\n[look]\ngaps_in = 1\n")),
            ProfileError::BadIcon
        );
        let deep_header = format!("[{}]\na = 1\n", vec!["a"; 5000].join("."));
        assert!(Profile::parse(&with(&deep_header)).is_err());
        let inline = format!("{}1{}", "{a=".repeat(5000), "}".repeat(5000));
        assert!(Profile::parse(&with(&format!("[look]\ngaps_in = {inline}\n"))).is_err());
        let arrays = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
        assert!(Profile::parse(&with(&format!("[look]\ngaps_in = {arrays}\n"))).is_err());
    }

    #[test]
    fn errors_never_repeat_control_characters_from_the_file() {
        for body in [
            "[\"\\u001b[31mx\"]\na = 1\n",
            "[look]\n\"\\u001b]0;pwn\\u0007\" = 1\n",
            "[look]\ngaps_in = \"\\u001b[2J\"\n",
            "[shell.\"\\u009b6n\"]\na = 1\n",
        ] {
            let text = err(&with(body)).to_string();
            assert!(
                !text.chars().any(char::is_control),
                "{body:?} gave {text:?}"
            );
            assert!(text.len() > 10, "{text:?}");
        }
    }

    #[test]
    fn too_many_keys_is_refused() {
        let mut p = Profile::parse(&with("[look]\ngaps_in = 1\n")).unwrap();
        p.values.insert(
            "shell".into(),
            serde_json::from_str(&format!(
                "{{{}}}",
                (0..501)
                    .map(|i| format!("\"k{i}\":1"))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
            .unwrap(),
        );
        assert_eq!(p.validate(), Err(ProfileError::TooManyKeys));
    }

    #[test]
    fn profile_holds_startup_domains() {
        let body = "[env]\nFOO = \"bar\"\nOLD = false\n[compat]\nsharp_x11 = true\n\
                    [compat.apps.\"code.desktop\"]\nplatform = \"x11\"\nscale = 1.5\n\
                    [autostart.\"nm-applet.desktop\"]\nenabled = false\n\
                    [autostart.\"a-3f9c\"]\ndelay = 10\n";
        let p = Profile::parse(&with(body)).unwrap();
        assert_eq!(p.keys().len(), 7);
        assert_eq!(Profile::parse(&p.to_toml()), Ok(p.clone()));
        // 501 keys across the three is too many, as anywhere.
        let mut big = p;
        let vars: Map<String, Value> = (0..300).map(|i| (format!("V{i}"), json!("x"))).collect();
        big.values.insert("env".into(), Value::Object(vars));
        let ids: Map<String, Value> = (0..197)
            .map(|i| (format!("e{i}.desktop"), json!({"enabled": true})))
            .collect();
        big.values.insert("autostart".into(), Value::Object(ids));
        assert_eq!(big.keys().len(), 300 + 3 + 197);
        assert_eq!(big.validate(), Ok(()));
        let mut over = big.clone();
        over.with_key(Key {
            domain: "autostart".into(),
            path: vec!["e0.desktop".into(), "delay".into()],
            value: json!(1),
        });
        assert_eq!(over.validate(), Err(ProfileError::TooManyKeys));
    }

    #[test]
    fn empty_profile_is_refused() {
        assert_eq!(err(&with("")), ProfileError::Empty);
        assert_eq!(err(&with("[look]\n")), ProfileError::Empty);
    }

    #[test]
    fn write_leaves_sets_and_removes() {
        let mut st = State::default();
        write_leaves(
            &mut st,
            "appearance",
            &[
                (vec!["theme".into()], Some(json!("my-theme"))),
                (vec!["variant".into()], Some(json!("light"))),
                (vec!["font_size".into()], Some(json!(12))),
            ],
        )
        .unwrap();
        assert_eq!(st.theme, "my-theme");
        assert_eq!(st.variant, Variant::Light);
        assert_eq!(
            read_leaf(&st, "appearance", &["font_size".into()]),
            Some(json!(12))
        );
        assert_eq!(
            read_leaf(&st, "appearance", &["theme".into()]),
            Some(json!("my-theme"))
        );
        write_leaves(&mut st, "appearance", &[(vec!["font_size".into()], None)]).unwrap();
        assert_eq!(read_leaf(&st, "appearance", &["font_size".into()]), None);
        let before = st.clone();
        assert!(
            write_leaves(
                &mut st,
                "appearance",
                &[(vec!["theme".into()], Some(json!("../x")))]
            )
            .is_err()
        );
        assert!(
            write_leaves(
                &mut st,
                "look",
                &[(vec!["gaps_in".into()], Some(json!(999)))]
            )
            .is_err()
        );
        assert_eq!(st, before);
    }

    #[test]
    fn id_for_name_slugs_and_dedups() {
        assert_eq!(id_for_name("Gaming", &[]), "gaming");
        assert_eq!(id_for_name("Gaming", &["gaming".into()]), "gaming-2");
        assert_eq!(
            id_for_name("Gaming", &["gaming".into(), "gaming-2".into()]),
            "gaming-3"
        );
        assert_eq!(id_for_name("!!!", &[]), "profile");
        assert_eq!(id_for_name("  Work & Play!  ", &[]), "work-play");
        let long = id_for_name(&"a".repeat(80), &[]);
        assert_eq!(long.len(), 32);
        assert!(valid_id(&id_for_name(&"a".repeat(80), &[long])));
        assert!(!valid_id("../x") && !valid_id("") && !valid_id("A") && !valid_id("-a"));
    }

    #[test]
    fn key_label_names_every_kind() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(key_label("appearance", &s(&["theme"])), "Theme");
        assert_eq!(key_label("appearance", &s(&["variant"])), "Light or dark");
        assert_eq!(key_label("power", &s(&["profile"])), "Power mode");
        assert_eq!(key_label("look", &s(&["gaps_in"])), "Gaps between windows");
        assert_eq!(
            key_label("shell", &s(&["islands", "time"])),
            "Bar island: Clock"
        );
        assert_eq!(power_label("power-saver"), "Power saver");
        assert_eq!(power_label("balanced"), "Balanced");
        assert_eq!(power_label("performance"), "Performance");
        assert_eq!(value_text("look", &s(&["blur"]), &json!(true)), "On");
        assert_eq!(value_text("look", &s(&["blur"]), &json!(false)), "Off");
        assert_eq!(value_text("look", &s(&["gaps_in"]), &json!(5)), "5");
        assert_eq!(
            value_text("shell", &s(&["providers"]), &json!(["a", "b"])),
            "2 items"
        );
        assert_eq!(
            value_text("power", &s(&["profile"]), &json!("balanced")),
            "Balanced"
        );
        assert_eq!(
            value_text("appearance", &s(&["ui_font"]), &json!("Inter")),
            "Inter"
        );
        assert_eq!(
            power_notice("it isn't available"),
            "Power mode wasn't changed: it isn't available."
        );
    }

    #[test]
    fn leaves_descends_objects_only() {
        let v = json!({"a": {"b": 1, "c": []}, "d": {}, "e": [1, 2]});
        let got = leaves(&v);
        assert_eq!(
            got,
            vec![
                (vec!["a".to_string(), "b".into()], json!(1)),
                (vec!["a".to_string(), "c".into()], json!([])),
                (vec!["e".to_string()], json!([1, 2])),
            ]
        );
    }

    #[test]
    fn without_key_prunes_empty_domains() {
        let mut p = Profile::parse(GAMING).unwrap();
        let path = vec!["profile".to_string()];
        assert!(p.without_key("power", &path));
        assert!(!p.values.contains_key("power"));
        assert!(!p.without_key("power", &path));
        assert_eq!(p.power(), None);
    }

    #[test]
    fn read_file_refuses_big_and_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.toml");
        std::fs::write(&big, vec![b'#'; 1 << 20]).unwrap();
        assert_eq!(read_file(&big), Err(ProfileError::TooBig));
        let bin = dir.path().join("bin.toml");
        std::fs::write(&bin, [0xff, 0xfe, 0x00, 0x80]).unwrap();
        assert_eq!(read_file(&bin), Err(ProfileError::NotAProfile));
        let ok = dir.path().join("ok.toml");
        std::fs::write(&ok, GAMING).unwrap();
        assert_eq!(read_file(&ok).unwrap(), GAMING);
        assert!(matches!(
            read_file(&dir.path().join("none.toml")),
            Err(ProfileError::Unreadable(_))
        ));
    }
}
