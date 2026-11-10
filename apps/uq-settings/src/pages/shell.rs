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

use crate::desktop_entries::{self, AppEntry};
use crate::pages::runner::{PageLogic, parse_json, patch_section};

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
            Op::SetSetting { key, value } => proxy.set_setting(&key, &value.to_string()).await,
            Op::ClearLaunchCounts => proxy.clear_launch_counts().await,
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
        let terminals: Vec<&AppEntry> = apps.iter().filter(|a| a.terminal_emulator).collect();
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
mod tests {
    use super::*;
    use crate::pages::fake_uqd::{self, Rig};
    use crate::pages::runner::run;
    use uq_core::state::State;

    async fn rig_in(app_dirs: Vec<PathBuf>, provider_dirs: Vec<PathBuf>) -> (Rig, ShellState) {
        let mut state = State::default();
        state
            .set_section(
                "shell",
                json!({"islands": {"work": false}, "clock_24h": false,
                       "providers": [{"id": "run", "enabled": false}, {"id": "org.gone.P"}],
                       "terminal": "kitty.desktop"}),
            )
            .unwrap();
        state.settings.pinned = vec!["a.desktop".into(), "b.desktop".into()];
        let r = fake_uqd::rig(state).await;
        let page = ShellState::with_dirs(r.client.clone(), app_dirs, provider_dirs);
        (r, page)
    }

    async fn rig() -> (Rig, ShellState) {
        rig_in(vec![], vec![]).await
    }

    fn shell(field: &str, value: Value) -> Op {
        Op::SetShell {
            field: field.into(),
            value,
        }
    }

    fn setting(key: &str, value: Value) -> Op {
        Op::SetSetting {
            key: key.into(),
            value,
        }
    }

    #[tokio::test]
    async fn set_shell_keeps_other_fields() {
        let (r, page) = rig().await;
        let out = run(&page, shell("clock_24h", json!(true))).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["shell"]["clock_24h"], true);
        assert_eq!(out.state["shell"]["terminal"], "kitty.desktop");
        assert_eq!(out.state["shell"]["islands"]["work"], false);
        assert_eq!(out.state["shell"]["providers"][0]["id"], "run");

        let out = run(&page, shell("terminal", json!("foot.desktop"))).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["shell"]["terminal"], "foot.desktop");
        assert_eq!(out.state["shell"]["clock_24h"], true);
        assert_eq!(
            r.state.lock().unwrap().settings.shell.terminal.as_deref(),
            Some("foot.desktop")
        );

        let out = run(&page, shell("terminal", json!("../x.desktop"))).await;
        assert!(out.error.unwrap().contains("terminal"));
        assert_eq!(out.state["shell"]["terminal"], "foot.desktop");

        let out = run(&page, shell("terminal", Value::Null)).await;
        assert_eq!(out.error, None);
        assert_eq!(r.state.lock().unwrap().settings.shell.terminal, None);

        let out = run(&page, shell("nope", json!(1))).await;
        assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
        let out = run(&page, shell("islands.nope", json!(true))).await;
        assert!(out.error.unwrap().contains("unknown field"));
    }

    #[tokio::test]
    async fn islands_patch_one() {
        let (r, page) = rig().await;
        let out = run(&page, shell("islands.notif", json!(false))).await;
        assert_eq!(out.error, None);
        let islands = r.state.lock().unwrap().settings.shell.islands;
        assert!(!islands.notif && !islands.work, "work was already off");
        assert!(islands.system && islands.time && islands.status);
        assert_eq!(out.state["shell"]["islands"]["notif"], false);
        assert_eq!(out.state["shell"]["islands"]["system"], true);
    }

    #[tokio::test]
    async fn providers_list_marks_missing() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("x.provider"),
            "[Launcher Provider]\nBusName=org.x.Search\nName=X search\n",
        )
        .unwrap();
        let (_r, page) = rig_in(vec![], vec![d.path().to_path_buf()]).await;
        let out = run(&page, Op::Refresh).await;
        assert_eq!(out.error, None);
        assert_eq!(
            out.state["providers"],
            json!([
                {"id": "calc", "name": "Calculator", "builtin": true},
                {"id": "run", "name": "Run", "builtin": true},
                {"id": "org.x.Search", "name": "X search", "builtin": false},
                {"id": "org.gone.P", "name": "org.gone.P", "builtin": false, "missing": true},
            ])
        );
        assert_eq!(out.state["shell"]["providers"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn pins_go_through_set_setting() {
        let (r, page) = rig().await;
        let out = run(&page, setting("pinned", json!(["b.desktop", "a.desktop"]))).await;
        assert_eq!(out.error, None);
        assert_eq!(
            out.state["settings"]["pinned"],
            json!(["b.desktop", "a.desktop"])
        );
        assert_eq!(
            r.state.lock().unwrap().settings.pinned,
            ["b.desktop", "a.desktop"]
        );

        let out = run(&page, setting("pinned", json!(["../evil"]))).await;
        let message = out.error.expect("a bad id is refused");
        assert!(message.contains("pinned"), "{message}");
        assert_eq!(
            out.state["settings"]["pinned"],
            json!(["b.desktop", "a.desktop"])
        );

        let out = run(&page, setting("shell", json!(1))).await;
        assert_eq!(out.error.as_deref(), Some("unknown setting \"shell\""));
    }

    #[tokio::test]
    async fn idle_order_refusal_snaps_back() {
        let (r, page) = rig().await;
        let out = run(&page, Op::Refresh).await;
        assert_eq!(out.state["settings"]["idle"]["lock"], 300);
        let out = run(&page, setting("idle.dim", json!(400))).await;
        let message = out.error.expect("dim above lock is refused");
        assert!(message.contains("strictly increasing"), "{message}");
        assert_eq!(out.state["settings"]["idle"]["dim"], 240);
        assert_eq!(r.state.lock().unwrap().settings.idle.dim, Some(240));

        // Off is allowed, and so is a free value in order.
        let out = run(&page, setting("idle.dim", Value::Null)).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["settings"]["idle"]["dim"], Value::Null);
        let out = run(&page, setting("idle.locked_screen_off", json!(5))).await;
        assert!(out.error.is_some());
        assert_eq!(out.state["settings"]["idle"]["locked_screen_off"], 30);
    }

    #[tokio::test]
    async fn clear_launch_counts_calls_uqd() {
        let (r, page) = rig().await;
        r.state
            .lock()
            .unwrap()
            .launch_counts
            .insert("a.desktop".into(), 3);
        let out = run(&page, Op::ClearLaunchCounts).await;
        assert_eq!(out.error, None);
        assert!(r.state.lock().unwrap().launch_counts.is_empty());
    }
}
