//! Widgets from other apps (Phase 4 spec §2.2): `/usr/share/uq/shell/widgets/<id>/`.
//!
//! The bridge scans the directory at start and on every change (inotify, 1 s debounce),
//! validates each `widget.toml` with `uq_core::integration`, tracks the owner of every
//! `requires.bus_name` and `[dbus] name` through `NameOwnerChanged`, and tells the shell with a
//! `Widgets` event whenever the list or a `visible` flag changes. Widget QML reaches D-Bus and
//! commands only through `WidgetCall`, `WidgetGet` and `WidgetRun`, which accept only what the
//! manifest lists; `WidgetProps` and `WidgetSignal` carry its listed properties and signals.
//!
//! The limits are enforced here, not in QML: listed members only, types through
//! `dbus_json`, 16 KiB of arguments, 256 KiB of results, 2 s per call, at most 8 calls in flight
//! per widget, and `NO_AUTO_START` unless the widget has no `requires.bus_name` and its name is
//! activatable.

use super::protocol::Outbound;
use futures_util::StreamExt;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::AbortHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use uq_core::integration::command::{self, BusySet, CommandError, Runner};
use uq_core::integration::dbus_json::{
    self, CALL_DEADLINE, DbusJsonError, MAX_RESULT_JSON, Target, parse_signature, value_to_json,
};
use uq_core::integration::manifest::{Bus, DbusSpec, Slot, WidgetManifest};
use uq_core::integration::scan::scan_widgets;
use uq_core::integration::watch::{DEBOUNCE, watch_dirs};
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::wire::{OwnedVariant as OwnedValue, Structure};
use zbus::{MatchRule, MessageStream};
mod subs;
use subs::*;
mod calls;
use calls::*;

/// D-Bus calls in flight per widget (§2.2).
pub const MAX_IN_FLIGHT: usize = 8;
/// `WidgetProps` at most four times a second per widget (§2.2).
pub const PROPS_EVERY: Duration = Duration::from_millis(250);
/// How long to wait before reconnecting to a bus that went away.
const RETRY: Duration = Duration::from_secs(5);
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A lazily opened connection to one bus (the session bus, or the system bus for
/// `bus = "system"` tables). Tests point both at a private bus.
pub(crate) struct BusConn {
    addr: Option<String>,
    bus: Bus,
    conn: tokio::sync::Mutex<Option<zbus::Connection>>,
}

impl BusConn {
    pub(crate) fn new(addr: Option<&str>, bus: Bus) -> BusConn {
        BusConn {
            addr: addr.map(str::to_owned),
            bus,
            conn: tokio::sync::Mutex::new(None),
        }
    }

    /// The connection, opening it (within 2 s) if needed.
    pub(crate) async fn get(&self) -> Option<zbus::Connection> {
        let mut slot = self.conn.lock().await;
        if let Some(c) = slot.as_ref() {
            return Some(c.clone());
        }
        let open = async {
            match (&self.addr, self.bus) {
                (Some(a), _) => zbus::connection::Builder::address(a.as_str()).build().await,
                (None, Bus::Session) => zbus::Connection::session().await,
                (None, Bus::System) => zbus::Connection::system().await,
            }
        };
        let conn = tokio::time::timeout(CALL_DEADLINE, open).await.ok()?.ok()?;
        *slot = Some(conn.clone());
        Some(conn)
    }

    /// Forget the connection (it went away); the next `get` opens a new one.
    async fn reset(&self) {
        *self.conn.lock().await = None;
    }
}

/// A widget that loaded: its manifest, QML URL, and whether its `requires.bins` exist.
#[derive(Debug, Clone)]
struct Known {
    manifest: WidgetManifest,
    qml_url: String,
    /// A digest of the folder's files, so the shell can tell its code changed (see `content_rev`).
    rev: String,
    bins_ok: bool,
}

impl Known {
    /// The bus `requires.bus_name` and `[dbus]` live on.
    fn bus(&self) -> Bus {
        self.manifest.dbus.as_ref().map_or(Bus::Session, |d| d.bus)
    }

    /// The names whose owners this widget depends on, with their bus.
    fn names(&self) -> Vec<(Bus, String)> {
        let mut out = Vec::new();
        if let Some(n) = &self.manifest.requires.bus_name {
            out.push((self.bus(), n.clone()));
        }
        if let Some(d) = &self.manifest.dbus {
            out.push((d.bus, d.name.clone()));
        }
        out
    }
}

/// `file://` URL of a local path, percent-encoding anything outside a conservative set.
pub fn file_url(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/._-~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A digest of the files in a widget's folder (names, sizes, modification times; one level of
/// sub-folders). The shell keeps compiled QML for the life of the session, keyed by URL; when
/// this changes it loads the widget under a new spelling of its URL, so a package update's new
/// code runs without restarting the shell.
pub fn content_rev(dir: &Path) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    fn files(dir: &Path, depth: u8, out: &mut Vec<(String, u64, u128)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            let path = e.path();
            if md.is_dir() {
                if depth > 0 {
                    files(&path, depth - 1, out);
                }
                continue;
            }
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            out.push((path.to_string_lossy().into_owned(), md.len(), mtime));
        }
    }
    let mut all = Vec::new();
    files(dir, 1, &mut all);
    all.sort();
    let mut h = DefaultHasher::new();
    all.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Whether a widget shows: its programs exist and, with a `requires.bus_name`, that name has an
/// owner.
fn is_visible(k: &Known, owners: &HashMap<(Bus, String), Option<String>>) -> bool {
    k.bins_ok
        && k.manifest.requires.bus_name.as_ref().is_none_or(|n| {
            owners
                .get(&(k.bus(), n.clone()))
                .is_some_and(Option::is_some)
        })
}

/// The `Widgets` event data: `[{id, name, slot, order, qml_url, rev, visible}]` by order, then id.
fn list_json(
    widgets: &BTreeMap<String, Known>,
    owners: &HashMap<(Bus, String), Option<String>>,
) -> Value {
    let mut all: Vec<&Known> = widgets.values().collect();
    all.sort_by(|a, b| (a.manifest.order, &a.manifest.id).cmp(&(b.manifest.order, &b.manifest.id)));
    Value::Array(
        all.into_iter()
            .map(|k| {
                json!({
                    "id": k.manifest.id,
                    "name": k.manifest.name,
                    "slot": match k.manifest.slot { Slot::Bar => "bar", Slot::Quick => "quick" },
                    "order": k.manifest.order,
                    "qml_url": k.qml_url,
                    "rev": k.rev,
                    "visible": is_visible(k, owners),
                })
            })
            .collect(),
    )
}

#[derive(Default)]
struct Inner {
    widgets: BTreeMap<String, Known>,
    /// Owner (unique name) of every tracked name; `None` while it has none.
    owners: HashMap<(Bus, String), Option<String>>,
    /// The last `Widgets` data sent.
    sent: Option<Value>,
    /// Invalid folders already logged, with their reason (§5: once).
    logged: HashSet<(PathBuf, String)>,
    /// D-Bus calls in flight, per widget.
    in_flight: HashMap<String, usize>,
    /// `WidgetProps` / `WidgetSignal` subscriptions, per widget.
    subs: HashMap<String, (SubKey, AbortHandle)>,
    /// Owner watches started, per bus.
    watching: HashSet<Bus>,
    /// Whether any of the widget's slots is on screen (`WidgetShown`), per widget; a widget the
    /// shell hasn't mentioned counts as shown. While it isn't, its property subscription is
    /// dropped: no `WidgetProps` (signals still go through).
    shown: HashMap<String, watch::Sender<bool>>,
}

impl Inner {
    fn shown_rx(&mut self, id: &str) -> watch::Receiver<bool> {
        self.shown
            .entry(id.to_string())
            .or_insert_with(|| watch::channel(true).0)
            .subscribe()
    }
}

pub(crate) struct Widgets {
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
    root: PathBuf,
    bin_dir: PathBuf,
    session: BusConn,
    system: BusConn,
    runner: Runner,
    busy: BusySet,
    inner: Mutex<Inner>,
}

impl Widgets {
    pub(crate) fn new(
        tx: mpsc::Sender<Outbound>,
        stop: CancellationToken,
        root: PathBuf,
        bin_dir: PathBuf,
        session: Option<&str>,
        system: Option<&str>,
    ) -> Arc<Widgets> {
        Arc::new(Widgets {
            tx,
            stop,
            root,
            bin_dir,
            session: BusConn::new(session, Bus::Session),
            system: BusConn::new(system, Bus::System),
            runner: Runner::system(),
            busy: BusySet::new(),
            inner: Mutex::new(Inner::default()),
        })
    }

    fn conn(&self, bus: Bus) -> &BusConn {
        match bus {
            Bus::Session => &self.session,
            Bus::System => &self.system,
        }
    }

    /// Scan, send the first `Widgets` event, then follow the directory until the bridge stops.
    pub(crate) async fn run(self: Arc<Self>) {
        self.rescan().await;
        let mut changes = match watch_dirs(vec![self.root.clone()], DEBOUNCE) {
            Ok(rx) => Some(rx),
            Err(e) => {
                eprintln!(
                    "uqctl bridge: can't watch {}: {e}; new widgets need a restart",
                    self.root.display()
                );
                None
            }
        };
        loop {
            let next = async {
                match changes.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = self.stop.cancelled() => break,
                got = next => match got {
                    Some(()) => self.rescan().await,
                    None => changes = None,
                },
            }
        }
        let mut inner = lock(&self.inner);
        for (_, (_, h)) in inner.subs.drain() {
            h.abort();
        }
    }

    /// Read the directory again, learn the owners of any new names, and tell the shell.
    pub(crate) async fn rescan(self: &Arc<Self>) {
        let found = match scan_widgets(&self.root) {
            Ok(f) => f,
            Err(e) => {
                self.log_once(&self.root, &e);
                Vec::new()
            }
        };
        let mut widgets = BTreeMap::new();
        for (path, r) in found {
            match r {
                Ok(l) => {
                    let bins_ok = l.manifest.requires.missing_bins(&self.bin_dir).is_empty();
                    let qml_url = file_url(&l.qml_path);
                    let rev = content_rev(&l.dir);
                    widgets.insert(
                        l.manifest.id.clone(),
                        Known {
                            manifest: l.manifest,
                            qml_url,
                            rev,
                            bins_ok,
                        },
                    );
                }
                Err(e) => self.log_once(&path, &e.to_string()),
            }
        }
        let names: HashSet<(Bus, String)> = widgets.values().flat_map(Known::names).collect();
        let new_names: Vec<(Bus, String)> = {
            let mut inner = lock(&self.inner);
            inner.widgets = widgets;
            inner.owners.retain(|k, _| names.contains(k));
            names
                .iter()
                .filter(|k| !inner.owners.contains_key(*k))
                .cloned()
                .collect()
        };
        for bus in [Bus::Session, Bus::System] {
            let mine: Vec<String> = new_names
                .iter()
                .filter(|(b, _)| *b == bus)
                .map(|(_, n)| n.clone())
                .collect();
            if mine.is_empty() {
                continue;
            }
            self.ensure_watch(bus);
            for n in &mine {
                lock(&self.inner).owners.insert((bus, n.clone()), None);
            }
            if let Some(conn) = self.conn(bus).get().await
                && let Ok(proxy) = DBusProxy::new(&conn).await
            {
                for n in mine {
                    let owner = owner_of(&proxy, &n).await;
                    lock(&self.inner).owners.insert((bus, n), owner);
                }
            }
        }
        self.changed().await;
    }

    fn log_once(&self, path: &Path, reason: &str) {
        if lock(&self.inner)
            .logged
            .insert((path.to_path_buf(), reason.to_string()))
        {
            eprintln!("uqctl bridge: widget {} skipped: {reason}", path.display());
        }
    }

    /// Send `Widgets` if it changed, and bring the signal subscriptions in line.
    async fn changed(self: &Arc<Self>) {
        let data = {
            let mut inner = lock(&self.inner);
            let data = list_json(&inner.widgets, &inner.owners);
            if inner.sent.as_ref() == Some(&data) {
                None
            } else {
                inner.sent = Some(data.clone());
                Some(data)
            }
        };
        self.reconcile_subs().await;
        if let Some(data) = data {
            let ev = Outbound::Event {
                event: "Widgets".into(),
                data,
            };
            let _ = self.tx.send(ev).await;
        }
    }

    /// A visible widget's manifest, or why it can't be used.
    fn visible(&self, id: &str) -> Result<WidgetManifest, String> {
        let inner = lock(&self.inner);
        match inner.widgets.get(id) {
            Some(k) if is_visible(k, &inner.owners) => Ok(k.manifest.clone()),
            Some(_) => Err(format!("widget {id} is hidden")),
            None => Err(format!("no widget {id}")),
        }
    }
}

/// Whether `name` has a D-Bus service file (`ListActivatableNames`).
pub(crate) async fn activatable(conn: &zbus::Connection, name: &str) -> bool {
    let Ok(proxy) = DBusProxy::new(conn).await else {
        return false;
    };
    proxy
        .list_activatable_names()
        .await
        .is_ok_and(|names| names.iter().any(|n| n.as_str() == name))
}

#[cfg(test)]
mod tests;
