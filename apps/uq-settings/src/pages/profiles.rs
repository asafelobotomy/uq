//! The Profiles page's logic: [`ProfilesState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::profiles_model` is the thin QML object around it.
//!
//! A profile is a named set of settings that apply together (spec §4). uqd keeps the profiles
//! and applies them; this page makes them from the current values (of whole pages, or of what
//! changed since a time), edits them as their file text (read, change, write back with
//! `replace`), and exports and imports that text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::atomic::atomic_write;
use uq_core::client::DesktopProxy;
use uq_core::history::Entry;
use uq_core::keybinds::Keybinds;
use uq_core::profile::{
    DomainKind, Key, PROFILE_DOMAINS, Profile, ProfileError, bind_keys, bind_label, changed_since,
    from_journal, id_for_name, key_label, leaves, power_label, read_file, rule_keys, rule_label,
    value_text,
};
use uq_core::rules::Rules;

use crate::desktop_entries;
use crate::pages::runner::{PageLogic, parse_json};
use crate::registry;
use startup::Startup;

mod startup;
mod values;
use values::*;

/// The icons a profile can have, offered by the page.
pub const ICONS: [&str; 7] = [
    "preferences-desktop",
    "input-gaming",
    "video-display",
    "battery",
    "weather-clear-night",
    "applications-office",
    "media-playback-start",
];

/// How far back "changed since" looks in the journal.
const HISTORY_LIMIT: u32 = 500;

/// The error name uqd's `SetProfile` answers when the id exists and `replace` is false.
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    Apply {
        id: String,
    },
    /// "Back to normal".
    Off,
    /// A new profile with the current values of these pages' settings.
    CreateFromPages {
        name: String,
        icon: String,
        pages: Vec<String>,
    },
    /// A new profile with the current values of the settings changed since `since` (Unix seconds).
    CreateFromHistory {
        name: String,
        icon: String,
        since: i64,
    },
    /// Add `(domain, path)` keys to a profile, with their current values.
    AddKeys {
        id: String,
        keys: Vec<(String, Vec<String>)>,
    },
    RemoveKey {
        id: String,
        domain: String,
        path: Vec<String>,
    },
    Rename {
        id: String,
        name: String,
        icon: String,
    },
    Delete {
        id: String,
    },
    Export {
        id: String,
        path: PathBuf,
    },
    /// `replace` answers a conflict: a profile with the file's id exists.
    Import {
        path: PathBuf,
        replace: bool,
    },
}

/// The last confirmation to show, numbered so the page shows each once.
#[derive(Debug, Default)]
struct Notice {
    seq: u64,
    text: String,
}

/// What the last apply or "Back to normal" said: keys kept because they were changed by hand
/// (plain labels), and notices (the power mode).
#[derive(Debug, Default)]
struct Report {
    seq: u64,
    kept: Vec<String>,
    notices: Vec<String>,
}

/// An import uqd refused because a profile with its id exists: the page asks to replace it.
#[derive(Debug)]
struct Conflict {
    seq: u64,
    path: String,
    /// uqd's message without its closing "Replace it?": the page's dialog asks the question.
    message: String,
}

/// uqd's conflict message without the question it ends with.
fn conflict_message(text: &str) -> String {
    let text = text.trim();
    text.strip_suffix("Replace it?")
        .map_or(text, str::trim_end)
        .to_string()
}

#[derive(Debug, Default)]
struct Shown {
    seq: u64,
    notice: Notice,
    report: Report,
    conflict: Option<Conflict>,
}

impl Shown {
    fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

pub struct ProfilesState {
    conn: zbus::Connection,
    /// Where installed apps are looked up, to name app fixes.
    app_dirs: Vec<PathBuf>,
    /// Edits read-modify-write a profile, so one at a time.
    write: Mutex<()>,
    shown: StdMutex<Shown>,
}

fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

fn profile_error(e: ProfileError) -> zbus::Error {
    failure(e.to_string())
}

/// Whether a profile can hold keys of journal domain `domain`. For `rules`, only the switches;
/// for startup apps, variables and app fixes, only what a profile can set (`startup.rs`).
fn holds_journal_domain(domain: &str) -> bool {
    matches!(domain, "theme" | "rules" | "keybinds")
        || startup::DOMAINS.contains(&domain)
        || from_journal(domain, &[]).is_some()
}

async fn read_rules(proxy: &DesktopProxy<'_>) -> zbus::Result<Rules> {
    serde_json::from_value(parse_json(&proxy.get_section("rules").await?)?)
        .map_err(|_| failure("uQ's settings service sent unreadable rules"))
}

fn icon_of(icon: &str) -> Option<String> {
    let icon = icon.trim();
    (!icon.is_empty()).then(|| icon.to_string())
}

impl ProfilesState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self::with_dirs(conn, desktop_entries::default_dirs())
    }

    pub fn with_dirs(conn: zbus::Connection, app_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            app_dirs,
            write: Mutex::new(()),
            shown: StdMutex::new(Shown::default()),
        }
    }

    fn notice(&self, text: String) {
        let mut shown = self.shown.lock().unwrap();
        let seq = shown.next();
        shown.notice = Notice { seq, text };
    }

    /// The count of keys that couldn't go in a profile, as a notice.
    fn skipped(&self, n: usize) {
        if n > 0 {
            self.notice(format!("{n} of those changes can't go in a profile"));
        }
    }

    async fn ids(proxy: &DesktopProxy<'_>) -> zbus::Result<Vec<String>> {
        let list = parse_json(&proxy.list_profiles().await?)?;
        Ok(list
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["id"].as_str().map(str::to_string))
            .collect())
    }

    async fn load(proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<Profile> {
        Profile::parse(&proxy.get_profile(id).await?).map_err(profile_error)
    }
}

/// The state is `{profiles: [{id, name, icon, active, power, keys: [{domain, path, label,
/// text}]}], active, available: [{domain, path, label, text}], pages: [{id, name}], icons,
/// notice: {seq, text}, report: {seq, kept, notices}, conflict: {seq, path, message} | null}`.
/// `power` is the power mode's plain name, or `""`; the power mode is also one of `keys`, so it
/// can be removed.
impl PageLogic for ProfilesState {
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
        if matches!(op, Op::Refresh) {
            return Ok(());
        }
        self.shown.lock().unwrap().conflict = None;
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::Apply { id } => self.report(&proxy.apply_profile(&id).await?),
            Op::Off => self.report(&proxy.profile_off().await?),
            Op::CreateFromPages { name, icon, pages } => {
                let mut domains: Vec<&str> = Vec::new();
                for page in pages.iter().filter_map(|p| registry::find(p)) {
                    for d in page.domains {
                        if !domains.contains(d) {
                            domains.push(d);
                        }
                    }
                }
                let keys = current(&proxy, &domains).await?;
                self.create(&proxy, &name, &icon, keys).await
            }
            Op::CreateFromHistory { name, icon, since } => {
                let text = proxy.list_history(HISTORY_LIMIT).await?;
                let newest_first: Vec<Entry> = serde_json::from_str(&text).map_err(|e| {
                    failure(format!(
                        "uQ's settings service sent unreadable history: {e}"
                    ))
                })?;
                let (changed, cant) = changed_since(&newest_first, since);
                let (keys, unset) = with_values(&proxy, changed).await?;
                self.create(&proxy, &name, &icon, keys).await?;
                self.skipped(cant + unset);
                Ok(())
            }
            Op::AddKeys { id, keys } => {
                let (keys, unset) = with_values(&proxy, keys).await?;
                Self::edit(&proxy, &id, |p| {
                    for key in keys {
                        p.with_key(key);
                    }
                })
                .await?;
                self.skipped(unset);
                Ok(())
            }
            Op::RemoveKey { id, domain, path } => {
                Self::edit(&proxy, &id, |p| {
                    p.without_key(&domain, &path);
                })
                .await
            }
            Op::Rename { id, name, icon } => {
                Self::edit(&proxy, &id, |p| {
                    p.name = name.trim().to_string();
                    p.icon = icon_of(&icon);
                })
                .await
            }
            Op::Delete { id } => proxy.delete_profile(&id).await,
            Op::Export { id, path } => {
                let text = proxy.get_profile(&id).await?;
                atomic_write(&path, &text)
                    .map_err(|e| failure(format!("Can't save the file: {e}")))?;
                // Variables can hold things like tokens; the file carries them as they are.
                if Profile::parse(&text).is_ok_and(|p| p.values.contains_key("env")) {
                    self.notice(startup::EXPORT_NOTICE.to_string());
                }
                Ok(())
            }
            Op::Import { path, replace } => self.import(&proxy, &path, replace).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let list = parse_json(&proxy.list_profiles().await?)?;
        // Read once: they name the rule switches, startup apps and fixes, and give their
        // current values.
        let rules = read_rules(&proxy).await?;
        let binds = read_binds(&proxy).await?;
        let apps = Startup::installed(self.app_dirs.clone()).await;
        let startup = Startup::read(&proxy, apps).await?;
        let mut profiles = Vec::new();
        let mut active = String::new();
        for summary in list.as_array().into_iter().flatten() {
            let Some(id) = summary["id"].as_str() else {
                continue;
            };
            // Deleted since the list was read, say.
            let Ok(p) = Self::load(&proxy, id).await else {
                continue;
            };
            let is_active = summary["active"].as_bool().unwrap_or(false);
            if is_active {
                active = p.id.clone();
            }
            let keys: Vec<Value> = p
                .keys()
                .iter()
                .map(|k| key_json(&k.domain, &k.path, &k.value, &rules, &binds, &startup))
                .collect();
            profiles.push(json!({
                "id": p.id,
                "name": p.name,
                "icon": p.icon.clone().unwrap_or_default(),
                "active": is_active,
                "power": p.power().map(power_label).unwrap_or_default(),
                "keys": keys,
            }));
        }
        let available: Vec<Value> =
            current_with(&proxy, &profile_journal_domains(), &rules, &binds, &startup)
                .await?
                .iter()
                .map(|k| key_json(&k.domain, &k.path, &k.value, &rules, &binds, &startup))
                .collect();
        let pages: Vec<Value> = registry::core_pages()
            .iter()
            .filter(|p| p.domains.iter().any(|d| holds_journal_domain(d)))
            .map(|p| json!({"id": p.id, "name": p.name}))
            .collect();
        let shown = self.shown.lock().unwrap();
        Ok(json!({
            "profiles": profiles,
            "active": active,
            "available": available,
            "pages": pages,
            "icons": ICONS,
            "notice": {"seq": shown.notice.seq, "text": shown.notice.text},
            "report": {
                "seq": shown.report.seq,
                "kept": shown.report.kept,
                "notices": shown.report.notices,
            },
            "conflict": shown.conflict.as_ref().map(|c| json!({
                "seq": c.seq,
                "path": c.path,
                "message": c.message,
            })),
        }))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod startup_tests;
