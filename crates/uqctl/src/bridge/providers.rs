//! look=see>do_ search: built-in rows plus third-party D-Bus launcher providers (spec §2.3).
//!
//! The built-ins: the calculator and Run, a typed web address (`web`), the user's files grouped
//! by type (`files:<type>`, files.rs), files by their text (`files:contents`, contents.rs) and
//! Settings' pages and settings (`settings`). Activating one of those only ever opens what this
//! generation's search showed.
//!
//! Provider results are untrusted text. Everything is clamped here before it reaches the shell,
//! only names that currently own a bus name are called (never auto-started), and answers for a
//! stale generation are dropped.

use super::calc::{self, ProviderItem};
use super::contents;
use super::files::{self, Files};
use super::launch;
use super::protocol::Outbound;
use super::run::{self, RunState};
use super::settings_search;
use super::uqd::Link;
use super::web;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uq_core::launcher_providers::{ProviderDef, load_defs};
use uq_core::settings::{ProviderPref, provider_enabled};
use zbus::names::WellKnownName;
use zbus::proxy::{CacheProperties, MethodFlags};
mod activate;
mod search;
use activate::*;

const SEARCH_TIMEOUT: Duration = Duration::from_millis(300);
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(1);
const LOG_EVERY: Duration = Duration::from_secs(60);
/// An activatable provider is started at most this often (Phase 4 spec §2.4.1).
pub(crate) const START_EVERY: Duration = Duration::from_secs(30);
/// A provider that fails to start is logged at most this often (§5).
pub(crate) const START_LOG_EVERY: Duration = Duration::from_secs(600);
/// `StartServiceByName` gets this long; nothing waits for it.
const START_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_ITEMS: usize = 20;
const MAX_ID_BYTES: usize = 512;
const MAX_TITLE_BYTES: usize = 200;
const MAX_SUBTITLE_BYTES: usize = 300;
const MAX_ACTIONS_BYTES: usize = 2048;
const MAX_ACTION_BYTES: usize = 256;
const DEFAULT_ICON: &str = "system-search";
const PATH: &str = uq_core::uq_consts::dbus::LAUNCHER_PROVIDER_PATH;
const IFACE: &str = uq_core::uq_consts::dbus::LAUNCHER_PROVIDER_INTERFACE;
/// Keeps an external row below any exact or prefix app-name match (which score 1.0).
const EXTERNAL_SCALE: f64 = 0.9;

/// The `(id, title, subtitle, icon, actions_json, score)` row a provider returns.
pub(crate) type RawItem = (String, String, String, String, String, f64);

/// The query to send this provider, or None when it should not see it.
/// `all` is every registered provider: a query starting with any registered prefix goes only
/// to the providers with that prefix.
pub(crate) fn route(query: &str, def: &ProviderDef, all: &[ProviderDef]) -> Option<String> {
    if query.starts_with('>') || query.starts_with('=') {
        return None;
    }
    match &def.prefix {
        Some(p) => {
            let rest = query.strip_prefix(p.as_str())?.trim();
            (!rest.is_empty()).then(|| rest.to_string())
        }
        None => {
            let prefixed = all
                .iter()
                .filter_map(|d| d.prefix.as_deref())
                .any(|p| query.starts_with(p));
            (!prefixed && query.chars().count() >= 2).then(|| query.to_string())
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

pub(crate) fn valid_icon(icon: &str) -> bool {
    let theme = icon.chars().any(|c| c.is_ascii_alphanumeric())
        && icon.len() <= 128
        && icon
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    let path = icon.starts_with('/')
        && icon.len() <= 512
        && icon
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/+ @,~".contains(c))
        && !icon
            .split('/')
            .skip(1)
            .any(|seg| seg.is_empty() || seg == "..")
        // Device and kernel files (`/dev/zero`, a FIFO under /proc) could stall the image loader.
        && !["dev", "proc", "sys"].contains(&icon.split('/').nth(1).unwrap_or(""));
    theme || path
}

/// The actions the provider offered: a JSON list of strings, whole entries only, at most 2 KiB
/// once serialised. Anything else is no actions.
fn clamp_actions(json: &str) -> Vec<String> {
    let Ok(all) = serde_json::from_str::<Vec<String>>(json) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    let mut size = 2;
    for a in all {
        let a = truncate(&a, MAX_ACTION_BYTES);
        size += serde_json::to_string(&a).map_or(usize::MAX, |s| s.len() + 1);
        if size > MAX_ACTIONS_BYTES {
            break;
        }
        out.push(a);
    }
    out
}

/// Limit one provider row: lengths, icon, actions and score (scaled below app-name matches).
pub(crate) fn clamp(item: RawItem) -> ProviderItem {
    let (id, title, subtitle, icon, actions, score) = item;
    let score = if score.is_nan() {
        0.0
    } else {
        score.clamp(0.0, 1.0)
    };
    ProviderItem {
        id,
        title: truncate(&title, MAX_TITLE_BYTES),
        subtitle: truncate(&subtitle, MAX_SUBTITLE_BYTES),
        icon: if valid_icon(&icon) {
            icon
        } else {
            DEFAULT_ICON.into()
        },
        score: score * EXTERNAL_SCALE,
        actions: clamp_actions(&actions),
    }
}

/// Clamp a provider's whole answer: rows whose id could not be activated are dropped, at most
/// 20 are kept.
pub(crate) fn clamp_all(items: Vec<RawItem>) -> Vec<ProviderItem> {
    items
        .into_iter()
        .filter(|i| !i.0.is_empty() && i.0.len() <= MAX_ID_BYTES)
        .take(MAX_ITEMS)
        .map(clamp)
        .collect()
}

/// The launcher provider preferences; with no settings yet (uqd not seen) every provider is on.
pub(super) fn provider_prefs(link: &Link) -> Vec<ProviderPref> {
    link.settings_rx()
        .borrow()
        .as_ref()
        .map(|s| s.shell.providers.clone())
        .unwrap_or_default()
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The definitions, re-read when a directory's mtime changes.
struct DefCache {
    dirs: Vec<PathBuf>,
    stamp: Option<Vec<Option<SystemTime>>>,
    /// Shared, so a search (one per keystroke) takes it without copying every definition.
    defs: Arc<Vec<ProviderDef>>,
}

impl DefCache {
    fn current(&mut self) -> Arc<Vec<ProviderDef>> {
        let now: Vec<Option<SystemTime>> = self
            .dirs
            .iter()
            .map(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok())
            .collect();
        if self.stamp.as_ref() != Some(&now) {
            self.defs = Arc::new(load_defs(&self.dirs));
            self.stamp = Some(now);
        }
        self.defs.clone()
    }
}

#[derive(Default)]
struct State {
    generation: u64,
    /// The newest generation's query, so an index that finishes later can answer it.
    query: String,
    /// The query behind the calculator row of the newest generation.
    calc: Option<String>,
    /// The web address the newest generation offered.
    web: Option<String>,
    /// The files the newest generation showed: only these can be opened.
    files: HashSet<String>,
}

/// The preference id a provider is switched on or off by: every file group is `files`.
fn pref_id(provider: &str) -> &str {
    if provider.starts_with("files:") {
        "files"
    } else {
        provider
    }
}

/// Whether the built-in search rows (web, files, Settings) see this query: not a `>` command,
/// `=` sum or another provider's prefix (`:` emoji, …).
fn builtin_sees(query: &str, defs: &[ProviderDef]) -> bool {
    !query.is_empty()
        && !query.starts_with(['>', '=', ':'])
        && !defs
            .iter()
            .filter_map(|d| d.prefix.as_deref())
            .any(|p| query.starts_with(p))
}

pub(crate) struct Providers {
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
    state: Mutex<State>,
    defs: Mutex<DefCache>,
    logged: Mutex<HashMap<String, Instant>>,
    starts: Mutex<StartThrottle>,
    files: Mutex<Files>,
    settings: Mutex<settings_search::Index>,
}

fn results_event(
    generation: u64,
    provider: &str,
    provider_name: &str,
    source_dir: &str,
    items: &[ProviderItem],
) -> Outbound {
    Outbound::Event {
        event: "SearchResults".into(),
        data: json!({
            "generation": generation,
            "provider": provider,
            "provider_name": provider_name,
            "source_dir": source_dir,
            "items": items,
        }),
    }
}

async fn owned(conn: &zbus::Connection, name: &str) -> bool {
    let Ok(wk) = WellKnownName::try_from(name) else {
        return false;
    };
    match zbus::fdo::DBusProxy::new(conn).await {
        Ok(p) => p.name_has_owner(wk.into()).await.unwrap_or(false),
        Err(_) => false,
    }
}

async fn proxy<'a>(conn: &'a zbus::Connection, name: &'a str) -> zbus::Result<zbus::Proxy<'a>> {
    zbus::proxy::Builder::new(conn)
        .destination(name)
        .path(PATH)
        .interface(IFACE)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

impl Providers {
    pub(crate) fn new(
        tx: mpsc::Sender<Outbound>,
        stop: CancellationToken,
        dirs: Vec<PathBuf>,
    ) -> Arc<Providers> {
        Arc::new(Providers {
            tx,
            stop,
            state: Mutex::new(State::default()),
            defs: Mutex::new(DefCache {
                dirs,
                stamp: None,
                defs: Arc::default(),
            }),
            logged: Mutex::default(),
            starts: Mutex::default(),
            files: Mutex::new(Files {
                roots: Arc::new(files::default_roots()),
                ..Files::default()
            }),
            settings: Mutex::default(),
        })
    }

    /// Build the file index on a blocking thread when it is missing or stale; when it is done,
    /// answer the newest query from it.
    fn refresh_files(self: &Arc<Self>) {
        let roots = {
            let mut f = lock(&self.files);
            if !f.wants_build(Instant::now()) {
                return;
            }
            f.building = true;
            f.roots.clone()
        };
        let me = self.clone();
        tokio::spawn(async move {
            let built =
                tokio::task::spawn_blocking(move || files::build(&roots, SystemTime::now())).await;
            {
                let mut f = lock(&me.files);
                f.building = false;
                if let Ok(idx) = built {
                    f.index = Arc::new(idx);
                    f.built = Some(Instant::now());
                }
            }
            me.answer_latest(true, false).await;
        });
    }

    /// Read Settings' pages and settings when missing, stale or Settings changed; when done,
    /// answer the newest query from them.
    fn refresh_settings(self: &Arc<Self>) {
        let Some(bin) = settings_search::binary() else {
            return;
        };
        {
            let mut s = lock(&self.settings);
            if !s.wants_read(Instant::now(), &bin) {
                return;
            }
            s.reading = true;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let got = tokio::select! {
                _ = me.stop.cancelled() => None,
                got = settings_search::read(bin) => got,
            };
            {
                let mut s = lock(&me.settings);
                s.reading = false;
                s.read = Some(Instant::now());
                if let Some((rows, stamp)) = got {
                    s.rows = rows;
                    s.stamp = stamp;
                }
            }
            me.answer_latest(false, true).await;
        });
    }

    /// Log a provider error at most once a minute per provider.
    fn log_once(&self, who: &str, msg: &str) {
        let mut l = lock(&self.logged);
        let now = Instant::now();
        if l.get(who)
            .is_none_or(|t| now.duration_since(*t) >= LOG_EVERY)
        {
            l.insert(who.to_string(), now);
            eprintln!("uqctl bridge: provider {who}: {msg}");
        }
    }
}

#[cfg(test)]
mod tests;
