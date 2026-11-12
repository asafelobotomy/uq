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
use tokio::sync::mpsc;
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
use zbus::zvariant::{OwnedValue, Structure};
use zbus::{MatchRule, MessageStream};

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
                (Some(a), _) => {
                    zbus::connection::Builder::address(a.as_str())?
                        .build()
                        .await
                }
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

/// What a running signal subscription was made for; a different key restarts it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SubKey {
    spec: DbusSpec,
    owner: String,
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

/// Held while a widget's D-Bus call runs; frees its slot on drop.
struct CallSlot<'a> {
    w: &'a Widgets,
    id: String,
}

impl Drop for CallSlot<'_> {
    fn drop(&mut self) {
        let mut inner = lock(&self.w.inner);
        if let Some(n) = inner.in_flight.get_mut(&self.id) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                inner.in_flight.remove(&self.id);
            }
        }
    }
}

fn dbus_err(e: DbusJsonError) -> String {
    e.to_string()
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

    /// One subscription per visible widget whose `[dbus]` lists properties or signals and whose
    /// name has an owner.
    async fn reconcile_subs(self: &Arc<Self>) {
        let want: HashMap<String, SubKey> = {
            let inner = lock(&self.inner);
            inner
                .widgets
                .values()
                .filter(|k| is_visible(k, &inner.owners))
                .filter_map(|k| {
                    let d = k.manifest.dbus.as_ref()?;
                    if d.properties.is_empty() && d.signals.is_empty() {
                        return None;
                    }
                    let owner = inner.owners.get(&(d.bus, d.name.clone()))?.clone()?;
                    Some((
                        k.manifest.id.clone(),
                        SubKey {
                            spec: d.clone(),
                            owner,
                        },
                    ))
                })
                .collect()
        };
        let start: Vec<(String, SubKey)> = {
            let mut inner = lock(&self.inner);
            inner.subs.retain(|id, (key, h)| {
                let keep = want.get(id) == Some(key);
                if !keep {
                    h.abort();
                }
                keep
            });
            want.into_iter()
                .filter(|(id, _)| !inner.subs.contains_key(id))
                .collect()
        };
        for (id, key) in start {
            let Some(conn) = self.conn(key.spec.bus).get().await else {
                continue;
            };
            let (tx, stop, wid, k) = (self.tx.clone(), self.stop.clone(), id.clone(), key.clone());
            let task = tokio::spawn(async move {
                tokio::select! {
                    _ = stop.cancelled() => {}
                    _ = signal_loop(tx, conn, wid, k) => {}
                }
            });
            let mut inner = lock(&self.inner);
            // A newer reconcile may have started one meanwhile; keep only one.
            if let Some((_, old)) = inner.subs.insert(id, (key, task.abort_handle())) {
                old.abort();
            }
        }
    }

    /// Start following `NameOwnerChanged` on `bus`, once.
    fn ensure_watch(self: &Arc<Self>, bus: Bus) {
        if !lock(&self.inner).watching.insert(bus) {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let stop = me.stop.clone();
            tokio::select! {
                _ = stop.cancelled() => {}
                _ = me.owner_loop(bus) => {}
            }
        });
    }

    async fn owner_loop(self: Arc<Self>, bus: Bus) {
        loop {
            if let Some(conn) = self.conn(bus).get().await
                && let Ok(proxy) = DBusProxy::new(&conn).await
                && let Ok(mut changes) = proxy.receive_name_owner_changed().await
            {
                // Subscribed first, so a change in between is not lost.
                let names: Vec<String> = lock(&self.inner)
                    .owners
                    .keys()
                    .filter(|(b, _)| *b == bus)
                    .map(|(_, n)| n.clone())
                    .collect();
                for n in names {
                    let owner = owner_of(&proxy, &n).await;
                    if let Some(slot) = lock(&self.inner).owners.get_mut(&(bus, n)) {
                        *slot = owner;
                    }
                }
                self.changed().await;
                while let Some(sig) = changes.next().await {
                    let Ok(args) = sig.args() else { continue };
                    let key = (bus, args.name().to_string());
                    let owner = args.new_owner().as_ref().map(|o| o.to_string());
                    let hit = {
                        let mut inner = lock(&self.inner);
                        match inner.owners.get_mut(&key) {
                            Some(slot) if *slot != owner => {
                                *slot = owner;
                                true
                            }
                            _ => false,
                        }
                    };
                    if hit {
                        self.changed().await;
                    }
                }
            }
            self.conn(bus).reset().await;
            tokio::time::sleep(RETRY).await;
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

    /// Take one of the widget's 8 call slots, or `busy`.
    fn take_slot(&self, id: &str) -> Result<CallSlot<'_>, String> {
        let mut inner = lock(&self.inner);
        let n = inner.in_flight.entry(id.to_string()).or_insert(0);
        if *n >= MAX_IN_FLIGHT {
            return Err("busy".into());
        }
        *n += 1;
        Ok(CallSlot {
            w: self,
            id: id.to_string(),
        })
    }

    /// The `[dbus]` target, the connection, and whether the call may start the service (§2.2:
    /// only without `requires.bus_name`, and only if the name is activatable).
    async fn target(
        &self,
        m: &WidgetManifest,
    ) -> Result<(DbusSpec, zbus::Connection, bool), String> {
        let spec = m
            .dbus
            .clone()
            .ok_or_else(|| format!("widget {} has no [dbus] table", m.id))?;
        let conn = self
            .conn(spec.bus)
            .get()
            .await
            .ok_or("the bus is not reachable")?;
        let auto_start = m.requires.bus_name.is_none() && activatable(&conn, &spec.name).await;
        Ok((spec, conn, auto_start))
    }

    /// `WidgetCall [id, method, args]`.
    pub(crate) async fn call(&self, args: &[Value]) -> Result<Value, String> {
        let (id, method, call_args) = match args {
            [Value::String(id), Value::String(m)] => (id, m, json!([])),
            [Value::String(id), Value::String(m), a @ Value::Array(_)] => (id, m, a.clone()),
            _ => return Err("WidgetCall takes a widget id, a method and an argument list".into()),
        };
        let m = self.visible(id)?;
        let listed = m
            .dbus
            .as_ref()
            .is_some_and(|d| d.methods.iter().any(|x| x == method));
        if !listed {
            return Err(format!("{method} is not one of widget {id}'s methods"));
        }
        let _slot = self.take_slot(id)?;
        bounded(async {
            let (spec, conn, auto) = self.target(&m).await?;
            dbus_json::call(&conn, &target(&spec), method, &call_args, auto)
                .await
                .map_err(dbus_err)
        })
        .await
    }

    /// `WidgetGet [id]`: every listed property as one object.
    pub(crate) async fn get(&self, args: &[Value]) -> Result<Value, String> {
        let [Value::String(id)] = args else {
            return Err("WidgetGet takes a widget id".into());
        };
        let m = self.visible(id)?;
        let _slot = self.take_slot(id)?;
        bounded(async {
            let (spec, conn, auto) = self.target(&m).await?;
            if spec.properties.is_empty() {
                return Ok(json!({}));
            }
            dbus_json::get_properties(&conn, &target(&spec), &spec.properties, auto)
                .await
                .map_err(dbus_err)
        })
        .await
    }

    /// `WidgetRun [id, command, args]`: `{code, output}` (or `{code, error, output}` for a
    /// non-zero exit).
    pub(crate) async fn run_command(&self, args: &[Value]) -> Result<Value, String> {
        let (id, cmd, extra) = match args {
            [Value::String(id), Value::String(c)] => (id, c, Vec::new()),
            [Value::String(id), Value::String(c), Value::Array(a)] => {
                let a: Option<Vec<String>> =
                    a.iter().map(|v| v.as_str().map(str::to_owned)).collect();
                (id, c, a.ok_or("arguments must be strings")?)
            }
            _ => {
                return Err(
                    "WidgetRun takes a widget id, a command id and a list of strings".into(),
                );
            }
        };
        let m = self.visible(id)?;
        let spec = command::find(&m.commands, cmd).map_err(|e| e.to_string())?;
        if spec.confirm.is_some() {
            // The bar has no confirmation dialog; such a command belongs on a settings page.
            return Err(format!(
                "{cmd} asks for confirmation, which widgets can't show"
            ));
        }
        let _token = self.busy.try_acquire(id, cmd).map_err(|_| "busy")?;
        match self.runner.run(spec, &extra).await {
            Ok(r) => {
                if r.code != 0 && !r.stderr_head.is_empty() {
                    eprintln!(
                        "uqctl bridge: widget {id} command {cmd} exited {}: {}",
                        r.code, r.stderr_head
                    );
                }
                Ok(r.to_json())
            }
            Err(CommandError::Busy) => Err("busy".into()),
            Err(e) => {
                eprintln!("uqctl bridge: widget {id} command {cmd}: {e}");
                Err(e.to_string())
            }
        }
    }
}

/// Run `f` within the 2 s call deadline (connecting and the activatable check included).
async fn bounded(
    f: impl std::future::Future<Output = Result<Value, String>>,
) -> Result<Value, String> {
    tokio::time::timeout(CALL_DEADLINE, f)
        .await
        .unwrap_or_else(|_| Err(DbusJsonError::Deadline.to_string()))
}

fn target(spec: &DbusSpec) -> Target {
    Target {
        name: spec.name.clone(),
        path: spec.path.clone(),
        interface: spec.interface.clone(),
    }
}

async fn owner_of(proxy: &DBusProxy<'_>, name: &str) -> Option<String> {
    let bus_name = zbus::names::BusName::try_from(name).ok()?;
    proxy
        .get_name_owner(bus_name)
        .await
        .ok()
        .map(|o| o.to_string())
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

/// A signal's arguments as a JSON array, through the same type allowlist as calls.
pub(crate) fn signal_args(msg: &zbus::Message) -> Result<Value, DbusJsonError> {
    let body = msg.body();
    if body.len() > MAX_RESULT_JSON {
        return Err(DbusJsonError::ResultTooLarge);
    }
    let sig = body.signature().to_string();
    if body.is_empty() || sig.is_empty() {
        return Ok(json!([]));
    }
    let inner = sig
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(&sig)
        .to_string();
    parse_signature(&inner)?;
    let fields: Structure<'_> = body
        .deserialize()
        .map_err(|e| DbusJsonError::Call(e.to_string()))?;
    let values: Vec<Value> = fields
        .fields()
        .iter()
        .map(value_to_json)
        .collect::<Result<_, _>>()?;
    Ok(Value::Array(values))
}

fn rule(spec: &DbusSpec, props: bool) -> zbus::Result<MatchRule<'static>> {
    let b = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(spec.name.clone())?
        .path(spec.path.clone())?;
    Ok(if props {
        b.interface(PROPERTIES)?
            .member("PropertiesChanged")?
            .arg(0, spec.interface.clone())?
            .build()
    } else {
        b.interface(spec.interface.clone())?.build()
    })
}

async fn next_msg(s: &mut Option<MessageStream>) -> Option<zbus::Result<zbus::Message>> {
    match s.as_mut() {
        Some(s) => s.next().await,
        None => std::future::pending().await,
    }
}

/// Forward a widget's listed signals as `WidgetSignal` and its listed properties as
/// `WidgetProps`, at most [`PROPS_EVERY`]. Only messages from the name's current owner count.
async fn signal_loop(
    tx: mpsc::Sender<Outbound>,
    conn: zbus::Connection,
    widget: String,
    key: SubKey,
) {
    let spec = &key.spec;
    let open = |props: bool| {
        let conn = conn.clone();
        async move {
            let r = rule(spec, props).ok()?;
            MessageStream::for_match_rule(r, &conn, Some(64)).await.ok()
        }
    };
    let mut props = if spec.properties.is_empty() {
        None
    } else {
        open(true).await
    };
    let mut sigs = if spec.signals.is_empty() {
        None
    } else {
        open(false).await
    };
    let mut pending = Map::new();
    let mut last: Option<Instant> = None;
    let mut warned = false;
    loop {
        let due = last.map(|t| t + PROPS_EVERY);
        let waiting = !pending.is_empty();
        let flush = async move {
            match due {
                Some(at) if waiting => tokio::time::sleep_until(at).await,
                _ if waiting => {}
                _ => std::future::pending().await,
            }
        };
        tokio::select! {
            m = next_msg(&mut props) => {
                let Some(Ok(m)) = m else { props = None; continue };
                if m.header().sender().map(|s| s.as_str()) != Some(key.owner.as_str()) {
                    continue;
                }
                let Ok((_, changed, invalidated)) = m
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                else {
                    continue;
                };
                for p in &spec.properties {
                    if let Some(v) = changed.get(p) {
                        match value_to_json(v) {
                            Ok(j) => {
                                pending.insert(p.clone(), j);
                            }
                            Err(e) if !warned => {
                                warned = true;
                                eprintln!("uqctl bridge: widget {widget} property {p}: {e}");
                            }
                            Err(_) => {}
                        }
                    } else if invalidated.iter().any(|i| i == p)
                        && let Ok(j) = dbus_json::get_property(&conn, &target(spec), p, false).await
                    {
                        pending.insert(p.clone(), j);
                    }
                }
            }
            m = next_msg(&mut sigs) => {
                let Some(Ok(m)) = m else { sigs = None; continue };
                let h = m.header();
                if h.sender().map(|s| s.as_str()) != Some(key.owner.as_str()) {
                    continue;
                }
                let Some(member) = h.member().map(|m| m.to_string()) else { continue };
                if !spec.signals.contains(&member) {
                    continue;
                }
                match signal_args(&m) {
                    Ok(args) => {
                        let ev = Outbound::Event {
                            event: "WidgetSignal".into(),
                            data: json!({"widget": widget, "member": member, "args": args}),
                        };
                        if tx.send(ev).await.is_err() {
                            return;
                        }
                    }
                    Err(e) if !warned => {
                        warned = true;
                        eprintln!("uqctl bridge: widget {widget} signal {member}: {e}");
                    }
                    Err(_) => {}
                }
            }
            () = flush => {
                last = Some(Instant::now());
                let props = Value::Object(std::mem::take(&mut pending));
                let ev = Outbound::Event {
                    event: "WidgetProps".into(),
                    data: json!({"widget": widget, "props": props}),
                };
                if tx.send(ev).await.is_err() {
                    return;
                }
            }
        }
        if props.is_none() && sigs.is_none() && pending.is_empty() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_are_encoded() {
        assert_eq!(
            file_url(Path::new("/usr/share/uq/shell/widgets/a-b/W idget.qml")),
            "file:///usr/share/uq/shell/widgets/a-b/W%20idget.qml"
        );
        assert_eq!(file_url(Path::new("/x/ü.qml")), "file:///x/%C3%BC.qml");
    }

    fn known(id: &str, order: u8, bus_name: Option<&str>, bins_ok: bool) -> Known {
        let toml = format!(
            "uq_api = 1\nid = \"{id}\"\nname = \"N {id}\"\nslot = \"bar\"\nqml = \"W.qml\"\n\
             order = {order}\n{}",
            bus_name.map_or(String::new(), |n| format!(
                "[requires]\nbus_name = \"{n}\"\n"
            ))
        );
        Known {
            manifest: uq_core::integration::manifest::parse_widget(&toml).unwrap(),
            qml_url: format!("file:///w/{id}/W.qml"),
            rev: format!("rev-{id}"),
            bins_ok,
        }
    }

    #[test]
    fn rev_follows_file_changes() {
        let dir = std::env::temp_dir().join(format!("uq-rev-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("W.qml"), "Item {}").unwrap();
        std::fs::write(dir.join("sub/x.js"), "1").unwrap();
        let first = content_rev(&dir);
        assert_eq!(content_rev(&dir), first, "same files, same rev");
        std::fs::write(dir.join("sub/x.js"), "22").unwrap();
        let second = content_rev(&dir);
        assert_ne!(second, first, "a changed file beside the QML changes it");
        std::fs::write(dir.join("Details.qml"), "Item {}").unwrap();
        assert_ne!(content_rev(&dir), second, "a new file changes it");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn list_orders_and_gates() {
        let mut w = BTreeMap::new();
        w.insert("b".into(), known("b", 10, None, true));
        w.insert("a".into(), known("a", 10, Some("org.x.A"), true));
        w.insert("c".into(), known("c", 5, None, false));
        let mut owners = HashMap::new();
        owners.insert((Bus::Session, "org.x.A".to_string()), None);
        let l = list_json(&w, &owners);
        let ids: Vec<&str> = l
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["c", "a", "b"], "order, then id");
        assert_eq!(l[0]["visible"], false, "a missing program hides it");
        assert_eq!(l[1]["visible"], false, "no owner yet");
        assert_eq!(l[2]["visible"], true);
        assert_eq!(
            l[2],
            json!({"id":"b","name":"N b","slot":"bar","order":10,
                   "qml_url":"file:///w/b/W.qml","rev":"rev-b","visible":true})
        );
        owners.insert((Bus::Session, "org.x.A".to_string()), Some(":1.5".into()));
        assert_eq!(list_json(&w, &owners)[1]["visible"], true);
    }
}
