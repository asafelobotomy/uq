//! Explain: where each setting's value comes from.

use super::*;

/// `explain`'s dotted key as a profile path.
pub(crate) fn json_path(key: &str) -> Vec<String> {
    key.split('.').map(str::to_string).collect()
}

/// How much of `user.lua` `explain` scans.
pub(crate) const USER_LUA_SCAN_LIMIT: u64 = 1 << 20;

impl Desktop {
    /// Where each key of `domain` gets its live value: `{key: {source, live, stored, detail}}`,
    /// keyed by the dotted path within the section. `source` is `theme`, `default`, `uq`,
    /// `profile` (a `uq` value the active profile set), `user_lua`, `runtime` or `error`.
    /// Without a live Hyprland every key reports from state.
    pub async fn explain(&self, domain: &str) -> Result<serde_json::Value, DesktopError> {
        use serde_json::{Value, json};
        if !SECTION_DOMAINS.contains(&domain) {
            return Err(DesktopError::InvalidArgument(format!(
                "unknown domain {domain:?}"
            )));
        }
        if domain == "rules" {
            return Ok(self.explain_rules());
        }
        if domain == "keybinds" {
            return Ok(self.explain_keybinds().await);
        }
        if domain == "integration_rules" {
            return Ok(self.explain_integration_rules());
        }
        if matches!(domain, "env" | "autostart") {
            return Ok(self.explain_startup(domain, Value::Null));
        }
        let section = self
            .state
            .section(domain)
            .map_err(|e| DesktopError::InvalidSetting(e.to_string()))?;
        let theme = self.themes.get(&self.state.theme, self.state.variant);
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == domain)
            .map(|(_, m)| m.clone());
        let mut up = self.comp.config_errors().await.is_ok();
        let mut user_lua: Option<Option<String>> = None;
        let mut out = serde_json::Map::new();
        for row in provenance::options_for(domain) {
            let stored = row.stored(&section);
            let from_state = if stored.is_null() {
                row.null_source.as_str()
            } else {
                "uq"
            };
            let mut live = Value::Null;
            if up && let Some(path) = row.hypr {
                match self.comp.get_option(path).await {
                    Ok(v) => live = v,
                    Err(CompositorError::Rejected(e)) => {
                        tracing::warn!("[HYPR-003] getoption {path}: {e}")
                    }
                    Err(e) => {
                        tracing::warn!("[HYPR-003] getoption {path}: {e}");
                        up = false;
                    }
                }
            } else if up && let Some((schema, key)) = row.gsettings {
                live = self.sys.read(schema, key).map_or(Value::Null, Value::from);
            }
            let expected = theme.and_then(|t| provenance::expected(row, &self.state, t));
            let (source, detail) = if let Some(message) = &rejected {
                ("error", json!({ "message": message }))
            } else if live.is_null()
                || expected
                    .as_ref()
                    .is_some_and(|e| provenance::live_matches(e, &live))
            {
                (from_state, Value::Null)
            } else {
                // A mismatch, or a value uQ leaves to base.lua: look for it in user.lua.
                let line = row.hypr.and_then(|path| {
                    let text = user_lua.get_or_insert_with(|| self.read_user_lua());
                    provenance::find_in_user_lua(text.as_deref()?, path)
                });
                match line {
                    Some(line) => (
                        "user_lua",
                        json!({ "line": line, "path": self.paths.user_lua.to_string_lossy() }),
                    ),
                    None if expected.is_none() => (from_state, Value::Null),
                    None => ("runtime", Value::Null),
                }
            };
            let (source, detail) = match &self.active {
                Some(a)
                    if source == "uq" && holds(a, domain, &json_path(row.json_key()), &stored) =>
                {
                    ("profile", json!({ "name": a.name }))
                }
                _ => (source, detail),
            };
            out.insert(
                row.json_key().to_string(),
                json!({ "source": source, "live": live, "stored": stored, "detail": detail }),
            );
        }
        if domain == "input" {
            let text = user_lua.get_or_insert_with(|| self.read_user_lua());
            out.insert(
                "gestures".into(),
                self.explain_gestures(&section, text.as_deref()),
            );
        }
        let out = Value::Object(out);
        Ok(match domain {
            "compat" => self.explain_startup(domain, out),
            _ => out,
        })
    }

    /// The `gestures` row of `explain("input")` (R22): a gesture in `user.lua` is the one that
    /// can make Hyprland refuse uQ's own, so it comes first. Hyprland can't list its gestures,
    /// so there is no `runtime` source.
    pub(crate) fn explain_gestures(
        &self,
        section: &serde_json::Value,
        user_lua: Option<&str>,
    ) -> serde_json::Value {
        use serde_json::json;
        let stored = section.get("gestures").cloned().unwrap_or_default();
        let line = user_lua.and_then(provenance::find_gesture_in_user_lua);
        let (source, detail) = match line {
            Some(line) => (
                "user_lua",
                json!({ "line": line, "path": self.paths.user_lua.to_string_lossy() }),
            ),
            None if stored.is_null() => ("default", serde_json::Value::Null),
            None => ("uq", serde_json::Value::Null),
        };
        json!({ "source": source, "live": null, "stored": stored, "detail": detail })
    }

    /// The first [`USER_LUA_SCAN_LIMIT`] bytes of `user.lua`, decoded lossily; `None` if unreadable.
    pub(crate) fn read_user_lua(&self) -> Option<String> {
        use std::io::Read;
        let mut buf = Vec::new();
        std::fs::File::open(&self.paths.user_lua)
            .and_then(|f| f.take(USER_LUA_SCAN_LIMIT).read_to_end(&mut buf))
            .ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
    }
}
