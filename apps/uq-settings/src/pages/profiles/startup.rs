//! The Profiles page and startup apps, environment variables and app fixes (D1): their keys
//! from the current settings, and their labels with the names a person knows.

use std::path::PathBuf;

use serde_json::Value;
use uq_core::client::DesktopProxy;
use uq_core::profile::{Key, startup_keys_of, startup_label};
use uq_core::state::State;

use crate::desktop_entries::{self, AppEntry};
use crate::pages::runner::parse_json;

use super::failure;

/// The journal domains these keys come from.
pub const DOMAINS: [&str; 3] = ["env", "compat", "autostart"];

/// Said on export when the profile holds variables.
pub const EXPORT_NOTICE: &str = "This profile includes your variables' values.";

/// The three sections as uqd holds them, the entries found in the autostart folders
/// (`ListAutostart`), and the installed apps (for fix names; empty when only values are needed).
#[derive(Default)]
pub struct Startup {
    state: State,
    rows: Vec<Value>,
    apps: Vec<AppEntry>,
}

impl Startup {
    /// Read the sections and the startup list. A list that can't be read only costs names and
    /// the entries that have no choice yet.
    pub async fn read(proxy: &DesktopProxy<'_>, apps: Vec<AppEntry>) -> zbus::Result<Startup> {
        let mut state = State::default();
        for domain in DOMAINS {
            let v = parse_json(&proxy.get_section(domain).await?)?;
            state
                .set_section(domain, v)
                .map_err(|_| failure("uQ's settings service sent unreadable startup settings"))?;
        }
        let rows = match proxy.list_autostart().await.and_then(|t| parse_json(&t)) {
            Ok(Value::Array(rows)) => rows,
            _ => Vec::new(),
        };
        Ok(Startup { state, rows, apps })
    }

    /// The installed apps in `dirs`, read off the async thread.
    pub async fn installed(dirs: Vec<PathBuf>) -> Vec<AppEntry> {
        tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .unwrap_or_default()
    }

    /// Every key of `domain` as the settings hold it, then the found entries that have no
    /// choice yet: their switch (and their delay when they have one), from `ListAutostart`.
    pub fn keys(&self, domain: &str) -> Vec<Key> {
        let mut out: Vec<Key> = startup_keys_of(&self.state)
            .into_iter()
            .filter(|k| k.domain == domain)
            .collect();
        if domain != "autostart" {
            return out;
        }
        for row in &self.rows {
            let Some(id) = row["id"].as_str() else {
                continue;
            };
            if row["source"] == "added"
                || row["can_toggle"] != true
                || self.state.autostart.choice(id).is_some()
            {
                continue;
            }
            let mut add = |field: &str, value: &Value| {
                out.push(Key {
                    domain: "autostart".into(),
                    path: vec![id.to_string(), field.to_string()],
                    value: value.clone(),
                })
            };
            if row["enabled"].is_boolean() {
                add("enabled", &row["enabled"]);
            }
            if row["delay"].as_u64().is_some_and(|d| d > 0) {
                add("delay", &row["delay"]);
            }
        }
        out
    }

    /// A startup key's label: entries named as `ListAutostart` names them, fixes by the app's
    /// name.
    pub fn label(&self, domain: &str, path: &[String]) -> String {
        let names = |id: &str| -> Option<String> {
            let name = match domain {
                "autostart" => self
                    .rows
                    .iter()
                    .find(|r| r["id"] == id)
                    .and_then(|r| r["name"].as_str()),
                _ => self
                    .apps
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| a.name.as_str()),
            };
            name.filter(|n| !n.is_empty()).map(str::to_string)
        };
        startup_label(&self.state, domain, path, &names)
    }
}
