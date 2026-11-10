//! The Shell page's logic: [`ShellState`] talks to uqd (`org.uq.Desktop1`) and is plain Rust,
//! tested against the fake uqd. `crate::shell_model` is the thin QML object around it.
//!
//! Pins, idle, notifications and privacy are Phase 2 settings (`SetSetting`); the bar islands,
//! launcher providers, clock and terminal are the `shell` section, which is read, edited and
//! written back under the write lock.

use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::launcher_providers::{self, ProviderDef};
use uq_core::settings::{BUILTIN_PROVIDERS, Islands};

use crate::desktop_entries;
use crate::pages::runner::{ORIGIN, PageLogic, parse_json, patch_section};

/// The `SetSetting` keys this page may send.
const SETTING_KEYS: [&str; 11] = [
    "pinned",
    "idle.dim",
    "idle.lock",
    "idle.screen_off",
    "idle.suspend_battery",
    "idle.locked_screen_off",
    "idle.media_blocks_lock",
    "notif.backend",
    "notif.show_summary",
    "privacy.public_ip_lookup",
    "privacy.record_launches",
];

/// The `shell` section's fields (`islands.<id>` patches one island).
const SHELL_FIELDS: [&str; 3] = ["providers", "clock_24h", "terminal"];

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// A Phase 2 setting, by key.
    SetSetting {
        key: String,
        value: Value,
    },
    /// One field of the `shell` section; `islands.<id>` patches that island only.
    SetShell {
        field: String,
        value: Value,
    },
    ClearLaunchCounts,
    /// Unpin `id` from the pins as uqd has them now.
    PinRemove {
        id: String,
    },
    /// Move pin `id` by `to - from` places from where it is now (clamped to the list), so an
    /// edit made on an older list still means what the user saw.
    PinMove {
        id: String,
        from: usize,
        to: usize,
    },
    /// Move provider row `id` by `to - from` places, as [`Op::PinMove`].
    ProviderMove {
        id: String,
        from: usize,
        to: usize,
    },
    /// Turn provider `id` on or off.
    ProviderToggle {
        id: String,
        on: bool,
    },
}

pub struct ShellState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    app_dirs: Vec<PathBuf>,
    provider_dirs: Vec<PathBuf>,
}

impl ShellState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_dirs(
            conn,
            desktop_entries::default_dirs(),
            launcher_providers::default_dirs(),
        )
    }

    pub fn with_dirs(
        conn: zbus::Connection,
        app_dirs: Vec<PathBuf>,
        provider_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            app_dirs,
            provider_dirs,
        }
    }
}

fn refuse<T>(text: String) -> zbus::Result<T> {
    Err(zbus::Error::Failure(text))
}

/// Move `id` within `list` by `to - from` places from its current position, clamped; an id
/// that is gone is left alone.
fn move_by<T>(list: &mut Vec<T>, pos: Option<usize>, from: usize, to: usize) {
    let Some(i) = pos else { return };
    let target = (i as isize + to as isize - from as isize).clamp(0, list.len() as isize - 1);
    let item = list.remove(i);
    list.insert(target as usize, item);
}

/// Every provider row in the launcher's order, as `{id, enabled}`: the saved order, then the
/// providers the saved list lacks, in default order. Writing them all keeps that order.
fn provider_rows(defs: &[ProviderDef], shell: &Value) -> Vec<Value> {
    let mut rows: Vec<Value> = shell["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = p["id"].as_str()?;
            Some(json!({"id": id, "enabled": p["enabled"].as_bool().unwrap_or(true)}))
        })
        .collect();
    for p in providers_list(defs, shell) {
        if !rows.iter().any(|r| r["id"] == p["id"]) {
            rows.push(json!({"id": p["id"], "enabled": true}));
        }
    }
    rows
}

/// Builtins, then installed definitions, then ids only the saved preferences know (`missing`).
fn providers_list(defs: &[ProviderDef], shell: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = BUILTIN_PROVIDERS
        .iter()
        .map(|id| {
            let name = if *id == "calc" { "Calculator" } else { "Run" };
            json!({"id": id, "name": name, "builtin": true})
        })
        .collect();
    out.extend(
        defs.iter()
            .map(|d| json!({"id": d.bus_name, "name": d.name, "builtin": false})),
    );
    let saved = shell["providers"].as_array().into_iter().flatten();
    for id in saved.filter_map(|p| p["id"].as_str()) {
        if !out.iter().any(|p| p["id"] == id) {
            out.push(json!({"id": id, "name": id, "builtin": false, "missing": true}));
        }
    }
    out
}

impl ShellState {
    /// The pins as uqd has them now.
    async fn pins(&self, proxy: &DesktopProxy<'_>) -> zbus::Result<Vec<String>> {
        let settings = parse_json(&proxy.get_settings().await?)?;
        Ok(settings["pinned"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect())
    }

    async fn defs(&self) -> zbus::Result<Vec<ProviderDef>> {
        let dirs = self.provider_dirs.clone();
        tokio::task::spawn_blocking(move || launcher_providers::load_defs(&dirs))
            .await
            .map_err(|e| zbus::Error::Failure(format!("reading the providers failed: {e}")))
    }
}

impl PageLogic for ShellState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        // Refused before any call to uqd.
        match &op {
            Op::SetSetting { key, .. } if !SETTING_KEYS.contains(&key.as_str()) => {
                return refuse(format!("unknown setting {key:?}"));
            }
            Op::SetShell { field, .. } => {
                let known = SHELL_FIELDS.contains(&field.as_str())
                    || field
                        .strip_prefix("islands.")
                        .is_some_and(|i| Islands::FIELDS.contains(&i));
                if !known {
                    return refuse(format!("unknown field {field:?}"));
                }
            }
            _ => {}
        }
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetSetting { key, value } => {
                proxy.set_setting(&key, &value.to_string(), ORIGIN).await
            }
            Op::ClearLaunchCounts => proxy.clear_launch_counts().await,
            // Pins and providers are edited as intents on a fresh read, under the write lock,
            // so two quick edits both land.
            Op::PinRemove { id } => {
                let mut pins = self.pins(&proxy).await?;
                pins.retain(|p| *p != id);
                proxy
                    .set_setting("pinned", &json!(pins).to_string(), ORIGIN)
                    .await
            }
            Op::PinMove { id, from, to } => {
                let mut pins = self.pins(&proxy).await?;
                let pos = pins.iter().position(|p| *p == id);
                move_by(&mut pins, pos, from, to);
                proxy
                    .set_setting("pinned", &json!(pins).to_string(), ORIGIN)
                    .await
            }
            Op::ProviderMove { .. } | Op::ProviderToggle { .. } => {
                let defs = self.defs().await?;
                patch_section(&proxy, "shell", |section| {
                    let mut rows = provider_rows(&defs, &Value::Object(section.clone()));
                    match op {
                        Op::ProviderMove { id, from, to } => {
                            let pos = rows.iter().position(|r| r["id"] == id.as_str());
                            move_by(&mut rows, pos, from, to);
                        }
                        Op::ProviderToggle { id, on } => {
                            if let Some(r) = rows.iter_mut().find(|r| r["id"] == id.as_str()) {
                                r["enabled"] = json!(on);
                            }
                        }
                        _ => unreachable!(),
                    }
                    section.insert("providers".into(), Value::Array(rows));
                    Ok(())
                })
                .await
            }
            Op::SetShell { field, value } => {
                patch_section(&proxy, "shell", |section| {
                    match field.strip_prefix("islands.") {
                        Some(island) => {
                            let islands = section
                                .entry("islands")
                                .or_insert_with(|| json!({}))
                                .as_object_mut()
                                .ok_or_else(|| {
                                    zbus::Error::Failure("the islands aren't an object".into())
                                })?;
                            islands.insert(island.to_string(), value);
                        }
                        None => {
                            section.insert(field, value);
                        }
                    }
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let settings = parse_json(&proxy.get_settings().await?)?;
        let shell = parse_json(&proxy.get_section("shell").await?)?;
        let (app_dirs, provider_dirs) = (self.app_dirs.clone(), self.provider_dirs.clone());
        let (apps, defs) = tokio::task::spawn_blocking(move || {
            (
                desktop_entries::scan(&app_dirs),
                launcher_providers::load_defs(&provider_dirs),
            )
        })
        .await
        .map_err(|e| zbus::Error::Failure(format!("reading the installed apps failed: {e}")))?;
        let terminals = desktop_entries::terminals(&apps);
        Ok(json!({
            "settings": settings,
            "providers": providers_list(&defs, &shell),
            "terminals": terminals,
            "apps": apps,
            "shell": shell,
        }))
    }
}

#[cfg(test)]
mod tests;
