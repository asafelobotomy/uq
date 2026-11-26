//! Which integration rules the user switched on, and their history summary.

use super::*;

// ---------------------------------------------------------------------------------------------
// The `integration_rules` domain

/// The packaged rules the person turned on, by id. Empty by default: every rule is opt-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntegrationRules {
    pub enabled: Vec<String>,
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
                    tracing::warn!("[STATE-002] state: dropping {DOMAIN}.enabled entry {id:?}");
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

pub(crate) fn ids(v: &Value) -> Vec<&str> {
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
