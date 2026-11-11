//! The Network page (spec §2.6, §3): Wi-Fi (on/off, scan, join, forget), saved connections and
//! their IP, DNS and metered settings, wired connections, a hotspot, VPNs (WireGuard imported
//! from a config file and edited here; OpenVPN and other plugin VPNs listed, started and
//! stopped) and work or school (802.1x: PEAP, TTLS, TLS) Wi-Fi. Everything goes through
//! NetworkManager on the system bus; its own polkit actions guard the changes.
//!
//! **Secrets.** A password typed on the page goes into the settings sent to NetworkManager and
//! nowhere else; uQ never logs or keeps one. While the page is open it is also a NetworkManager
//! secret agent ([`agent`]), but only for connections started from the page: when one of those
//! needs a password (a wrong one, a saved network without one), the page asks for it. New
//! connections are private to the person who adds them unless they choose "available to all
//! users".
//!
//! Network names and device names come from the air and from the system: the page shows them
//! through UqLabel (plain text) only.

mod agent;
pub(crate) mod nm;
pub(crate) mod profile;
pub(crate) mod wireguard;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use zbus::zvariant::OwnedObjectPath;

use super::prompt::Answer;
use super::runner::PageLogic;
use super::sysbus::{self, Service, SystemBus, failure, settle};
use nm::{
    AgentManagerProxy, NetworkManagerProxy, NmConnectionProxy, NmSettingsProxy, NmWirelessProxy,
    Props, Settings,
};
use profile::{Eap, IpInput, Security, WifiInput};

pub const NETWORK: Service = Service {
    name: "Network settings",
    package: "networkmanager",
};

/// Where NetworkManager's VPN plugins announce themselves (under the page's root).
const VPN_PLUGINS: &str = "usr/lib/NetworkManager/VPN";
/// The biggest WireGuard file the page reads.
const MAX_IMPORT: u64 = 64 * 1024;

pub const SAVED_NOTE: &str = "Saved. It applies the next time this connection starts.";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetWifi(bool),
    Scan,
    /// Join the network an access point belongs to (a saved one is started again).
    ConnectWifi {
        ap: String,
        device: String,
        password: String,
        all_users: bool,
        eap: Option<Eap>,
    },
    /// Join a network that doesn't show its name.
    ConnectHidden {
        ssid: String,
        /// `open` or `password`.
        security: String,
        password: String,
        all_users: bool,
    },
    Activate {
        uuid: String,
    },
    Deactivate {
        uuid: String,
    },
    Forget {
        uuid: String,
    },
    SetAutoconnect {
        uuid: String,
        on: bool,
    },
    SetMetered {
        uuid: String,
        metered: String,
    },
    SetEveryone {
        uuid: String,
        on: bool,
    },
    SetIp {
        uuid: String,
        family: u8,
        ip: IpInput,
    },
    ImportWireGuard {
        path: PathBuf,
        all_users: bool,
    },
    EditWireGuard {
        uuid: String,
        peer: wireguard::PeerEdit,
    },
    Hotspot {
        on: bool,
        name: String,
        password: String,
    },
    /// The person's answer to the password question on show.
    AnswerSecret {
        id: u64,
        values: HashMap<String, String>,
    },
    CancelSecret {
        id: u64,
    },
}

pub struct NetworkState {
    system: SystemBus,
    root: PathBuf,
    user: String,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
    shared: Arc<agent::Shared>,
    registered: std::sync::atomic::AtomicBool,
}

impl Drop for NetworkState {
    fn drop(&mut self) {
        // A question still on show is dropped: NetworkManager hears "cancelled".
        self.shared.prompts.close();
    }
}

/// The name of the account this app runs as, from `/etc/passwd`, else `$USER`.
pub fn my_user_name() -> String {
    let uid = sysbus::my_uid().to_string();
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|p| {
            p.lines().find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 2 && f[2] == uid).then(|| f[0].to_string())
            })
        })
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default()
}

/// Why a device failed, in words (NMDeviceStateReason).
pub fn reason_text(reason: u32) -> String {
    match reason {
        7 => "a password is needed and none was given.".into(),
        8..=10 => "the network didn't accept the password or settings.".into(),
        11 => "the network didn't answer in time.".into(),
        5 | 6 => "the network didn't give this computer an address.".into(),
        53 => "the network is out of range.".into(),
        n => format!("something went wrong (code {n})."),
    }
}

/// A device's state in words.
fn state_text(state: u32, carrier: bool) -> &'static str {
    match state {
        nm::STATE_ACTIVATED => "Connected",
        40..=90 => "Connecting…",
        110 => "Disconnecting…",
        nm::STATE_FAILED => "Couldn't connect",
        nm::STATE_UNAVAILABLE if !carrier => "Cable unplugged",
        nm::STATE_UNAVAILABLE => "Not ready",
        10 => "Not managed here",
        _ => "Not connected",
    }
}

/// The band of a frequency in MHz.
fn band(mhz: u32) -> &'static str {
    match mhz {
        2400..=2500 => "2.4 GHz",
        4900..=5900 => "5 GHz",
        5925..=7125 => "6 GHz",
        _ => "",
    }
}

/// One access point as read.
struct Ap {
    path: OwnedObjectPath,
    device: OwnedObjectPath,
    ssid: Vec<u8>,
    strength: u8,
    security: Security,
    frequency: u32,
}

/// One active connection as read.
struct Active {
    path: OwnedObjectPath,
    uuid: String,
    state: u32,
    vpn: bool,
}

/// One saved connection as read.
struct Saved {
    path: OwnedObjectPath,
    settings: Settings,
}

impl NetworkState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system(), PathBuf::from("/"), my_user_name())
    }

    pub fn with(system: SystemBus, root: PathBuf, user: String) -> Self {
        Self {
            system,
            root,
            user,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
            shared: Arc::new(agent::Shared::default()),
            registered: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    /// Whether NetworkManager's OpenVPN plugin is installed.
    fn openvpn(&self) -> bool {
        self.root
            .join(VPN_PLUGINS)
            .join("nm-openvpn-service.name")
            .is_file()
    }

    async fn conn(&self) -> zbus::Result<zbus::Connection> {
        self.system.conn().await
    }

    async fn active(&self, conn: &zbus::Connection) -> zbus::Result<Vec<Active>> {
        let props = nm::get_all(conn, nm::NM_PATH, nm::IFACE_NM).await?;
        let mut out = Vec::new();
        for path in nm::paths(&props, "ActiveConnections") {
            // One may go away between the two reads.
            let Ok(a) = nm::get_all(conn, path.as_str(), nm::IFACE_ACTIVE).await else {
                continue;
            };
            out.push(Active {
                path,
                uuid: nm::text(&a, "Uuid"),
                state: nm::num(&a, "State"),
                vpn: nm::flag(&a, "Vpn"),
            });
        }
        Ok(out)
    }

    async fn saved(&self, conn: &zbus::Connection) -> zbus::Result<Vec<Saved>> {
        let settings = NmSettingsProxy::new(conn).await?;
        let mut out = Vec::new();
        for path in settings.list_connections().await? {
            let c: NmConnectionProxy = nm::at(conn, &path).await?;
            let Ok(s) = c.get_settings().await else {
                continue;
            };
            out.push(Saved { path, settings: s });
        }
        Ok(out)
    }

    async fn find(&self, conn: &zbus::Connection, uuid: &str) -> zbus::Result<Saved> {
        self.saved(conn)
            .await?
            .into_iter()
            .find(|s| profile::uuid(&s.settings) == uuid)
            .ok_or_else(|| failure("That connection is gone. It may have been removed elsewhere."))
    }

    /// Read a saved connection, let `edit` change it, and save it back.
    async fn edit(
        &self,
        uuid: &str,
        edit: impl FnOnce(&mut Settings) -> Result<(), String>,
    ) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let mut saved = self.find(&conn, uuid).await?;
        edit(&mut saved.settings).map_err(failure)?;
        let c: NmConnectionProxy = nm::at(&conn, &saved.path).await?;
        let r = c.update(&saved.settings).await;
        let ok = r.is_ok();
        settle(r, &NETWORK)?;
        if ok
            && self
                .active(&conn)
                .await
                .unwrap_or_default()
                .iter()
                .any(|a| a.uuid == uuid)
        {
            self.notice(SAVED_NOTE);
        }
        Ok(())
    }

    async fn devices(
        &self,
        conn: &zbus::Connection,
    ) -> zbus::Result<Vec<(OwnedObjectPath, Props)>> {
        let nm = NetworkManagerProxy::new(conn).await?;
        let mut out = Vec::new();
        for path in nm.get_devices().await? {
            if let Ok(p) = nm::get_all(conn, path.as_str(), nm::IFACE_DEVICE).await {
                out.push((path, p));
            }
        }
        Ok(out)
    }

    async fn access_points(
        &self,
        conn: &zbus::Connection,
        device: &OwnedObjectPath,
    ) -> zbus::Result<(Props, Vec<Ap>)> {
        let w = nm::get_all(conn, device.as_str(), nm::IFACE_WIRELESS).await?;
        let mut aps = Vec::new();
        for path in nm::paths(&w, "AccessPoints") {
            let Ok(a) = nm::get_all(conn, path.as_str(), nm::IFACE_AP).await else {
                continue;
            };
            aps.push(Ap {
                path,
                device: device.clone(),
                ssid: nm::get::<Vec<u8>>(&a, "Ssid").unwrap_or_default(),
                strength: nm::get::<u8>(&a, "Strength").unwrap_or(0),
                security: profile::security(
                    nm::num(&a, "Flags"),
                    nm::num(&a, "WpaFlags"),
                    nm::num(&a, "RsnFlags"),
                ),
                frequency: nm::num(&a, "Frequency"),
            });
        }
        Ok((w, aps))
    }

    async fn read(&self) -> zbus::Result<Value> {
        let conn = self.conn().await?;
        let props = nm::get_all(&conn, nm::NM_PATH, nm::IFACE_NM).await?;
        let active = self.active(&conn).await?;
        let saved = self.saved(&conn).await?;
        let state_of = |uuid: &str| {
            active
                .iter()
                .find(|a| a.uuid == uuid)
                .map_or("off", |a| match a.state {
                    nm::ACTIVE_ACTIVATED => "on",
                    nm::ACTIVE_ACTIVATING => "starting",
                    _ => "stopping",
                })
        };

        let mut wifi_devices = Vec::new();
        let mut wired = Vec::new();
        let mut aps = Vec::new();
        let mut can_hotspot = false;
        let mut scanned = false;
        for (path, d) in self.devices(&conn).await? {
            let ty = nm::num(&d, "DeviceType");
            let state = nm::num(&d, "State");
            let iface = nm::text(&d, "Interface");
            match ty {
                nm::DEVICE_WIFI => {
                    let (w, mut list) = self.access_points(&conn, &path).await?;
                    can_hotspot |= nm::num(&w, "WirelessCapabilities") & nm::WIFI_CAP_AP != 0;
                    scanned |=
                        nm::get::<i64>(&w, "LastScan").unwrap_or(-1) >= 0 || !list.is_empty();
                    wifi_devices.push(json!({
                        "path": path.as_str(), "name": iface,
                        "state": state_text(state, true),
                    }));
                    aps.append(&mut list);
                }
                nm::DEVICE_ETHERNET => {
                    let carrier = nm::get_all(&conn, path.as_str(), nm::IFACE_WIRED)
                        .await
                        .ok();
                    let has_carrier = carrier.as_ref().is_some_and(|c| nm::flag(c, "Carrier"));
                    let speed = carrier.as_ref().map_or(0, |c| nm::num(c, "Speed"));
                    let ac = nm::path(&d, "ActiveConnection");
                    let using = active.iter().find(|a| a.path == ac).map(|a| a.uuid.clone());
                    wired.push(json!({
                        "path": path.as_str(), "name": iface,
                        "state": state_text(state, has_carrier),
                        "connected": state == nm::STATE_ACTIVATED,
                        "speed": if speed > 0 { format!("{speed} Mb/s") } else { String::new() },
                        "uuid": using,
                    }));
                }
                _ => {}
            }
        }

        // One row per network name: the strongest access point stands for it.
        aps.sort_by_key(|a| std::cmp::Reverse(a.strength));
        let mut networks: Vec<Value> = Vec::new();
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for ap in &aps {
            if ap.ssid.is_empty() || seen.contains(&ap.ssid) {
                continue;
            }
            seen.push(ap.ssid.clone());
            let saved_uuid = saved
                .iter()
                .find(|s| {
                    profile::kind(&s.settings) == "wifi" && profile::ssid(&s.settings) == ap.ssid
                })
                .map(|s| profile::uuid(&s.settings));
            let state = saved_uuid.as_deref().map_or("off", state_of);
            networks.push(json!({
                "ssid": nm::ssid_text(&ap.ssid),
                "strength": ap.strength,
                "bars": match ap.strength { 0..=24 => 1, 25..=49 => 2, 50..=74 => 3, _ => 4 },
                "security": ap.security.id(),
                "band": band(ap.frequency),
                "ap": ap.path.as_str(),
                "device": ap.device.as_str(),
                "saved": saved_uuid,
                "state": state,
            }));
        }
        networks.sort_by_key(|n| {
            (
                n["state"] == "off",
                n["saved"].is_null(),
                std::cmp::Reverse(n["strength"].as_u64().unwrap_or(0)),
            )
        });

        let mut list = Vec::new();
        let mut hotspot = json!({"can": can_hotspot, "state": "off", "uuid": null, "name": ""});
        for s in &saved {
            let k = profile::kind(&s.settings);
            if k == "loopback" {
                continue;
            }
            let mut j = profile::summary(s.path.as_str(), &s.settings);
            let st = state_of(&profile::uuid(&s.settings));
            j["state"] = json!(st);
            if k == "hotspot" && (hotspot["uuid"].is_null() || st != "off") {
                hotspot["uuid"] = j["uuid"].clone();
                hotspot["state"] = json!(st);
                hotspot["name"] = j["ssid"].clone();
            }
            list.push(j);
        }
        list.sort_by_key(|j| {
            (
                j["state"] == "off",
                j["kind"].as_str().unwrap_or("").to_string(),
                j["name"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        let vpn_on = active
            .iter()
            .any(|a| a.vpn && a.state == nm::ACTIVE_ACTIVATED);

        Ok(json!({
            "available": true,
            "wifi": {
                "enabled": nm::flag(&props, "WirelessEnabled"),
                "hardware": nm::get::<bool>(&props, "WirelessHardwareEnabled").unwrap_or(true),
                "devices": wifi_devices,
                "networks": networks,
                "scanned": scanned,
            },
            "wired": wired,
            "saved": list,
            "vpn_on": vpn_on,
            "openvpn": self.openvpn(),
            "hotspot": hotspot,
        }))
    }

    /// Mark `uuid` as started from the page: the agent may ask for its secrets.
    fn started(&self, uuid: &str) {
        self.shared.started.lock().unwrap().insert(uuid.to_string());
    }

    async fn connect_wifi(
        &self,
        ap: &str,
        device: &str,
        password: String,
        all_users: bool,
        eap: Option<Eap>,
    ) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let ap = OwnedObjectPath::try_from(ap)
            .map_err(|_| failure("That network is gone. Scan again."))?;
        let device =
            OwnedObjectPath::try_from(device).map_err(|_| failure("That Wi-Fi device is gone."))?;
        let a = nm::get_all(&conn, ap.as_str(), nm::IFACE_AP)
            .await
            .map_err(|_| failure("That network is gone. Scan again."))?;
        let ssid = nm::get::<Vec<u8>>(&a, "Ssid").unwrap_or_default();
        let sec = profile::security(
            nm::num(&a, "Flags"),
            nm::num(&a, "WpaFlags"),
            nm::num(&a, "RsnFlags"),
        );
        let nm = NetworkManagerProxy::new(&conn).await?;
        let existing =
            self.saved(&conn).await?.into_iter().find(|s| {
                profile::kind(&s.settings) == "wifi" && profile::ssid(&s.settings) == ssid
            });
        if let Some(s) = existing {
            self.started(&profile::uuid(&s.settings));
            return settle(
                nm.activate_connection(&s.path, &device, &ap).await,
                &NETWORK,
            );
        }
        let uuid = profile::new_uuid();
        let settings = profile::wifi_settings(
            &uuid,
            &self.user,
            &WifiInput {
                ssid,
                security: Some(sec),
                password,
                all_users,
                eap,
            },
        )
        .map_err(failure)?;
        self.started(&uuid);
        settle(
            nm.add_and_activate_connection(&settings, &device, &ap)
                .await,
            &NETWORK,
        )
    }

    async fn first_wifi(
        &self,
        conn: &zbus::Connection,
        need_ap: bool,
    ) -> zbus::Result<OwnedObjectPath> {
        for (path, d) in self.devices(conn).await? {
            if nm::num(&d, "DeviceType") != nm::DEVICE_WIFI {
                continue;
            }
            if need_ap {
                let w = nm::get_all(conn, path.as_str(), nm::IFACE_WIRELESS).await?;
                if nm::num(&w, "WirelessCapabilities") & nm::WIFI_CAP_AP == 0 {
                    continue;
                }
            }
            return Ok(path);
        }
        Err(failure(if need_ap {
            "This computer's Wi-Fi can't share a connection."
        } else {
            "This computer has no Wi-Fi."
        }))
    }

    async fn hotspot(&self, on: bool, name: &str, password: String) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let nm = NetworkManagerProxy::new(&conn).await?;
        let saved = self.saved(&conn).await?;
        let mine = saved
            .into_iter()
            .find(|s| profile::kind(&s.settings) == "hotspot");
        if !on {
            let Some(s) = mine else { return Ok(()) };
            let uuid = profile::uuid(&s.settings);
            for a in self.active(&conn).await? {
                if a.uuid == uuid {
                    settle(nm.deactivate_connection(&a.path).await, &NETWORK)?;
                }
            }
            return Ok(());
        }
        let device = self.first_wifi(&conn, true).await?;
        match mine {
            Some(mut s) => {
                let uuid = profile::uuid(&s.settings);
                if !password.is_empty() {
                    let name = if name.trim().is_empty() {
                        nm::ssid_text(&profile::ssid(&s.settings))
                    } else {
                        name.to_string()
                    };
                    let mut fresh = profile::hotspot_settings(&uuid, &self.user, &name, &password)
                        .map_err(failure)?;
                    for key in ["802-11-wireless", "802-11-wireless-security"] {
                        if let Some(block) = fresh.remove(key) {
                            s.settings.insert(key.into(), block);
                        }
                    }
                    let c: NmConnectionProxy = nm::at(&conn, &s.path).await?;
                    let r = c.update(&s.settings).await;
                    let ok = r.is_ok();
                    settle(r, &NETWORK)?;
                    if !ok {
                        return Ok(());
                    }
                }
                self.started(&uuid);
                settle(
                    nm.activate_connection(&s.path, &device, &nm::root()).await,
                    &NETWORK,
                )
            }
            None => {
                let uuid = profile::new_uuid();
                let settings = profile::hotspot_settings(&uuid, &self.user, name, &password)
                    .map_err(failure)?;
                self.started(&uuid);
                settle(
                    nm.add_and_activate_connection(&settings, &device, &nm::root())
                        .await,
                    &NETWORK,
                )
            }
        }
    }

    async fn import_wireguard(&self, path: &Path, all_users: bool) -> zbus::Result<()> {
        let meta = std::fs::metadata(path)
            .map_err(|e| failure(format!("Can't read {}: {e}", path.display())))?;
        if !meta.is_file() || meta.len() > MAX_IMPORT {
            return Err(failure(format!(
                "{} isn't a WireGuard config file.",
                path.display()
            )));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| failure(format!("Can't read {}: {e}", path.display())))?;
        let cfg = wireguard::parse(&text).map_err(failure)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "WireGuard".into());
        let uuid = profile::new_uuid();
        let settings =
            wireguard::settings(&cfg, &name, &uuid, &self.user, all_users).map_err(failure)?;
        let conn = self.conn().await?;
        let s = NmSettingsProxy::new(&conn).await?;
        let r = s.add_connection(&settings).await;
        let ok = r.is_ok();
        settle(r, &NETWORK)?;
        if ok {
            let mut text = format!("Added {name}. Start it from the list below.");
            if !cfg.left_out.is_empty() {
                text.push_str(&format!(
                    " These lines of the file were left out because only the command-line tool uses them: {}.",
                    cfg.left_out.join(", ")
                ));
            }
            self.notice(&text);
        }
        Ok(())
    }

    /// Answer the question on show; a stale one (already answered or cancelled) does nothing.
    fn answer(&self, id: u64, a: Answer) {
        self.shared.prompts.answer(id, a);
    }

    /// Register the secret agent on `conn` (once its object is served).
    async fn register(&self, conn: &zbus::Connection) -> bool {
        let ok = async {
            let m = AgentManagerProxy::new(conn).await?;
            m.register_with_capabilities(nm::AGENT_ID, 0).await
        };
        let ok = sysbus::within(ok).await.is_ok();
        self.registered
            .store(ok, std::sync::atomic::Ordering::SeqCst);
        ok
    }

    /// Serve the agent, register it, and follow NetworkManager's signals while the page is
    /// open. A device that fails while something started here was connecting shows why.
    async fn follow(&self, changed: &Notify) -> zbus::Result<()> {
        let conn = self.conn().await?;
        conn.object_server()
            .at(
                nm::AGENT_PATH,
                agent::SecretAgent {
                    shared: self.shared.clone(),
                },
            )
            .await?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path_namespace(nm::NM_PATH)?
            .build();
        let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, Some(256)).await?;
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let mut owners = dbus
            .receive_name_owner_changed_with_args(&[(0, nm::NM)])
            .await?;
        self.register(&conn).await;
        changed.notify_one();
        loop {
            tokio::select! {
                Some(msg) = signals.next() => {
                    let Ok(msg) = msg else { continue };
                    let h = msg.header();
                    if h.member().is_some_and(|m| m.as_str() == "StateChanged")
                        && h.interface().is_some_and(|i| i.as_str() == nm::IFACE_DEVICE)
                        && let Ok((new, _old, reason)) = msg.body().deserialize::<(u32, u32, u32)>()
                        && new == nm::STATE_FAILED
                        && !self.shared.started.lock().unwrap().is_empty()
                    {
                        self.notice(&format!("Couldn't connect: {}", reason_text(reason)));
                    }
                    changed.notify_one();
                }
                Some(owner) = owners.next() => {
                    // NetworkManager restarted: it forgot its agents.
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

impl PageLogic for NetworkState {
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
        match op {
            Op::Refresh => Ok(()),
            Op::AnswerSecret { id, values } => {
                self.answer(id, Answer::Yes(values));
                Ok(())
            }
            Op::CancelSecret { id } => {
                self.answer(id, Answer::No);
                Ok(())
            }
            Op::SetWifi(on) => {
                let conn = self.conn().await?;
                let nm = NetworkManagerProxy::new(&conn).await?;
                let r = nm
                    .inner()
                    .set_property("WirelessEnabled", on)
                    .await
                    .map_err(|e| zbus::Error::FDO(Box::new(e)));
                settle(r, &NETWORK)
            }
            Op::Scan => {
                let conn = self.conn().await?;
                for (path, d) in self.devices(&conn).await? {
                    if nm::num(&d, "DeviceType") == nm::DEVICE_WIFI {
                        let w: NmWirelessProxy = nm::at(&conn, &path).await?;
                        // "Scanning not allowed" right after a scan is not worth a message.
                        let _ = w.request_scan(HashMap::new()).await;
                    }
                }
                Ok(())
            }
            Op::ConnectWifi {
                ap,
                device,
                password,
                all_users,
                eap,
            } => {
                self.connect_wifi(&ap, &device, password, all_users, eap)
                    .await
            }
            Op::ConnectHidden {
                ssid,
                security,
                password,
                all_users,
            } => {
                let conn = self.conn().await?;
                let device = self.first_wifi(&conn, false).await?;
                let sec = match security.as_str() {
                    "open" => Security::Open,
                    "password" => Security::Password { sae_only: false },
                    _ => return Err(failure("Choose Open or Password.")),
                };
                let uuid = profile::new_uuid();
                let mut settings = profile::wifi_settings(
                    &uuid,
                    &self.user,
                    &WifiInput {
                        ssid: ssid.trim().as_bytes().to_vec(),
                        security: Some(sec),
                        password,
                        all_users,
                        eap: None,
                    },
                )
                .map_err(failure)?;
                settings
                    .get_mut("802-11-wireless")
                    .expect("set by wifi_settings")
                    .insert("hidden".into(), profile::ov(true));
                self.started(&uuid);
                let nm = NetworkManagerProxy::new(&conn).await?;
                settle(
                    nm.add_and_activate_connection(&settings, &device, &nm::root())
                        .await,
                    &NETWORK,
                )
            }
            Op::Activate { uuid } => {
                let conn = self.conn().await?;
                let s = self.find(&conn, &uuid).await?;
                self.started(&uuid);
                let nm = NetworkManagerProxy::new(&conn).await?;
                settle(
                    nm.activate_connection(&s.path, &nm::root(), &nm::root())
                        .await,
                    &NETWORK,
                )
            }
            Op::Deactivate { uuid } => {
                let conn = self.conn().await?;
                let nm = NetworkManagerProxy::new(&conn).await?;
                for a in self.active(&conn).await? {
                    if a.uuid == uuid {
                        settle(nm.deactivate_connection(&a.path).await, &NETWORK)?;
                    }
                }
                Ok(())
            }
            Op::Forget { uuid } => {
                let conn = self.conn().await?;
                let s = self.find(&conn, &uuid).await?;
                let c: NmConnectionProxy = nm::at(&conn, &s.path).await?;
                settle(c.delete().await, &NETWORK)
            }
            Op::SetAutoconnect { uuid, on } => {
                self.edit(&uuid, |s| {
                    profile::set_autoconnect(s, on);
                    Ok(())
                })
                .await
            }
            Op::SetMetered { uuid, metered } => {
                self.edit(&uuid, |s| profile::set_metered(s, &metered))
                    .await
            }
            Op::SetEveryone { uuid, on } => {
                let user = self.user.clone();
                self.edit(&uuid, |s| {
                    s.entry("connection".into()).or_default().insert(
                        "permissions".into(),
                        profile::ov(profile::permissions(&user, on)),
                    );
                    Ok(())
                })
                .await
            }
            Op::SetIp { uuid, family, ip } => {
                self.edit(&uuid, |s| profile::apply_ip(s, family, &ip))
                    .await
            }
            Op::ImportWireGuard { path, all_users } => {
                self.import_wireguard(&path, all_users).await
            }
            Op::EditWireGuard { uuid, peer } => {
                self.edit(&uuid, |s| wireguard::edit_peer(s, &peer)).await
            }
            Op::Hotspot { on, name, password } => self.hotspot(on, &name, password).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let mut state = match sysbus::within(self.read()).await {
            Ok(v) => v,
            Err(e) => sysbus::read_error(&e, &NETWORK),
        };
        let (seq, text) = self.notice.lock().unwrap().clone();
        state["notice"] = json!({"seq": seq, "text": text});
        state["secret"] = self.shared.prompts.current();
        state["agent"] = json!(self.registered.load(std::sync::atomic::Ordering::SeqCst));
        Ok(state)
    }

    async fn watch(&self, changed: Arc<Notify>) {
        self.shared.prompts.follow(changed.clone());
        if let Err(e) = self.follow(&changed).await {
            eprintln!(
                "uq-settings: can't follow network changes: {}",
                sysbus::error_text(&e)
            );
        }
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests;
