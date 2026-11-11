//! Spotlight search: built-in rows plus third-party D-Bus launcher providers (spec §2.3).
//!
//! Provider results are untrusted text. Everything is clamped here before it reaches the shell,
//! only names that currently own a bus name are called (never auto-started), and answers for a
//! stale generation are dropped.

use super::calc::{self, ProviderItem};
use super::launch;
use super::protocol::Outbound;
use super::run::{self, RunState};
use super::uqd::Link;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uq_core::launcher_providers::{ProviderDef, load_defs};
use uq_core::settings::{ProviderPref, provider_enabled};
use zbus::names::WellKnownName;
use zbus::proxy::{CacheProperties, MethodFlags};

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
const PATH: &str = "/org/uq/Launcher1";
const IFACE: &str = "org.uq.Launcher1.Provider";
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

fn valid_icon(icon: &str) -> bool {
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
    defs: Vec<ProviderDef>,
}

impl DefCache {
    fn current(&mut self) -> Vec<ProviderDef> {
        let now: Vec<Option<SystemTime>> = self
            .dirs
            .iter()
            .map(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok())
            .collect();
        if self.stamp.as_ref() != Some(&now) {
            self.defs = load_defs(&self.dirs);
            self.stamp = Some(now);
        }
        self.defs.clone()
    }
}

/// When each activatable provider was last started and when its failure was last logged.
#[derive(Debug, Default)]
pub(crate) struct StartThrottle {
    tried: HashMap<String, Instant>,
    logged: HashMap<String, Instant>,
}

fn due(map: &mut HashMap<String, Instant>, name: &str, now: Instant, every: Duration) -> bool {
    if map
        .get(name)
        .is_some_and(|t| now.saturating_duration_since(*t) < every)
    {
        return false;
    }
    map.insert(name.to_string(), now);
    true
}

impl StartThrottle {
    /// Whether `name` may be started now (at most once per [`START_EVERY`]); records the try.
    pub(crate) fn try_start(&mut self, name: &str, now: Instant) -> bool {
        due(&mut self.tried, name, now, START_EVERY)
    }

    /// Whether a failure of `name` should be logged now (at most once per [`START_LOG_EVERY`]).
    pub(crate) fn should_log(&mut self, name: &str, now: Instant) -> bool {
        due(&mut self.logged, name, now, START_LOG_EVERY)
    }
}

#[derive(Default)]
struct State {
    generation: u64,
    /// The query behind the calculator row of the newest generation.
    calc: Option<String>,
}

pub(crate) struct Providers {
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
    state: Mutex<State>,
    defs: Mutex<DefCache>,
    logged: Mutex<HashMap<String, Instant>>,
    starts: Mutex<StartThrottle>,
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
        .destination(name)?
        .path(PATH)?
        .interface(IFACE)?
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
                defs: Vec::new(),
            }),
            logged: Mutex::default(),
            starts: Mutex::default(),
        })
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

    /// The bridge's `LauncherOpened` call (Spotlight or the app menu opened): start each
    /// switched-on `Activatable=true` provider whose name has no owner and has a session service
    /// file, at most once per 30 s each, without waiting (§2.4.1). Queries still go only to
    /// owned names, with `NO_AUTO_START`.
    pub(crate) async fn launcher_opened(
        self: &Arc<Self>,
        link: &Link,
        args: &[Value],
    ) -> Result<Value, String> {
        if !args.is_empty() {
            return Err("LauncherOpened takes no arguments".into());
        }
        let prefs = provider_prefs(link);
        let defs: Vec<ProviderDef> = lock(&self.defs)
            .current()
            .into_iter()
            .filter(|d| d.activatable && provider_enabled(&prefs, &d.bus_name))
            .collect();
        if defs.is_empty() {
            return Ok(Value::Null);
        }
        let Some(conn) = link.connection().await else {
            return Ok(Value::Null);
        };
        let me = self.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = me.stop.cancelled() => {}
                _ = me.start_cold(&conn, defs) => {}
            }
        });
        Ok(Value::Null)
    }

    async fn start_cold(self: &Arc<Self>, conn: &zbus::Connection, defs: Vec<ProviderDef>) {
        let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
            return;
        };
        let names = match tokio::time::timeout(START_TIMEOUT, dbus.list_activatable_names()).await {
            Ok(Ok(n)) => n,
            _ => return,
        };
        for def in defs {
            if !names.iter().any(|n| n.as_str() == def.bus_name) {
                // Activatable=true without a service file: the key is ignored (§2.4.1).
                if lock(&self.starts).should_log(&def.bus_name, Instant::now()) {
                    eprintln!(
                        "uqctl bridge: provider {}: Activatable=true but no D-Bus service file",
                        def.bus_name
                    );
                }
                continue;
            }
            if owned(conn, &def.bus_name).await
                || !lock(&self.starts).try_start(&def.bus_name, Instant::now())
            {
                continue;
            }
            let (me, conn) = (self.clone(), conn.clone());
            tokio::spawn(async move {
                let name = def.bus_name.as_str();
                let start = async {
                    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
                    let wk = WellKnownName::try_from(name).map_err(zbus::Error::from)?;
                    dbus.start_service_by_name(wk, 0)
                        .await
                        .map_err(zbus::Error::from)
                };
                let err = match tokio::time::timeout(START_TIMEOUT, start).await {
                    Ok(Ok(_)) => return,
                    Ok(Err(e)) => e.to_string(),
                    Err(_) => "didn't start within 2 s".to_string(),
                };
                if lock(&me.starts).should_log(name, Instant::now()) {
                    eprintln!("uqctl bridge: provider {name}: couldn't start: {err}");
                }
            });
        }
    }

    fn latest(&self) -> u64 {
        lock(&self.state).generation
    }

    /// The bridge's `Search` call. Built-ins are emitted before this returns; external providers
    /// answer later, from tasks that stop with the bridge.
    pub(crate) async fn search(
        self: &Arc<Self>,
        link: &Link,
        runs: &Mutex<RunState>,
        args: &[Value],
    ) -> Result<Value, String> {
        let [Value::String(query), generation] = args else {
            return Err("Search takes a query string and a generation".into());
        };
        let generation = generation.as_u64().ok_or("generation must be a u64")?;
        let query = query.trim();
        let prefs = provider_prefs(link);
        let mut builtin: Vec<(&str, &str, ProviderItem)> = Vec::new();
        {
            let mut st = lock(&self.state);
            if generation < st.generation {
                return Ok(Value::Null);
            }
            st.generation = generation;
            st.calc = None;
            if provider_enabled(&prefs, "calc")
                && !query.starts_with('>')
                && calc::applies(query)
                && let Some(it) = calc::item(query)
            {
                st.calc = Some(query.to_string());
                builtin.push(("calc", "Calculator", it));
            }
            // Always called, so a stale command is cleared by any later query. With Run off it
            // sees an empty query: nothing is stored, so nothing can be activated either.
            let run_query = if provider_enabled(&prefs, "run") {
                query
            } else {
                ""
            };
            if let Some(it) = run::run_item(run_query, generation, &mut lock(runs)) {
                builtin.push(("run", "Run", it));
            }
        }
        for (id, name, it) in builtin {
            let ev = results_event(generation, id, name, "builtin", &[it]);
            let _ = self.tx.send(ev).await;
        }

        let defs = lock(&self.defs).current();
        let all = defs.clone();
        let routed: Vec<(ProviderDef, String)> = defs
            .into_iter()
            .filter(|d| provider_enabled(&prefs, &d.bus_name))
            .filter_map(|d| route(query, &d, &all).map(|q| (d, q)))
            .collect();
        if routed.is_empty() {
            return Ok(Value::Null);
        }
        let Some(conn) = link.connection().await else {
            return Ok(Value::Null);
        };
        for (def, q) in routed {
            let (me, conn) = (self.clone(), conn.clone());
            tokio::spawn(async move {
                tokio::select! {
                    _ = me.stop.cancelled() => {}
                    _ = me.ask(&conn, &def, &q, generation) => {}
                }
            });
        }
        Ok(Value::Null)
    }

    async fn ask(&self, conn: &zbus::Connection, def: &ProviderDef, q: &str, generation: u64) {
        if !owned(conn, &def.bus_name).await {
            return;
        }
        let call = async {
            let p = proxy(conn, &def.bus_name).await?;
            p.call_with_flags::<_, _, Vec<RawItem>>(
                "Search",
                MethodFlags::NoAutoStart.into(),
                &(q,),
            )
            .await
        };
        let raw = match tokio::time::timeout(SEARCH_TIMEOUT, call).await {
            Ok(Ok(Some(r))) => r,
            Ok(Ok(None)) => return,
            Ok(Err(e)) => return self.log_once(&def.bus_name, &e.to_string()),
            Err(_) => return self.log_once(&def.bus_name, "no answer within 300 ms"),
        };
        if self.latest() != generation {
            return;
        }
        let items = clamp_all(raw);
        let dir = def.source_dir.to_string_lossy();
        let ev = results_event(generation, &def.bus_name, &def.name, &dir, &items);
        let _ = self.tx.send(ev).await;
    }

    /// The bridge's `Activate` call.
    pub(crate) async fn activate(
        &self,
        link: &Link,
        runs: &Mutex<RunState>,
        args: &[Value],
    ) -> Result<Value, String> {
        let [
            Value::String(provider),
            Value::String(id),
            Value::String(action),
        ] = args
        else {
            return Err("Activate takes provider, id and action strings".into());
        };
        // Checked now, not only in `search`: a row shown before the provider was switched off
        // must not act.
        if !provider_enabled(&provider_prefs(link), provider) {
            match provider.as_str() {
                "calc" => lock(&self.state).calc = None,
                "run" => lock(runs).forget(),
                _ => {}
            }
            return Err(format!("{provider} is switched off"));
        }
        match provider.as_str() {
            "calc" => {
                let q = lock(&self.state).calc.clone();
                match (id.as_str(), q.as_deref().and_then(calc::evaluate)) {
                    ("calc", Some(result)) => Ok(json!({ "copy": result })),
                    _ => Err("unknown calculator result".into()),
                }
            }
            "run" => launch::launch(link, &[json!({ "run_id": id })], runs).await,
            _ => {
                if id.is_empty() || id.len() > MAX_ID_BYTES || action.len() > MAX_ACTION_BYTES {
                    return Err("id or action too long".into());
                }
                let defs = lock(&self.defs).current();
                let def = defs
                    .iter()
                    .find(|d| d.bus_name == *provider)
                    .ok_or("unknown provider")?;
                let conn = link.connection().await.ok_or("no session bus")?;
                if !owned(&conn, &def.bus_name).await {
                    return Err(format!("{} is not running", def.name));
                }
                let call = async {
                    let p = proxy(&conn, &def.bus_name).await?;
                    p.call_with_flags::<_, _, ()>(
                        "Activate",
                        MethodFlags::NoAutoStart.into(),
                        &(id.as_str(), action.as_str()),
                    )
                    .await
                };
                match tokio::time::timeout(ACTIVATE_TIMEOUT, call).await {
                    Ok(Ok(_)) => Ok(Value::Null),
                    Ok(Err(e)) => {
                        self.log_once(&def.bus_name, &e.to_string());
                        Err(format!("{} failed", def.name))
                    }
                    Err(_) => {
                        self.log_once(&def.bus_name, "Activate timed out");
                        Err(format!("{} did not answer", def.name))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(prefix: Option<&str>) -> ProviderDef {
        ProviderDef {
            bus_name: "org.example.P".into(),
            name: "P".into(),
            prefix: prefix.map(String::from),
            source_dir: "/x".into(),
            activatable: false,
        }
    }

    fn raw(id: &str, title: &str, icon: &str, score: f64) -> RawItem {
        (
            id.into(),
            title.into(),
            String::new(),
            icon.into(),
            "[]".into(),
            score,
        )
    }

    #[test]
    fn starts_at_most_every_30_s_and_logs_every_10_min() {
        let mut t = StartThrottle::default();
        let t0 = Instant::now();
        assert!(t.try_start("a", t0));
        assert!(!t.try_start("a", t0 + Duration::from_secs(29)));
        assert!(t.try_start("b", t0), "per provider");
        assert!(t.try_start("a", t0 + Duration::from_secs(30)));
        assert!(t.should_log("a", t0));
        assert!(!t.should_log("a", t0 + Duration::from_secs(599)));
        assert!(t.should_log("a", t0 + Duration::from_secs(600)));
    }

    #[test]
    fn route_prefix_and_gating() {
        let p = def(Some(":"));
        let n = def(None);
        let all = [p.clone(), n.clone()];
        assert_eq!(route(":smi", &p, &all).as_deref(), Some("smi"));
        assert_eq!(route(":smi", &n, &all), None);
        assert_eq!(route("smi", &p, &all), None);
        assert_eq!(route("smi", &n, &all).as_deref(), Some("smi"));
        assert_eq!(route("a", &n, &all), None);
        assert_eq!(route(">ls", &n, &all), None);
        assert_eq!(route("=1+1", &n, &all), None);
        assert_eq!(
            route("ab", &n, std::slice::from_ref(&n)).as_deref(),
            Some("ab")
        );
    }

    #[test]
    fn clamp_limits() {
        let items: Vec<RawItem> = (0..25).map(|i| raw(&i.to_string(), "t", "", 0.5)).collect();
        assert_eq!(clamp_all(items).len(), 20);
        let it = clamp(raw("a", &"é".repeat(150), "", 0.5));
        assert_eq!(it.title.len(), 200);
        assert!(it.title.chars().all(|c| c == 'é'));
        let mut r = raw("a", "t", "", 0.5);
        r.2 = "s".repeat(400);
        assert_eq!(clamp(r).subtitle.len(), 300);
        let mut r = raw("a", "t", "", 0.5);
        r.4 = serde_json::to_string(&vec!["x".repeat(100); 40]).unwrap();
        let a = clamp(r).actions;
        assert!(!a.is_empty() && serde_json::to_string(&a).unwrap().len() <= 2048);
        let mut r = raw("a", "t", "", 0.5);
        r.4 = "{not a list".into();
        assert!(clamp(r).actions.is_empty());
        assert!(clamp_all(vec![raw(&"i".repeat(513), "t", "", 0.5)]).is_empty());
    }

    #[test]
    fn score_clamped_and_scaled() {
        assert!((clamp(raw("a", "t", "", 5.0)).score - 0.9).abs() < 1e-12);
        assert_eq!(clamp(raw("a", "t", "", -1.0)).score, 0.0);
        assert_eq!(clamp(raw("a", "t", "", f64::NAN)).score, 0.0);
    }

    #[test]
    fn icon_clamped_to_theme_name_or_local_path() {
        for ok in ["firefox", "/usr/share/x.png", "org.kde.app-1_x"] {
            assert_eq!(clamp(raw("a", "t", ok, 0.5)).icon, ok);
        }
        for bad in [
            "http://x/y.png",
            "file:///x",
            "../x",
            "<img>",
            "",
            "/a/../b",
            "/a?b",
            "//host/x.png",
            "/a//b",
            ".",
            "..",
            "...",
            "/dev/zero",
            "/proc/self/fd/0",
            "/sys/kernel/x.png",
            "/dev",
        ] {
            assert_eq!(
                clamp(raw("a", "t", bad, 0.5)).icon,
                "system-search",
                "{bad}"
            );
        }
    }
}
