#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]
//! The Apps and scaling page's logic (spec §6): the five switches, the "Blurry right now" list
//! of windows that run under XWayland, and each app's "Fix this app" fix. [`CompatState`] talks
//! to uqd (`org.uq.Desktop1`) and is plain Rust, tested against the fake uqd.
//!
//! Every write reads the `compat` section, changes it, checks it with uq-core's own rules so a
//! refusal reads as a plain reason (F21), and writes it back under the write lock. A window's
//! class is matched to an installed app here, from the desktop files (R17).

mod catalog;
mod rows;

use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::compat::hypr::SwitchKey;
use uq_core::desktop_entry::normalise_id;
use uq_core::rules::WindowInfo;

pub use catalog::{fields, preview};

use crate::desktop_entries::{self, AppEntry};
use crate::pages::runner::{PageLogic, parse_json};
use crate::pages::startup::edit::{failure, find, list_mut, write_section};
use crate::pages::startup::explain;
use crate::pages::startup::rows::section_error;

/// The heading over "Blurry right now", by `(scaled, sharp_on)`.
pub const BLURRY: &str = "These older apps may look blurry.";
pub const SHARP_ON: &str = "These older apps are open right now.";
pub const NOT_SCALED: &str = "Your screens aren't scaled, so nothing is blurry.";
/// How many window names a reopen note lists.
const REOPEN_NAMES: usize = 3;
const NO_FIX: &str = "This app has no fix any more.";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Read the open windows again.
    RefreshWindows,
    SetSwitch {
        key: String,
        on: bool,
    },
    /// Add a fix, or replace the one with the same app id.
    SaveFix {
        fix: Value,
    },
    RemoveFix {
        id: String,
    },
    /// "Also when started elsewhere".
    SetElsewhere {
        id: String,
        on: bool,
    },
}

#[derive(Debug, Default)]
struct Reopen {
    seq: u64,
    names: Vec<String>,
    more: usize,
}

pub struct CompatState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    /// Where the installed apps are read from, to name a window's app.
    app_dirs: Vec<PathBuf>,
    /// The last window list.
    windows: StdMutex<Vec<WindowInfo>>,
    reopen: StdMutex<Reopen>,
}

/// The installed app a window class belongs to. Each rule is tried over every app before the
/// next: `StartupWMClass`, then the id without `.desktop`, then the id's last dot part
/// (`org.gnome.Foo` for `foo`), all ignoring ASCII case; then the program's file name, exactly.
pub fn app_for_class<'a>(class: &str, apps: &'a [AppEntry]) -> Option<&'a AppEntry> {
    let stem = |a: &AppEntry| a.id.strip_suffix(".desktop").unwrap_or(&a.id).to_string();
    let rules: [&dyn Fn(&AppEntry) -> bool; 4] = [
        &|a| {
            a.wm_class
                .as_deref()
                .is_some_and(|w| w.eq_ignore_ascii_case(class))
        },
        &|a| stem(a).eq_ignore_ascii_case(class),
        &|a| {
            stem(a)
                .rsplit('.')
                .next()
                .is_some_and(|p| p.eq_ignore_ascii_case(class))
        },
        &|a| a.exec_name.as_deref() == Some(class),
    ];
    if class.is_empty() {
        return None;
    }
    rules.iter().find_map(|rule| apps.iter().find(|a| rule(a)))
}

impl CompatState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_dirs(conn, desktop_entries::default_dirs())
    }

    pub fn with_dirs(conn: zbus::Connection, app_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            app_dirs,
            windows: StdMutex::new(Vec::new()),
            reopen: StdMutex::new(Reopen::default()),
        }
    }

    async fn installed(&self) -> zbus::Result<Vec<AppEntry>> {
        let dirs = self.app_dirs.clone();
        tokio::task::spawn_blocking(move || desktop_entries::scan(&dirs))
            .await
            .map_err(|e| failure(format!("reading the installed apps failed: {e}")))
    }

    async fn refresh_windows(&self, proxy: &DesktopProxy<'_>) -> zbus::Result<()> {
        let list: Vec<WindowInfo> =
            serde_json::from_value(parse_json(&proxy.list_windows().await?)?)
                .map_err(|_| failure("uQ's settings service sent an unreadable window list"))?;
        *self.windows.lock().unwrap() = list;
        Ok(())
    }

    /// After a fix changed: note the open windows of its app, which pick it up only when they
    /// start again (no button closes windows).
    async fn reopen_for(&self, proxy: &DesktopProxy<'_>, id: &str) {
        self.refresh_windows(proxy).await.ok();
        let Ok(apps) = self.installed().await else {
            return;
        };
        let names: Vec<String> = self
            .windows
            .lock()
            .unwrap()
            .iter()
            .filter(|w| app_for_class(rows::class_of(w), &apps).is_some_and(|a| a.id == id))
            .map(|w| {
                let name = if w.title.is_empty() {
                    rows::class_of(w)
                } else {
                    &w.title
                };
                name.to_string()
            })
            .collect();
        if names.is_empty() {
            return;
        }
        let mut reopen = self.reopen.lock().unwrap();
        *reopen = Reopen {
            seq: reopen.seq + 1,
            more: names.len().saturating_sub(REOPEN_NAMES),
            names: names.into_iter().take(REOPEN_NAMES).collect(),
        };
    }

    async fn set_switch(&self, proxy: &DesktopProxy<'_>, key: &str, on: bool) -> zbus::Result<()> {
        let key = SwitchKey::from_key(key).ok_or_else(|| failure("That switch isn't known."))?;
        write_section(proxy, "compat", |o| {
            o.insert(key.key().into(), json!(on));
            Ok(())
        })
        .await
    }

    async fn save_fix(&self, proxy: &DesktopProxy<'_>, mut fix: Value) -> zbus::Result<()> {
        let id = fix["id"].as_str().map(normalise_id);
        if let (Some(id), Some(o)) = (&id, fix.as_object_mut()) {
            o.insert("id".into(), json!(id));
        }
        write_section(proxy, "compat", |o| {
            let apps = list_mut(o, "apps")?;
            match id.as_deref().and_then(|id| find(apps, "id", id)) {
                Some(i) => apps[i] = fix,
                None => apps.push(fix),
            }
            Ok(())
        })
        .await?;
        if let Some(id) = id {
            self.reopen_for(proxy, &id).await;
        }
        Ok(())
    }

    async fn remove_fix(&self, proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<()> {
        let id = normalise_id(id);
        write_section(proxy, "compat", |o| {
            let apps = list_mut(o, "apps")?;
            let i = find(apps, "id", &id).ok_or_else(|| failure(NO_FIX))?;
            apps.remove(i);
            Ok(())
        })
        .await?;
        self.reopen_for(proxy, &id).await;
        Ok(())
    }

    async fn set_elsewhere(
        &self,
        proxy: &DesktopProxy<'_>,
        id: &str,
        on: bool,
    ) -> zbus::Result<()> {
        let id = normalise_id(id);
        write_section(proxy, "compat", |o| {
            let apps = list_mut(o, "apps")?;
            let i = find(apps, "id", &id).ok_or_else(|| failure(NO_FIX))?;
            apps[i]["elsewhere"] = json!(on);
            Ok(())
        })
        .await
    }

    /// Whether any enabled screen is scaled; `false` when the outputs can't be read.
    async fn scaled(proxy: &DesktopProxy<'_>) -> zbus::Result<bool> {
        let outputs = match proxy.list_outputs().await.and_then(|t| parse_json(&t)) {
            Ok(v) => v,
            Err(e) if client::no_owner(&e) => return Err(e),
            Err(_) => return Ok(false),
        };
        let live = outputs["live"].as_array().map_or(&[][..], Vec::as_slice);
        Ok(live.iter().any(|o| {
            o["disabled"] != true && o["scale"].as_f64().is_some_and(|s| (s - 1.0).abs() > 1e-6)
        }))
    }
}

/// The state is the snapshot of the plan's Task 11: `{switches, browsers, blurry, fixes,
/// installed, fields, reopen, section}`.
impl PageLogic for CompatState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh | Op::RefreshWindows)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let p = &proxy;
        match op {
            Op::Refresh | Op::RefreshWindows => self.refresh_windows(p).await,
            Op::SetSwitch { key, on } => self.set_switch(p, &key, on).await,
            Op::SaveFix { fix } => self.save_fix(p, fix).await,
            Op::RemoveFix { id } => self.remove_fix(p, &id).await,
            Op::SetElsewhere { id, on } => self.set_elsewhere(p, &id, on).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let section = parse_json(&proxy.get_section("compat").await?)?;
        let explained = explain(&proxy, "compat").await?;
        let scaled = Self::scaled(&proxy).await?;
        let apps = self.installed().await?;
        let sharp_on = section["sharp_x11"] == true;
        let fixed: Vec<&str> = section["apps"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|f| f["id"].as_str())
            .collect();
        let windows = {
            let windows = self.windows.lock().unwrap();
            rows::blurry_windows(&windows, &apps, |id| fixed.contains(&id))
        };
        let installed: Vec<Value> = apps
            .iter()
            .map(|a| json!({"id": a.id, "name": a.name, "icon": a.icon}))
            .collect();
        let reopen = {
            let r = self.reopen.lock().unwrap();
            json!({"seq": r.seq, "names": r.names, "more": r.more})
        };
        Ok(json!({
            "switches": rows::switches(&section, &explained),
            "browsers": rows::browsers(&explained),
            "blurry": {"scaled": scaled, "sharp_on": sharp_on,
                "heading": rows::heading(scaled, sharp_on), "windows": windows},
            "fixes": rows::fixes(&section, &explained, &apps),
            "installed": installed,
            "fields": fields(),
            "reopen": reopen,
            "section": section_error(&explained, "compat"),
        }))
    }
}

#[cfg(test)]
mod tests;
