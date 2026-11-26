//! A fake NetworkManager: one Wi-Fi card (`wlan0`, can be a hotspot) with access points, one
//! wired card (`enp3s0`), saved connections that keep their secrets the way NetworkManager does
//! (left out of `GetSettings`, kept by an `Update` that doesn't send them), active connections,
//! and the agent manager. A connection marked as needing a secret asks the registered secret
//! agent for it when started, like NetworkManager does for a missing or wrong password.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::wire::{OwnedObjectPath, OwnedVariant as OwnedValue};
use zbus::{ObjectServer, fdo};

use super::{Deny, Shared, SysRig};
mod devices;
use devices::*;
mod settings;
use settings::*;
mod connect;
use connect::*;

pub type Settings = HashMap<String, HashMap<String, OwnedValue>>;

pub const WIFI_DEV: &str = "/org/freedesktop/NetworkManager/Devices/1";
pub const WIRED_DEV: &str = "/org/freedesktop/NetworkManager/Devices/2";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager")]
pub enum NmError {
    #[zbus(error)]
    ZBus(zbus::Error),
    PermissionDenied(String),
    UnknownConnection(String),
    Failed(String),
}

pub struct FakeAp {
    pub ssid: Vec<u8>,
    pub strength: u8,
    pub flags: u32,
    pub wpa: u32,
    pub rsn: u32,
    pub frequency: u32,
}

pub struct FakeConn {
    pub path: String,
    /// Everything, secrets included.
    pub settings: Settings,
    /// Starting it asks the agent for this setting's secrets first.
    pub needs_secret: Option<String>,
}

pub struct FakeActive {
    pub path: String,
    pub conn: String,
    pub uuid: String,
    pub state: u32,
    pub vpn: bool,
}

#[derive(Default)]
pub struct NmState {
    pub wifi_enabled: bool,
    pub aps: Vec<FakeAp>,
    pub conns: Vec<FakeConn>,
    pub active: Vec<FakeActive>,
    pub carrier: bool,
    /// The unique bus name of the registered secret agent.
    pub agent: Option<String>,
    /// What the agent answered: the secrets, or the D-Bus error name.
    pub agent_replies: Vec<Result<Settings, String>>,
    pub scans: u32,
    next: u32,
}

pub fn clone_settings(s: &Settings) -> Settings {
    s.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.iter()
                    .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
                    .collect(),
            )
        })
        .collect()
}

fn text(s: &Settings, block: &str, key: &str) -> String {
    s.get(block)
        .and_then(|b| b.get(key))
        .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
        .unwrap_or_default()
}

/// The keys NetworkManager treats as secrets, per block.
const SECRETS: [(&str, &str); 7] = [
    ("802-11-wireless-security", "psk"),
    ("802-11-wireless-security", "wep-key0"),
    ("802-1x", "password"),
    ("802-1x", "private-key-password"),
    ("wireguard", "private-key"),
    ("vpn", "secrets"),
    ("wireguard", "peers/preshared-key"),
];

/// `s` without its secrets, as `GetSettings` returns it.
pub fn without_secrets(s: &Settings) -> Settings {
    let mut out = clone_settings(s);
    for (block, key) in SECRETS {
        let Some(b) = out.get_mut(block) else {
            continue;
        };
        if key == "peers/preshared-key" {
            if let Some(peers) = b.get("peers").and_then(|p| {
                Vec::<HashMap<String, OwnedValue>>::try_from(p.try_clone().ok()?).ok()
            }) {
                let peers: Vec<HashMap<String, OwnedValue>> = peers
                    .into_iter()
                    .map(|mut p| {
                        p.remove("preshared-key");
                        p
                    })
                    .collect();
                b.insert(
                    "peers".into(),
                    OwnedValue::try_from(zbus::wire::Variant::from(peers)).unwrap(),
                );
            }
        } else {
            b.remove(key);
        }
    }
    out
}

impl NmState {
    fn path(&mut self, kind: &str) -> String {
        self.next += 1;
        format!("/org/freedesktop/NetworkManager/{kind}/{}", self.next)
    }

    pub fn conn(&self, uuid: &str) -> Option<&FakeConn> {
        self.conns
            .iter()
            .find(|c| text(&c.settings, "connection", "uuid") == uuid)
    }

    pub fn secret(&self, uuid: &str, block: &str, key: &str) -> String {
        self.conn(uuid)
            .map(|c| text(&c.settings, block, key))
            .unwrap_or_default()
    }
}

struct NmIface {
    s: Shared<NmState>,
    deny: Deny,
}

fn denied() -> NmError {
    NmError::PermissionDenied("Not authorized to control networking.".into())
}

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl NmIface {
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.s.lock().unwrap().wifi_enabled
    }
    #[zbus(property)]
    fn set_wireless_enabled(&mut self, on: bool) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(fdo::Error::AccessDenied("Not authorized".into()));
        }
        self.s.lock().unwrap().wifi_enabled = on;
        Ok(())
    }
    #[zbus(property)]
    fn wireless_hardware_enabled(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn active_connections(&self) -> Vec<OwnedObjectPath> {
        self.s
            .lock()
            .unwrap()
            .active
            .iter()
            .map(|a| OwnedObjectPath::try_from(a.path.as_str()).unwrap())
            .collect()
    }
    #[zbus(property)]
    fn version(&self) -> String {
        "1.52.0".into()
    }

    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        [WIFI_DEV, WIRED_DEV]
            .iter()
            .map(|p| OwnedObjectPath::try_from(*p).unwrap())
            .collect()
    }

    async fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        _device: OwnedObjectPath,
        _specific_object: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<OwnedObjectPath, NmError> {
        if self.deny.on() {
            return Err(denied());
        }
        activate(&self.s, conn, server, connection.as_str()).await
    }

    async fn add_and_activate_connection(
        &self,
        connection: Settings,
        _device: OwnedObjectPath,
        _specific_object: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(OwnedObjectPath, OwnedObjectPath), NmError> {
        if self.deny.on() {
            return Err(denied());
        }
        let path = add(&self.s, server, connection).await?;
        let active = activate(&self.s, conn, server, path.as_str()).await?;
        Ok((path, active))
    }

    async fn deactivate_connection(
        &self,
        active: OwnedObjectPath,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(), NmError> {
        if self.deny.on() {
            return Err(denied());
        }
        self.s
            .lock()
            .unwrap()
            .active
            .retain(|a| a.path != active.as_str());
        let _ = server.remove::<ActiveIface, _>(active.as_str()).await;
        Ok(())
    }
}

pub struct FakeNm {
    pub state: Shared<NmState>,
    pub deny: Deny,
    pub conn: zbus::Connection,
}

/// Open, WPA2 and enterprise networks; the strongest first only by chance.
pub fn sample_aps() -> Vec<FakeAp> {
    vec![
        FakeAp {
            ssid: b"Cafe".to_vec(),
            strength: 40,
            flags: 0,
            wpa: 0,
            rsn: 0,
            frequency: 2412,
        },
        FakeAp {
            ssid: b"Home".to_vec(),
            strength: 80,
            flags: 1,
            wpa: 0,
            rsn: 0x100,
            frequency: 5180,
        },
        FakeAp {
            ssid: b"Home".to_vec(),
            strength: 30,
            flags: 1,
            wpa: 0,
            rsn: 0x100,
            frequency: 2437,
        },
        FakeAp {
            ssid: b"eduroam".to_vec(),
            strength: 60,
            flags: 1,
            wpa: 0x200,
            rsn: 0x200,
            frequency: 5500,
        },
        FakeAp {
            ssid: Vec::new(),
            strength: 90,
            flags: 1,
            wpa: 0,
            rsn: 0x100,
            frequency: 2462,
        },
    ]
}

impl FakeNm {
    pub async fn start(rig: &mut SysRig) -> Self {
        let state: Shared<NmState> = Arc::new(Mutex::new(NmState {
            wifi_enabled: true,
            carrier: true,
            aps: sample_aps(),
            ..Default::default()
        }));
        let deny = Deny::default();
        let conn = rig.own("org.freedesktop.NetworkManager").await;
        let os = conn.object_server();
        os.at(
            "/org/freedesktop/NetworkManager",
            NmIface {
                s: state.clone(),
                deny: deny.clone(),
            },
        )
        .await
        .unwrap();
        os.at(
            "/org/freedesktop/NetworkManager/Settings",
            SettingsIface {
                s: state.clone(),
                deny: deny.clone(),
            },
        )
        .await
        .unwrap();
        os.at(
            "/org/freedesktop/NetworkManager/AgentManager",
            AgentManagerIface { s: state.clone() },
        )
        .await
        .unwrap();
        os.at(
            WIFI_DEV,
            DeviceIface {
                s: state.clone(),
                wifi: true,
            },
        )
        .await
        .unwrap();
        os.at(WIFI_DEV, WirelessIface { s: state.clone() })
            .await
            .unwrap();
        os.at(
            WIRED_DEV,
            DeviceIface {
                s: state.clone(),
                wifi: false,
            },
        )
        .await
        .unwrap();
        os.at(WIRED_DEV, WiredIface { s: state.clone() })
            .await
            .unwrap();
        let n = state.lock().unwrap().aps.len();
        for i in 0..n {
            os.at(
                ap_path(i),
                ApIface {
                    s: state.clone(),
                    i,
                },
            )
            .await
            .unwrap();
        }
        Self { state, deny, conn }
    }

    /// Save a connection (secrets included), as if added earlier.
    pub async fn save(&self, settings: Settings, needs_secret: Option<&str>) -> String {
        let path = add(&self.state, self.conn.object_server(), settings)
            .await
            .unwrap()
            .to_string();
        let mut st = self.state.lock().unwrap();
        st.conns.last_mut().unwrap().needs_secret = needs_secret.map(str::to_string);
        path
    }
}
