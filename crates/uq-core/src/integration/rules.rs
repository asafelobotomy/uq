//! Window rules that integrations ship, and the `integration_rules` domain that turns them on
//! (Phase 4 spec §3.3, "Rules from apps").
//!
//! An integration may install `/usr/share/uq/integrations/<name>/rules.toml` with
//! `[[window_rule]]` entries. Nothing in it applies until the person turns a rule on in Settings:
//! uqd keeps the list of rule ids that are on (`integration_rules.enabled`, empty by default),
//! reads the packaged file at every render, checks it against the closed schema below, and
//! renders each rule that is on through `lua_string` like the person's own rules. A packaged
//! file is never Lua and never reaches `generated.lua` as text.
//!
//! The schema, every key optional unless marked:
//!
//! ```toml
//! uq_api = 1                       # optional; a newer one is refused with "needs a newer uQ"
//! [[window_rule]]
//! id = "romforge.emulators"        # required: "<folder name>.<a-z0-9->{1,40}"
//! name = "Games open full screen"  # what Settings shows; 1-100 characters
//! class = ["^(retroarch)$"]        # required: 1-8 app ID patterns, Hyprland's syntax
//! title = "^.*$"                   # one window title pattern
//! workspace = "name:game"          # "name:<A-Za-z0-9 _->{1,32}", or a number 1-100
//! fullscreen = true
//! float = false
//! ```
//!
//! At least one of `workspace`, `fullscreen` and `float` must be set. A file has at most 16
//! rules. Any problem refuses the whole file, so a half-valid file never half-applies.
//!
//! An id that is on but whose file (or rule) is gone or no longer valid is skipped at render and
//! stays in the list, so the rule comes back on when the app is installed again. Settings lists
//! such ids as missing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use crate::integration::manifest::{ManifestError, UQ_API, read_folder, valid_id, valid_text};
use crate::rules::{
    WindowEffects, WindowMatch, WindowRule, check_pattern, effects_text, match_text,
};
use crate::settings::{SettingError, check_known, invalid as setting_invalid};

/// The file an integration folder may hold.
pub const RULES_FILE: &str = "rules.toml";
/// `[[window_rule]]` entries in one file.
pub const MAX_RULES_PER_FILE: usize = 16;
/// Patterns in one rule's `class` list.
pub const MAX_CLASS_PATTERNS: usize = 8;
/// Rule ids that can be on at once.
pub const MAX_ENABLED: usize = 64;
/// The longest workspace name after `name:`.
pub const MAX_WORKSPACE_NAME: usize = 32;
/// The highest workspace number a rule may name.
pub const MAX_WORKSPACE_NUMBER: i64 = 100;
/// The domain's name in `state.toml`, `GetSection` and the journal.
pub const DOMAIN: &str = "integration_rules";
/// Hyprland rule names of packaged rules start with this (the person's own use `uq:`).
pub const RULE_NAME_PREFIX: &str = "app:";

fn invalid(msg: impl Into<String>) -> ManifestError {
    ManifestError::Invalid(msg.into())
}

/// `<integration>.<rule>`, both `[a-z0-9-]{1,40}`.
pub fn valid_rule_id(id: &str) -> bool {
    id.split_once('.')
        .is_some_and(|(name, rule)| valid_id(name) && valid_id(rule))
}

/// The integration a rule id belongs to (`romforge` for `romforge.emulators`).
pub fn integration_of(id: &str) -> &str {
    id.split_once('.').map_or(id, |(name, _)| name)
}

/// One valid packaged rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppRule {
    pub id: String,
    /// The folder the file is in: the integration's name.
    pub integration: String,
    pub name: Option<String>,
    pub class: Vec<String>,
    pub title: Option<String>,
    /// A Hyprland workspace selector: `name:<name>` or a number.
    pub workspace: Option<String>,
    pub fullscreen: Option<bool>,
    pub float: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    uq_api: Option<toml::Value>,
    #[serde(default)]
    window_rule: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    name: Option<String>,
    class: Vec<String>,
    title: Option<String>,
    workspace: Option<toml::Value>,
    fullscreen: Option<bool>,
    float: Option<bool>,
}

/// A workspace a packaged rule may send windows to, as Hyprland's selector.
fn workspace_from(raw: toml::Value) -> Result<String, String> {
    const WHAT: &str = "workspace must be \"name:<name>\" (letters, digits, spaces, _ and -, up to \
                        32) or a number from 1 to 100";
    let number = |n: i64| {
        (1..=MAX_WORKSPACE_NUMBER)
            .contains(&n)
            .then(|| n.to_string())
            .ok_or_else(|| WHAT.to_string())
    };
    match raw {
        toml::Value::Integer(n) => number(n),
        toml::Value::String(t) if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) => t
            .parse::<i64>()
            .map_err(|_| WHAT.to_string())
            .and_then(number),
        toml::Value::String(t) => {
            let name = t.strip_prefix("name:").ok_or_else(|| WHAT.to_string())?;
            let ok = (1..=MAX_WORKSPACE_NAME).contains(&name.len())
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'_' | b'-'))
                && name.trim() == name;
            ok.then(|| t.clone()).ok_or_else(|| WHAT.to_string())
        }
        _ => Err(WHAT.to_string()),
    }
}

/// The plain reason of a pattern refusal, with the key it names.
fn pattern(key: &str, p: &str) -> Result<(), String> {
    check_pattern(key, p).map_err(|e| match e {
        SettingError::Invalid { key, reason } => format!("{key}: {reason}"),
        other => other.to_string(),
    })
}

fn check_rule(r: RawRule, at: &str, integration: &str) -> Result<AppRule, String> {
    if !valid_rule_id(&r.id) || integration_of(&r.id) != integration {
        return Err(format!(
            "{at}: id {:?} must be \"{integration}.<rule>\", the rule part a-z, 0-9 and -, up \
             to 40",
            r.id
        ));
    }
    let at = format!("window_rule {:?}", r.id);
    if let Some(n) = &r.name
        && !valid_text(n, 100)
    {
        return Err(format!(
            "{at}: name must be 1-100 characters without control characters"
        ));
    }
    if r.class.is_empty() || r.class.len() > MAX_CLASS_PATTERNS {
        return Err(format!("{at}: class must list 1 to 8 patterns"));
    }
    for (i, p) in r.class.iter().enumerate() {
        pattern(&format!("{at}: class[{i}]"), p)?;
    }
    if let Some(t) = &r.title {
        pattern(&format!("{at}: title"), t)?;
    }
    let workspace = r
        .workspace
        .map(workspace_from)
        .transpose()
        .map_err(|e| format!("{at}: {e}"))?;
    if workspace.is_none() && r.fullscreen.is_none() && r.float.is_none() {
        return Err(format!(
            "{at}: set at least one of workspace, fullscreen and float"
        ));
    }
    Ok(AppRule {
        id: r.id,
        integration: integration.to_string(),
        name: r.name,
        class: r.class,
        title: r.title,
        workspace,
        fullscreen: r.fullscreen,
        float: r.float,
    })
}

/// Parse and validate `rules.toml` of the integration folder `integration`.
pub fn parse_rules(text: &str, integration: &str) -> Result<Vec<AppRule>, ManifestError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| {
        let line = e
            .span()
            .map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1);
        let at = line.map_or(String::new(), |n| format!(" line {n}"));
        invalid(format!("{RULES_FILE}{at}: {}", e.message().trim()))
    })?;
    match raw.uq_api {
        None => {}
        Some(toml::Value::Integer(n)) if n > i64::from(UQ_API) => {
            return Err(ManifestError::NeedsNewerUq(n));
        }
        Some(toml::Value::Integer(n)) if n >= 1 => {}
        Some(other) => {
            return Err(invalid(format!(
                "{RULES_FILE}: uq_api must be a whole number from 1, not {other}"
            )));
        }
    }
    if raw.window_rule.is_empty() {
        return Err(invalid(format!("{RULES_FILE} has no [[window_rule]]")));
    }
    if raw.window_rule.len() > MAX_RULES_PER_FILE {
        return Err(invalid(format!(
            "{RULES_FILE} has more than {MAX_RULES_PER_FILE} rules"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (i, r) in raw.window_rule.into_iter().enumerate() {
        let rule = check_rule(r, &format!("window_rule[{i}]"), integration).map_err(invalid)?;
        if !seen.insert(rule.id.clone()) {
            return Err(invalid(format!("id {:?} is used twice", rule.id)));
        }
        out.push(rule);
    }
    Ok(out)
}

/// Load `<dir>/rules.toml`. `root` is the canonical integrations directory; the folder name is
/// the integration's name, `[a-z0-9-]{1,40}`. Symlinks out of `root` and files over 64 KiB are
/// refused as for manifests.
pub fn load_rules(dir: &Path, root: &Path) -> Result<Vec<AppRule>, ManifestError> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !valid_id(&name) {
        return Err(invalid(format!(
            "folder name {name:?} must match [a-z0-9-]{{1,40}}"
        )));
    }
    let (_, text) = read_folder(dir, root, RULES_FILE).map_err(invalid)?;
    parse_rules(&text, &name)
}

/// One integration folder that holds a `rules.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFile {
    pub integration: String,
    /// The `rules.toml` file.
    pub path: PathBuf,
    pub rules: Result<Vec<AppRule>, ManifestError>,
}

/// Every folder of `root` (`/usr/share/uq/integrations`) with a `rules.toml`, in name order.
/// Folders without one are left out. A missing root is empty; one that can't be read is an error.
pub fn scan_rules(root: &Path) -> Result<Vec<RuleFile>, String> {
    let entries = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", root.display())),
    };
    let croot = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && std::fs::symlink_metadata(p.join(RULES_FILE)).is_ok())
        .collect();
    dirs.sort();
    Ok(dirs
        .into_iter()
        .map(|d| RuleFile {
            integration: d
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: d.join(RULES_FILE),
            rules: load_rules(&d, &croot),
        })
        .collect())
}

/// Every valid packaged rule under `root`, in folder then file order. A file that is refused
/// costs only its own rules, with one log line.
pub fn installed(root: &Path) -> Vec<AppRule> {
    let files = match scan_rules(root) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("integration rules: {e}");
            return Vec::new();
        }
    };
    files
        .into_iter()
        .flat_map(|f| match f.rules {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("{}: skipped: {e}", f.path.display());
                Vec::new()
            }
        })
        .collect()
}

/// The installed rules whose id is on, in installed order. Ids with no installed rule are
/// skipped (and kept in the list).
pub fn enabled_rules(installed: &[AppRule], on: &IntegrationRules) -> Vec<AppRule> {
    installed
        .iter()
        .filter(|r| on.enabled.contains(&r.id))
        .cloned()
        .collect()
}

impl AppRule {
    fn matches_for(&self, class: &str) -> WindowMatch {
        WindowMatch {
            class: Some(class.to_string()),
            title: self.title.clone(),
            ..WindowMatch::default()
        }
    }

    fn effects(&self) -> WindowEffects {
        WindowEffects {
            float: self.float,
            fullscreen: self.fullscreen,
            workspace: self.workspace.clone(),
            ..WindowEffects::default()
        }
    }

    /// The rule as uQ window rules, one per `class` pattern (Hyprland's `class` takes one
    /// pattern). Their ids, which become the Hyprland rule name after `uq:`, are
    /// `app:<id>`, or `app:<id>.<n>` (from 1) when there are several patterns.
    pub fn window_rules(&self) -> Vec<WindowRule> {
        let many = self.class.len() > 1;
        self.class
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let id = if many {
                    format!("{RULE_NAME_PREFIX}{}.{}", self.id, i + 1)
                } else {
                    format!("{RULE_NAME_PREFIX}{}", self.id)
                };
                WindowRule {
                    name: id.clone(),
                    id,
                    enabled: true,
                    matches: self.matches_for(c),
                    effects: self.effects(),
                }
            })
            .collect()
    }

    /// The rule for Settings: `{id, integration, name, matches_text, effects_text}` plus its
    /// fields. Every text is the packaged file's, shown as plain text.
    pub fn to_json(&self) -> Value {
        let matches: Vec<String> = self
            .class
            .iter()
            .flat_map(|c| match_text(&self.matches_for(c)))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        json!({
            "id": self.id,
            "integration": self.integration,
            "name": self.name.clone().unwrap_or_else(|| self.id.clone()),
            "class": self.class,
            "title": self.title,
            "workspace": self.workspace,
            "fullscreen": self.fullscreen,
            "float": self.float,
            "matches_text": matches,
            "effects_text": effects_text(&self.effects()),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// The `integration_rules` domain

/// The packaged rules the person turned on, by id. Empty by default: every rule is opt-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntegrationRules {
    pub enabled: Vec<String>,
}

/// Loads leniently: a value of the wrong kind is empty, and entries that aren't text are
/// dropped, each with a log line; [`IntegrationRules::repaired`] does the rest.
impl<'de> Deserialize<'de> for IntegrationRules {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<IntegrationRules, D::Error> {
        let v = Value::deserialize(d)?;
        let enabled = match v.get("enabled") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|e| {
                    let s = e.as_str().map(str::to_string);
                    if s.is_none() {
                        tracing::warn!("state: dropping {DOMAIN}.enabled entry {e}");
                    }
                    s
                })
                .collect(),
            Some(_) => {
                tracing::warn!("state: {DOMAIN}.enabled is not a list; ignoring it");
                Vec::new()
            }
        };
        Ok(IntegrationRules { enabled })
    }
}

impl IntegrationRules {
    pub fn section(&self) -> Value {
        json!({ "enabled": self.enabled })
    }

    /// A whole-section write: `{"enabled": [<rule id>, …]}`, at most 64 distinct ids of the
    /// `<integration>.<rule>` form. Whether each rule is installed and valid is uqd's check,
    /// which has the files.
    pub fn from_section(v: &Value) -> Result<IntegrationRules, SettingError> {
        if !v.is_object() {
            return Err(setting_invalid(DOMAIN, "expected an object"));
        }
        check_known(DOMAIN, v, &["enabled"])?;
        let key = format!("{DOMAIN}.enabled");
        let items = match v.get("enabled") {
            None => return Ok(IntegrationRules::default()),
            Some(Value::Array(a)) => a,
            Some(_) => return Err(setting_invalid(&key, "expected a list")),
        };
        if items.len() > MAX_ENABLED {
            return Err(setting_invalid(
                &key,
                "at most 64 rules from apps can be on",
            ));
        }
        let mut enabled = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let at = format!("{key}[{i}]");
            let Some(id) = item.as_str().filter(|id| valid_rule_id(id)) else {
                return Err(setting_invalid(&at, "a rule id looks like app.rule-name"));
            };
            if enabled.iter().any(|e| e == id) {
                return Err(setting_invalid(&at, "this rule is listed twice"));
            }
            enabled.push(id.to_string());
        }
        Ok(IntegrationRules { enabled })
    }

    /// Load-time repair: bad and repeated ids are dropped with a log line, then the list is cut
    /// to [`MAX_ENABLED`].
    pub fn repaired(self) -> IntegrationRules {
        let mut seen = BTreeSet::new();
        let mut enabled: Vec<String> = self
            .enabled
            .into_iter()
            .filter(|id| {
                let keep = valid_rule_id(id) && seen.insert(id.clone());
                if !keep {
                    tracing::warn!("state: dropping {DOMAIN}.enabled entry {id:?}");
                }
                keep
            })
            .collect();
        enabled.truncate(MAX_ENABLED);
        IntegrationRules { enabled }
    }

    /// The ids on here that are not on in `before`.
    pub fn turned_on<'a>(&'a self, before: &IntegrationRules) -> Vec<&'a str> {
        self.enabled
            .iter()
            .filter(|id| !before.enabled.contains(id))
            .map(String::as_str)
            .collect()
    }
}

fn ids(v: &Value) -> Vec<&str> {
    v.get("enabled")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// One plain line for History: "Rule from an app turned on: romforge.emulators", up to three
/// changes, then ", and N more".
pub fn summary(before: &Value, after: &Value) -> String {
    let (b, a) = (ids(before), ids(after));
    let mut lines: Vec<String> = a
        .iter()
        .filter(|id| !b.contains(id))
        .map(|id| format!("Rule from an app turned on: {id}"))
        .collect();
    lines.extend(
        b.iter()
            .filter(|id| !a.contains(id))
            .map(|id| format!("Rule from an app turned off: {id}")),
    );
    let mut line = lines.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if lines.len() > 3 {
        line.push_str(&format!(", and {} more", lines.len() - 3));
    }
    if line.is_empty() {
        line = "Rules from apps".into();
    }
    line
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
