#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
//! The Startup page's logic (spec §7.3): the apps that start when you log in, and the
//! session's environment variables. [`StartupState`] talks to uqd (`org.uq.Desktop1`) and is
//! plain Rust, tested against the fake uqd.
//!
//! Every write reads its section (`env` or `autostart`), changes it, checks it with uq-core's
//! own rules so a refusal reads as a plain reason (F21), and writes it back under the write
//! lock. The §6 switches are shown read-only.

mod edit;
mod rows;

use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::autostart;
use uq_core::client::{self, DesktopProxy};
use uq_core::desktop_entry::{ExecError, normalise_id, split_command};
use uq_core::env;

use crate::desktop_entries::{self, AppEntry};
use crate::pages::runner::{PageLogic, message, parse_json};
use edit::{choice_mut, command_error, failure, find, list_mut, var_index, write_section};

/// Shown after any change to the variables.
pub const APPLY_NOTE: &str =
    "Changes apply to apps started after the change, and fully after you log out and back in.";
/// Added to [`APPLY_NOTE`] after a variable is removed (R9).
const REMOVED_NOTE: &str = "Removed variables stay for apps started before you log out.";
pub const UNCLOSED_QUOTE: &str = "A quote in the command isn't closed.";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// A found entry's choice (`files[id].enabled`), or an added entry's own `enabled`.
    SetEnabled {
        id: String,
        on: bool,
    },
    /// `None` clears it; a found entry's choice left with neither field is removed.
    SetDelay {
        id: String,
        delay: Option<u32>,
    },
    /// An installed app, by desktop id.
    AddApp {
        desktop_id: String,
    },
    /// A command, split into its parts with the launcher-file quoting rules.
    AddCommand {
        name: String,
        command: String,
    },
    /// Only an added entry; a found one is turned off instead.
    Remove {
        id: String,
    },
    /// Replace the variable at `index`, or add one at the end (`None`).
    SetVar {
        index: Option<usize>,
        name: String,
        value: String,
    },
    RemoveVar {
        name: String,
    },
    MoveVar {
        name: String,
        to: usize,
    },
}

pub struct StartupState {
    conn: zbus::Connection,
    /// Writes read-modify-write a section, so one at a time.
    write: Mutex<()>,
    /// Where the installed apps are read from, for "Add an app".
    app_dirs: Vec<PathBuf>,
    /// `(seq, text)` of the last notice.
    notice: StdMutex<(u64, String)>,
}

impl StartupState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_dirs(conn, desktop_entries::default_dirs())
    }

    pub fn with_dirs(conn: zbus::Connection, app_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            app_dirs,
            notice: StdMutex::new((0, String::new())),
        }
    }

    fn notice(&self, text: String) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text);
    }

    async fn installed(&self) -> zbus::Result<Vec<AppEntry>> {
        let dirs = self.app_dirs.clone();
        tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .map_err(|e| failure(format!("reading the installed apps failed: {e}")))
    }

    /// Whether `id` is one of the added entries.
    async fn is_added(proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<bool> {
        let section = parse_json(&proxy.get_section("autostart").await?)?;
        let added = section["added"].as_array().map_or(&[][..], Vec::as_slice);
        Ok(find(added, "id", id).is_some())
    }

    /// A found entry can be switched only when uqd says so: one turned off by its own file,
    /// or one that can't run here, can't.
    async fn check_toggle(proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<()> {
        let rows = parse_json(&proxy.list_autostart().await?)?;
        let rows = rows.as_array().map_or(&[][..], Vec::as_slice);
        let row = find(rows, "id", id)
            .map(|i| &rows[i])
            .ok_or_else(|| failure("That app isn't in the list any more."))?;
        if row["can_toggle"] == true {
            return Ok(());
        }
        let mut text = "This app can't be turned on or off here.".to_string();
        if let Some(r) = row["reason"].as_str() {
            text = format!("{text} {r}.");
        }
        Err(failure(text))
    }

    async fn set_enabled(&self, proxy: &DesktopProxy<'_>, id: &str, on: bool) -> zbus::Result<()> {
        let added = Self::is_added(proxy, id).await?;
        if !added {
            Self::check_toggle(proxy, id).await?;
        }
        write_section(proxy, "autostart", |o| {
            if added {
                let list = list_mut(o, "added")?;
                let i = find(list, "id", id).ok_or_else(|| failure("That app was removed."))?;
                list[i]["enabled"] = json!(on);
            } else {
                choice_mut(list_mut(o, "files")?, id)["enabled"] = json!(on);
            }
            Ok(())
        })
        .await
    }

    async fn set_delay(
        &self,
        proxy: &DesktopProxy<'_>,
        id: &str,
        delay: Option<u32>,
    ) -> zbus::Result<()> {
        write_section(proxy, "autostart", |o| {
            let added = list_mut(o, "added")?;
            if let Some(i) = find(added, "id", id) {
                added[i]["delay"] = json!(delay);
                return Ok(());
            }
            let files = list_mut(o, "files")?;
            choice_mut(files, id)["delay"] = json!(delay);
            files.retain(|c| !(c["enabled"].is_null() && c["delay"].is_null()));
            Ok(())
        })
        .await
    }

    async fn add_app(&self, proxy: &DesktopProxy<'_>, desktop_id: &str) -> zbus::Result<()> {
        let id = normalise_id(desktop_id);
        let apps = self.installed().await?;
        let app = apps
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| failure("That app isn't installed."))?;
        // F43: a name from another package's file can be longer than an entry's name may be.
        let name: String = app.name.chars().take(autostart::MAX_NAME_CHARS).collect();
        let entry = json!({"name": name.trim(), "app": id, "enabled": true, "delay": null});
        write_section(proxy, "autostart", |o| {
            list_mut(o, "added")?.push(entry);
            Ok(())
        })
        .await
    }

    async fn add_command(
        &self,
        proxy: &DesktopProxy<'_>,
        name: &str,
        command: &str,
    ) -> zbus::Result<()> {
        let argv = split_command(command.trim()).map_err(|e| failure(command_error(&e)))?;
        let Some(program) = argv.first() else {
            return Err(failure(command_error(&ExecError::Empty)));
        };
        let name = match name.trim() {
            "" => program.rsplit('/').next().unwrap_or(program),
            typed => typed,
        };
        let entry = json!({"name": name, "argv": argv, "enabled": true, "delay": null});
        write_section(proxy, "autostart", |o| {
            list_mut(o, "added")?.push(entry);
            Ok(())
        })
        .await
    }

    async fn remove(&self, proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<()> {
        write_section(proxy, "autostart", |o| {
            let added = list_mut(o, "added")?;
            let i = find(added, "id", id).ok_or_else(|| {
                failure("This app wasn't added here, so it can't be removed. Turn it off instead.")
            })?;
            added.remove(i);
            Ok(())
        })
        .await
    }

    /// An `env` write, then the notice (R9 when a variable went).
    async fn write_env(
        &self,
        proxy: &DesktopProxy<'_>,
        removed: bool,
        edit: impl FnOnce(&mut Vec<Value>) -> zbus::Result<()>,
    ) -> zbus::Result<()> {
        write_section(proxy, "env", |o| edit(list_mut(o, "vars")?)).await?;
        self.notice(if removed {
            format!("{APPLY_NOTE} {REMOVED_NOTE}")
        } else {
            APPLY_NOTE.to_string()
        });
        Ok(())
    }
}

/// The state is the snapshot of the plan's Task 10: `{apps, apps_error, vars, switches,
/// section, installed, limits, notice}`.
impl PageLogic for StartupState {
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
        let proxy = DesktopProxy::new(&self.conn).await?;
        let p = &proxy;
        match op {
            Op::Refresh => Ok(()),
            Op::SetEnabled { id, on } => self.set_enabled(p, &id, on).await,
            Op::SetDelay { id, delay } => self.set_delay(p, &id, delay).await,
            Op::AddApp { desktop_id } => self.add_app(p, &desktop_id).await,
            Op::AddCommand { name, command } => self.add_command(p, &name, &command).await,
            Op::Remove { id } => self.remove(p, &id).await,
            Op::SetVar { index, name, value } => {
                let var = json!({"name": name, "value": value});
                self.write_env(p, false, |vars| {
                    match index {
                        None => vars.push(var),
                        Some(i) if i < vars.len() => vars[i] = var,
                        Some(_) => return Err(failure("That variable isn't there any more.")),
                    }
                    Ok(())
                })
                .await
            }
            Op::RemoveVar { name } => {
                self.write_env(p, true, |vars| {
                    vars.remove(var_index(vars, &name)?);
                    Ok(())
                })
                .await
            }
            Op::MoveVar { name, to } => {
                self.write_env(p, false, |vars| {
                    let var = vars.remove(var_index(vars, &name)?);
                    vars.insert(to.min(vars.len()), var);
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let env_section = parse_json(&proxy.get_section("env").await?)?;
        let auto_section = parse_json(&proxy.get_section("autostart").await?)?;
        let compat = parse_json(&proxy.get_section("compat").await?)?;
        let env_rows = explain(&proxy, "env").await?;
        let auto_rows = explain(&proxy, "autostart").await?;
        // A failed list leaves the variables on show, with the reason.
        let (found, apps_error) = match proxy.list_autostart().await.and_then(|t| parse_json(&t)) {
            Ok(Value::Array(rows)) => (rows, None),
            Ok(_) => (
                Vec::new(),
                Some("uQ's settings service sent an unreadable list of startup apps".to_string()),
            ),
            Err(e) if client::no_owner(&e) => return Err(e),
            Err(e) => (Vec::new(), Some(message(&e))),
        };
        let apps: Vec<Value> = found
            .iter()
            .map(|row| rows::app_row(row, &auto_section, &auto_rows))
            .collect();
        let installed: Vec<Value> = self
            .installed()
            .await?
            .into_iter()
            .map(|a| json!({"id": a.id, "name": a.name, "icon": a.icon}))
            .collect();
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "apps": apps,
            "apps_error": apps_error,
            "vars": rows::var_rows(&env_section, &env_rows),
            "switches": rows::switches(&compat),
            "section": {"env": rows::section_error(&env_rows, "env"),
                "autostart": rows::section_error(&auto_rows, "autostart")},
            "installed": installed,
            "limits": {"vars": env::MAX_VARS, "value_bytes": env::MAX_VALUE_BYTES,
                "added": autostart::MAX_ADDED, "delay_max": autostart::MAX_DELAY},
            "notice": {"seq": seq, "text": text},
        }))
    }
}

/// uqd's `Explain` rows for `domain`; a failure other than uqd being gone leaves the rows
/// without a source.
async fn explain(proxy: &DesktopProxy<'_>, domain: &str) -> zbus::Result<Value> {
    match proxy.explain(domain).await.and_then(|t| parse_json(&t)) {
        Ok(v) => Ok(v),
        Err(e) if client::no_owner(&e) => Err(e),
        Err(_) => Ok(json!({})),
    }
}

#[cfg(test)]
mod tests;
