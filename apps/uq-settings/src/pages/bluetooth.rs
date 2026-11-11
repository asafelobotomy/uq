//! The Bluetooth page (spec §2.6, §3): turn Bluetooth on or off, let other devices find this
//! computer, look for devices, pair, trust, connect, disconnect and forget them, and show a
//! device's battery (`org.bluez.Battery1`). Everything goes through BlueZ on the system bus.
//!
//! **Pairing.** While the page is open it is BlueZ's pairing agent (`KeyboardDisplay`), and only
//! then: a code to compare or type is shown in plain text and needs an explicit Confirm. Nothing
//! is accepted without the person. Closing the page drops the agent (BlueZ forgets an agent whose
//! bus connection closes) and stops any search it started.
//!
//! Pairing and connecting wait for the device, so they run in the background: the page stays
//! usable (the person may need to answer the code question meanwhile) and shows "Pairing…" until
//! they finish. Device names come from the devices themselves and are shown through UqLabel
//! (plain text) only.

mod agent;

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::prompt::{Answer, Prompts};
use super::runner::PageLogic;
use super::sysbus::{self, Service, SystemBus, failure};

pub const BLUETOOTH: Service = Service {
    name: "Bluetooth",
    package: "bluez",
};

pub const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";
/// Where the page serves its pairing agent.
pub const AGENT_PATH: &str = "/org/uq/settings/bluetooth_agent";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetPowered(bool),
    SetDiscoverable(bool),
    /// Start or stop looking for devices.
    Scan(bool),
    Pair(String),
    CancelPairing(String),
    Connect(String),
    Disconnect(String),
    SetTrusted(String, bool),
    Forget(String),
    /// The person's answer to the question on show; `value` is a typed PIN or passkey.
    Answer {
        id: u64,
        yes: bool,
        value: String,
    },
}

/// What the page, its agent and its background calls share.
#[derive(Default)]
pub struct Shared {
    pub prompts: Prompts,
    /// A code BlueZ asked us to show (the other device types it): `{device, code}`.
    pub display: StdMutex<Option<Value>>,
    /// Devices being paired or connected: path → "pairing" / "connecting".
    pub working: StdMutex<HashMap<String, &'static str>>,
    notice: StdMutex<(u64, String)>,
    changed: StdMutex<Option<Arc<Notify>>>,
}

impl Shared {
    pub fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    pub fn ping(&self) {
        if let Some(n) = &*self.changed.lock().unwrap() {
            n.notify_one();
        }
    }
}

pub struct BluetoothState {
    system: SystemBus,
    write: Mutex<()>,
    shared: Arc<Shared>,
    tasks: StdMutex<Vec<JoinHandle<()>>>,
    registered: std::sync::atomic::AtomicBool,
}

impl Drop for BluetoothState {
    fn drop(&mut self) {
        self.shared.prompts.close();
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
    }
}

/// BlueZ's error as words people understand; `None` for one that means "already done".
pub fn error_words(e: &zbus::Error) -> Option<String> {
    let name = sysbus::error_name(e).unwrap_or_default();
    let text = sysbus::error_text(e);
    Some(match name.as_str() {
        "org.bluez.Error.AlreadyExists" | "org.bluez.Error.AlreadyConnected" => return None,
        "org.bluez.Error.AuthenticationFailed" | "org.bluez.Error.AuthenticationRejected" => {
            "Pairing didn't work: the codes didn't match or the device said no.".into()
        }
        "org.bluez.Error.AuthenticationCanceled" | "org.bluez.Error.Canceled" => {
            "Pairing was cancelled.".into()
        }
        "org.bluez.Error.AuthenticationTimeout" | "org.bluez.Error.ConnectionAttemptFailed" => {
            "The device didn't answer. Make sure it's on, nearby and ready to pair.".into()
        }
        "org.bluez.Error.NotReady" => "Bluetooth is off.".into(),
        "org.bluez.Error.InProgress" => "That's already happening.".into(),
        "org.bluez.Error.DoesNotExist" => "That device is gone.".into(),
        _ if text.contains("rfkill") => {
            "Bluetooth is blocked by a switch or key on the computer.".into()
        }
        _ => text,
    })
}

/// What kind of device it is, from BlueZ's icon name.
fn kind_text(icon: &str) -> &'static str {
    match icon {
        "audio-headset" | "audio-headphones" => "Headphones",
        "audio-card" | "audio-speakers" => "Speaker",
        "input-keyboard" => "Keyboard",
        "input-mouse" => "Mouse",
        "input-tablet" => "Drawing tablet",
        "input-gaming" => "Game controller",
        "phone" => "Phone",
        "computer" => "Computer",
        "camera-photo" | "camera-video" => "Camera",
        "printer" => "Printer",
        _ => "Device",
    }
}

type Objects =
    HashMap<OwnedObjectPath, HashMap<zbus::names::OwnedInterfaceName, HashMap<String, OwnedValue>>>;

/// The first adapter (by path) in one `GetManagedObjects` answer, with its properties.
fn first_adapter(objects: &Objects) -> Option<(&OwnedObjectPath, &HashMap<String, OwnedValue>)> {
    objects
        .iter()
        .filter_map(|(p, o)| props(o, ADAPTER).map(|a| (p, a)))
        .min_by(|a, b| a.0.as_str().cmp(b.0.as_str()))
}

fn props<'a>(
    o: &'a HashMap<zbus::names::OwnedInterfaceName, HashMap<String, OwnedValue>>,
    iface: &str,
) -> Option<&'a HashMap<String, OwnedValue>> {
    o.iter().find(|(k, _)| k.as_str() == iface).map(|(_, v)| v)
}

fn get<T: TryFrom<OwnedValue>>(p: &HashMap<String, OwnedValue>, key: &str) -> Option<T> {
    T::try_from(p.get(key)?.try_clone().ok()?).ok()
}

impl BluetoothState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system())
    }

    pub fn with(system: SystemBus) -> Self {
        Self {
            system,
            write: Mutex::new(()),
            shared: Arc::new(Shared::default()),
            tasks: StdMutex::new(Vec::new()),
            registered: std::sync::atomic::AtomicBool::new(false),
        }
    }

    async fn objects(&self, conn: &zbus::Connection) -> zbus::Result<Objects> {
        let om = zbus::fdo::ObjectManagerProxy::builder(conn)
            .destination(BLUEZ)?
            .path("/")?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        Ok(om.get_managed_objects().await?)
    }

    /// The first adapter's path.
    async fn adapter(&self, conn: &zbus::Connection) -> zbus::Result<OwnedObjectPath> {
        let objects = self.objects(conn).await?;
        first_adapter(&objects)
            .map(|(p, _)| p.clone())
            .ok_or_else(|| failure("This computer has no Bluetooth adapter."))
    }

    async fn proxy(
        &self,
        conn: &zbus::Connection,
        path: &str,
        iface: &'static str,
    ) -> zbus::Result<zbus::Proxy<'static>> {
        zbus::proxy::Builder::<zbus::Proxy<'static>>::new(conn)
            .destination(BLUEZ)?
            .path(path.to_string())?
            .interface(iface)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    }

    async fn set(&self, path: &str, iface: &'static str, prop: &str, on: bool) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let p = self.proxy(&conn, path, iface).await?;
        p.set_property(prop, on)
            .await
            .map_err(|e| zbus::Error::FDO(Box::new(e)))
    }

    async fn read(&self) -> zbus::Result<Value> {
        let conn = self.system.conn().await?;
        let objects = self.objects(&conn).await?;
        // From the one snapshot: an adapter that went between two reads can't be missing here.
        let Some((adapter, a)) = first_adapter(&objects) else {
            return Ok(json!({"available": true, "adapter": null, "devices": []}));
        };
        let adapter = adapter.clone();
        let working = self.shared.working.lock().unwrap().clone();
        let mut devices = Vec::new();
        for (path, o) in &objects {
            let Some(d) = props(o, DEVICE) else { continue };
            if get::<OwnedObjectPath>(d, "Adapter").as_ref() != Some(&adapter) {
                continue;
            }
            let paired = get::<bool>(d, "Paired").unwrap_or(false);
            let name = get::<String>(d, "Name");
            // A device that doesn't say its name is noise unless it is already paired.
            if name.is_none() && !paired {
                continue;
            }
            let alias = get::<String>(d, "Alias")
                .or(name)
                .unwrap_or_else(|| "Unknown device".into());
            let icon = get::<String>(d, "Icon").unwrap_or_default();
            devices.push(json!({
                "path": path.as_str(),
                "name": alias,
                "icon": if icon.is_empty() { "bluetooth".to_string() } else { icon.clone() },
                "kind": kind_text(&icon),
                "paired": paired,
                "trusted": get::<bool>(d, "Trusted").unwrap_or(false),
                "connected": get::<bool>(d, "Connected").unwrap_or(false),
                "battery": props(o, BATTERY).and_then(|b| get::<u8>(b, "Percentage")),
                "working": working.get(path.as_str()).copied(),
            }));
        }
        devices.sort_by_key(|d| {
            (
                d["connected"] != true,
                d["paired"] != true,
                d["name"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        Ok(json!({
            "available": true,
            "adapter": {
                "path": adapter.as_str(),
                "name": get::<String>(a, "Alias").unwrap_or_default(),
                "powered": get::<bool>(a, "Powered").unwrap_or(false),
                "discoverable": get::<bool>(a, "Discoverable").unwrap_or(false),
                "discovering": get::<bool>(a, "Discovering").unwrap_or(false),
            },
            "devices": devices,
        }))
    }

    /// Run a device call that waits on the device in the background, marking it `what`.
    async fn background(
        &self,
        path: String,
        what: &'static str,
        method: &'static str,
    ) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let shared = self.shared.clone();
        shared.working.lock().unwrap().insert(path.clone(), what);
        let task = async move {
            let r = async {
                zbus::proxy::Builder::<zbus::Proxy<'static>>::new(&conn)
                    .destination(BLUEZ)?
                    .path(path.clone())?
                    .interface(DEVICE)?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?
                    .call_method(method, &())
                    .await
                    .map(|_| ())
            }
            .await;
            if let Err(e) = r
                && let Some(text) = error_words(&e)
            {
                shared.notice(&text);
            }
            shared.working.lock().unwrap().remove(&path);
            *shared.display.lock().unwrap() = None;
            shared.ping();
        };
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|t| !t.is_finished());
        tasks.push(tokio::spawn(task));
        Ok(())
    }

    /// Register the agent with BlueZ, as the default one.
    async fn register(&self, conn: &zbus::Connection) -> bool {
        let r = async {
            let m = self
                .proxy(conn, "/org/bluez", "org.bluez.AgentManager1")
                .await?;
            let path = OwnedObjectPath::try_from(AGENT_PATH).expect("a valid path");
            m.call_method("RegisterAgent", &(&path, "KeyboardDisplay"))
                .await?;
            m.call_method("RequestDefaultAgent", &(&path,)).await?;
            zbus::Result::Ok(())
        };
        let ok = sysbus::within(r).await.is_ok();
        self.registered
            .store(ok, std::sync::atomic::Ordering::SeqCst);
        ok
    }

    async fn follow(&self, changed: &Notify) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        conn.object_server()
            .at(
                AGENT_PATH,
                agent::PairingAgent {
                    shared: self.shared.clone(),
                },
            )
            .await?;
        let devices = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path_namespace("/org/bluez")?
            .build();
        let added = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path("/")?
            .interface("org.freedesktop.DBus.ObjectManager")?
            .build();
        let mut signals = futures_util::stream::select(
            zbus::MessageStream::for_match_rule(devices, &conn, Some(256)).await?,
            zbus::MessageStream::for_match_rule(added, &conn, Some(64)).await?,
        );
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let mut owners = dbus
            .receive_name_owner_changed_with_args(&[(0, BLUEZ)])
            .await?;
        self.register(&conn).await;
        changed.notify_one();
        loop {
            tokio::select! {
                Some(_) = signals.next() => changed.notify_one(),
                Some(owner) = owners.next() => {
                    // BlueZ restarted: it forgot its agents.
                    if owner.args().is_ok_and(|a| a.new_owner().is_some()) {
                        self.register(&conn).await;
                    } else {
                        self.registered.store(false, std::sync::atomic::Ordering::SeqCst);
                    }
                    changed.notify_one();
                }
                else => return Ok(()),
            }
        }
    }
}

/// A refusal BlueZ sends is shown in words; "already done" is not an error.
fn settle(r: zbus::Result<()>) -> zbus::Result<()> {
    match r {
        Ok(()) => Ok(()),
        Err(e) => match sysbus::classify(&e) {
            sysbus::Failure::Missing => Err(failure(BLUETOOTH.missing_text())),
            sysbus::Failure::Quiet => Ok(()),
            sysbus::Failure::Shown(_) => match error_words(&e) {
                Some(text) => Err(failure(text)),
                None => Ok(()),
            },
        },
    }
}

impl PageLogic for BluetoothState {
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
        let device =
            |p: &str| OwnedObjectPath::try_from(p).map_err(|_| failure("That device is gone."));
        match op {
            Op::Refresh => Ok(()),
            Op::Answer { id, yes, value } => {
                let a = if yes {
                    Answer::Yes(HashMap::from([("value".to_string(), value)]))
                } else {
                    Answer::No
                };
                self.shared.prompts.answer(id, a);
                Ok(())
            }
            Op::SetPowered(on) => {
                let conn = self.system.conn().await?;
                let adapter = match self.adapter(&conn).await {
                    Ok(a) => a,
                    Err(e) => return settle(Err(e)),
                };
                settle(self.set(adapter.as_str(), ADAPTER, "Powered", on).await)
            }
            Op::SetDiscoverable(on) => {
                let conn = self.system.conn().await?;
                let adapter = self.adapter(&conn).await.map_err(|e| {
                    if matches!(sysbus::classify(&e), sysbus::Failure::Missing) {
                        failure(BLUETOOTH.missing_text())
                    } else {
                        e
                    }
                })?;
                settle(
                    self.set(adapter.as_str(), ADAPTER, "Discoverable", on)
                        .await,
                )
            }
            Op::Scan(on) => {
                let conn = self.system.conn().await?;
                let adapter = self.adapter(&conn).await?;
                let a = self.proxy(&conn, adapter.as_str(), ADAPTER).await?;
                let method = if on {
                    "StartDiscovery"
                } else {
                    "StopDiscovery"
                };
                settle(a.call_method(method, &()).await.map(|_| ()))
            }
            Op::Pair(path) => {
                device(&path)?;
                self.background(path, "pairing", "Pair").await
            }
            Op::CancelPairing(path) => {
                let conn = self.system.conn().await?;
                let d = self.proxy(&conn, device(&path)?.as_str(), DEVICE).await?;
                // Nothing to cancel any more is fine.
                let _ = d.call_method("CancelPairing", &()).await;
                self.shared.prompts.cancel(None);
                Ok(())
            }
            Op::Connect(path) => {
                device(&path)?;
                self.background(path, "connecting", "Connect").await
            }
            Op::Disconnect(path) => {
                let conn = self.system.conn().await?;
                let d = self.proxy(&conn, device(&path)?.as_str(), DEVICE).await?;
                settle(d.call_method("Disconnect", &()).await.map(|_| ()))
            }
            Op::SetTrusted(path, on) => settle(
                self.set(device(&path)?.as_str(), DEVICE, "Trusted", on)
                    .await,
            ),
            Op::Forget(path) => {
                let conn = self.system.conn().await?;
                let adapter = self.adapter(&conn).await?;
                let a = self.proxy(&conn, adapter.as_str(), ADAPTER).await?;
                settle(
                    a.call_method("RemoveDevice", &(device(&path)?,))
                        .await
                        .map(|_| ()),
                )
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let mut state = match sysbus::within(self.read()).await {
            Ok(v) => v,
            Err(e) => sysbus::read_error(&e, &BLUETOOTH),
        };
        let (seq, text) = self.shared.notice.lock().unwrap().clone();
        state["notice"] = json!({"seq": seq, "text": text});
        state["question"] = self.shared.prompts.current();
        state["display"] = self
            .shared
            .display
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Value::Null);
        state["agent"] = json!(self.registered.load(std::sync::atomic::Ordering::SeqCst));
        Ok(state)
    }

    async fn watch(&self, changed: Arc<Notify>) {
        self.shared.prompts.follow(changed.clone());
        *self.shared.changed.lock().unwrap() = Some(changed.clone());
        if let Err(e) = self.follow(&changed).await {
            eprintln!(
                "uq-settings: can't follow Bluetooth changes: {}",
                sysbus::error_text(&e)
            );
        }
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests;
