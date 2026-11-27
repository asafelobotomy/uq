//! Settings profiles: a named overlay of a subset of uqd's settings, as a TOML file (spec §4).
//!
//! A profile holds set values only. `values` maps a domain to a JSON object tree that is never
//! empty. The same text is the stored file and the export, so `parse` is the one gate every file
//! passes: size, TOML, `format`, id, name, icon, domains, the power mode and every value.

mod active;
mod keybinds;
mod overlay;
mod rules;
mod startup;
mod store;

pub use active::{clear_active, load_active, save_active};
// `enabled_of`, `set_enabled` and `check_tree` are the rules' above, so the shortcuts' go by
// longer names.
pub use keybinds::{
    bind_keys, bind_label, bind_toggles, check_tree as check_bind_tree,
    enabled_of as bind_enabled_of, mark_binds, missing_binds, missing_binds_notice,
    set_enabled as set_bind_enabled,
};
pub use overlay::{
    ActiveProfile, Kept, KeyMemo, PowerMemo, apply, changed_since, from_journal, holds, next_after,
    turn_off,
};
pub use rules::{
    check_tree, enabled_of, mark_rules, missing_rules, missing_rules_notice, rule_keys, rule_label,
    rule_toggles, set_enabled,
};
pub use startup::{
    TOO_MANY_FIXES, TOO_MANY_VARS, check_autostart_tree, check_compat_tree, check_env_tree,
    emptied_elsewhere, kept_fix_notice, mark_startup, missing_startup, missing_startup_notice,
    startup_keys, startup_keys_of, startup_label, startup_notice,
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
mod leaves;
pub use leaves::*;
mod file;
pub use file::*;

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
    /// The on/off switch of existing keyboard shortcuts, by id (`keybinds.rs`).
    Keybinds,
    /// In the format, refused until a later version of uQ can apply it. No domain is this now.
    Reserved,
}

pub const PROFILE_DOMAINS: [(&str, DomainKind); 11] = [
    ("look", DomainKind::Section),
    ("layouts", DomainKind::Section),
    ("shell", DomainKind::Section),
    ("wallpaper", DomainKind::Section),
    ("appearance", DomainKind::Appearance),
    ("power", DomainKind::Power),
    ("keybinds", DomainKind::Keybinds),
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
    #[error("{}", TOO_MANY_FIXES)]
    TooManyFixes,
    #[error(
        "Turning the profile off would put back a variable it removed, making more than 100 \
         environment variables. Remove one on the Startup page first."
    )]
    OffTooManyVars,
    #[error(
        "Turning the profile off would put back an app fix it removed, making more than 100 app \
         fixes. Remove one on the Apps and scaling page first."
    )]
    OffTooManyFixes,
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
        if let Some(tree) = self.values.get("keybinds") {
            // Nor shortcuts of your own: only ids and on/off are checked here.
            keybinds::check_tree(tree)?;
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lookseedo_migration {
    use super::*;

    #[test]
    fn old_launcher_shortcut_key_moves() {
        let src = "format = 1\nid = \"p\"\nname = \"P\"\n[keybinds]\n\"uq.spotlight\" = false\n";
        let p = Profile::parse(src).unwrap();
        assert_eq!(
            p.values["keybinds"],
            serde_json::json!({"uq.lookseedo": false})
        );
    }
}
