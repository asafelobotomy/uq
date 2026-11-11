//! A fake NetworkManager: one Wi-Fi card (`wlan0`, can be a hotspot) with access points, one
//! wired card (`enp3s0`), saved connections that keep their secrets the way NetworkManager does
//! (left out of `GetSettings`, kept by an `Update` that doesn't send them), active connections,
//! and the agent manager. A connection marked as needing a secret asks the registered secret
//! agent for it when started, like NetworkManager does for a missing or wrong password.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{ObjectServer, fdo};

use super::{Deny, Shared, SysRig};

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
                    OwnedValue::try_from(zbus::zvariant::Value::from(peers)).unwrap(),
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

/// Start `conn_path`: active at once, or after the agent answers for a secret it needs.
async fn activate(
    s: &Shared<NmState>,
    conn: &zbus::Connection,
    server: &ObjectServer,
    conn_path: &str,
) -> Result<OwnedObjectPath, NmError> {
    let (active_path, needs, settings, agent, uuid) = {
        let mut st = s.lock().unwrap();
        let Some(c) = st.conns.iter().find(|c| c.path == conn_path) else {
            return Err(NmError::UnknownConnection("No such connection".into()));
        };
        let uuid = text(&c.settings, "connection", "uuid");
        let vpn = matches!(
            text(&c.settings, "connection", "type").as_str(),
            "vpn" | "wireguard"
        );
        let needs = c.needs_secret.clone();
        let settings = without_secrets(&c.settings);
        let path = st.path("ActiveConnection");
        st.active.push(FakeActive {
            path: path.clone(),
            conn: conn_path.to_string(),
            uuid: uuid.clone(),
            state: if needs.is_some() { 1 } else { 2 },
            vpn,
        });
        (path, needs, settings, st.agent.clone(), uuid)
    };
    server
        .at(
            active_path.as_str(),
            ActiveIface {
                s: s.clone(),
                path: active_path.clone(),
            },
        )
        .await?;
    if let Some(setting) = needs {
        let s = s.clone();
        let conn = conn.clone();
        let conn_path = conn_path.to_string();
        let active = active_path.clone();
        tokio::spawn(async move {
            let reply = match agent {
                None => Err("no agent".to_string()),
                Some(agent) => conn
                    .call_method(
                        Some(agent.as_str()),
                        "/org/freedesktop/NetworkManager/SecretAgent",
                        Some("org.freedesktop.NetworkManager.SecretAgent"),
                        "GetSecrets",
                        &(
                            settings,
                            OwnedObjectPath::try_from(conn_path.as_str()).unwrap(),
                            setting.as_str(),
                            Vec::<String>::new(),
                            1u32,
                        ),
                    )
                    .await
                    .map_err(|e| match e {
                        zbus::Error::MethodError(name, _, _) => name.to_string(),
                        other => other.to_string(),
                    })
                    .and_then(|m| {
                        m.body()
                            .deserialize::<Settings>()
                            .map_err(|e| e.to_string())
                    }),
            };
            let ok = reply.is_ok();
            {
                let mut st = s.lock().unwrap();
                if let Ok(secrets) = &reply {
                    if let Some(c) = st.conns.iter_mut().find(|c| c.path == conn_path) {
                        for (block, vals) in secrets {
                            let b = c.settings.entry(block.clone()).or_default();
                            for (k, v) in vals {
                                b.insert(k.clone(), v.try_clone().unwrap());
                            }
                        }
                        c.needs_secret = None;
                    }
                    if let Some(a) = st.active.iter_mut().find(|a| a.path == active) {
                        a.state = 2;
                    }
                } else {
                    st.active.retain(|a| a.path != active);
                }
                st.agent_replies.push(reply);
            }
            let (new, reason) = if ok { (100u32, 0u32) } else { (120, 7) };
            let _ = conn
                .emit_signal(
                    None::<()>,
                    WIFI_DEV,
                    "org.freedesktop.NetworkManager.Device",
                    "StateChanged",
                    &(new, 50u32, reason),
                )
                .await;
        });
    }
    let _ = uuid;
    Ok(OwnedObjectPath::try_from(active_path).unwrap())
}

async fn add(
    s: &Shared<NmState>,
    server: &ObjectServer,
    settings: Settings,
) -> Result<OwnedObjectPath, NmError> {
    let path = {
        let mut st = s.lock().unwrap();
        let path = st.path("Settings");
        st.conns.push(FakeConn {
            path: path.clone(),
            settings,
            needs_secret: None,
        });
        path
    };
    server
        .at(
            path.as_str(),
            ConnIface {
                s: s.clone(),
                path: path.clone(),
            },
        )
        .await?;
    Ok(OwnedObjectPath::try_from(path).unwrap())
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

struct SettingsIface {
    s: Shared<NmState>,
    deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl SettingsIface {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        self.s
            .lock()
            .unwrap()
            .conns
            .iter()
            .map(|c| OwnedObjectPath::try_from(c.path.as_str()).unwrap())
            .collect()
    }

    async fn add_connection(
        &self,
        connection: Settings,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<OwnedObjectPath, NmError> {
        if self.deny.on() {
            return Err(denied());
        }
        add(&self.s, server, connection).await
    }
}

struct ConnIface {
    s: Shared<NmState>,
    path: String,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl ConnIface {
    fn get_settings(&self) -> Result<Settings, NmError> {
        let st = self.s.lock().unwrap();
        st.conns
            .iter()
            .find(|c| c.path == self.path)
            .map(|c| without_secrets(&c.settings))
            .ok_or_else(|| NmError::UnknownConnection("gone".into()))
    }

    fn update(&self, properties: Settings) -> Result<(), NmError> {
        let mut st = self.s.lock().unwrap();
        let c = st
            .conns
            .iter_mut()
            .find(|c| c.path == self.path)
            .ok_or_else(|| NmError::UnknownConnection("gone".into()))?;
        // Secrets the update leaves out are kept.
        let mut next = properties;
        for (block, key) in SECRETS {
            if key.contains('/') {
                continue;
            }
            if let Some(v) = c.settings.get(block).and_then(|b| b.get(key)) {
                let b = next.entry(block.to_string()).or_default();
                if !b.contains_key(key) {
                    b.insert(key.to_string(), v.try_clone().unwrap());
                }
            }
        }
        c.settings = next;
        Ok(())
    }

    async fn delete(&self, #[zbus(object_server)] server: &ObjectServer) -> Result<(), NmError> {
        self.s.lock().unwrap().conns.retain(|c| c.path != self.path);
        let _ = server.remove::<ConnIface, _>(self.path.as_str()).await;
        Ok(())
    }
}

struct AgentManagerIface {
    s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AgentManager")]
impl AgentManagerIface {
    fn register_with_capabilities(
        &self,
        _identifier: String,
        _capabilities: u32,
        #[zbus(header)] hdr: Header<'_>,
    ) {
        self.s.lock().unwrap().agent = hdr.sender().map(|s| s.to_string());
    }

    fn unregister(&self) {
        self.s.lock().unwrap().agent = None;
    }
}

struct DeviceIface {
    s: Shared<NmState>,
    wifi: bool,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl DeviceIface {
    #[zbus(property)]
    fn interface(&self) -> String {
        if self.wifi { "wlan0" } else { "enp3s0" }.into()
    }
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        if self.wifi { 2 } else { 1 }
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        let st = self.s.lock().unwrap();
        let mine = |a: &&FakeActive| {
            let kind = st
                .conns
                .iter()
                .find(|c| c.path == a.conn)
                .map(|c| text(&c.settings, "connection", "type"))
                .unwrap_or_default();
            (kind == "802-11-wireless") == self.wifi && (kind != "vpn" && kind != "wireguard")
        };
        match st.active.iter().find(mine) {
            Some(a) if a.state == 2 => 100,
            Some(_) => 50,
            None if !self.wifi && !st.carrier => 20,
            None => 30,
        }
    }
    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        let st = self.s.lock().unwrap();
        let found = st.active.iter().find(|a| {
            st.conns.iter().find(|c| c.path == a.conn).is_some_and(|c| {
                (text(&c.settings, "connection", "type") == "802-3-ethernet") != self.wifi
            })
        });
        OwnedObjectPath::try_from(found.map_or("/", |a| a.path.as_str())).unwrap()
    }
}

fn ap_path(i: usize) -> String {
    format!("/org/freedesktop/NetworkManager/AccessPoint/{i}")
}

struct WirelessIface {
    s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl WirelessIface {
    #[zbus(property)]
    fn access_points(&self) -> Vec<OwnedObjectPath> {
        (0..self.s.lock().unwrap().aps.len())
            .map(|i| OwnedObjectPath::try_from(ap_path(i)).unwrap())
            .collect()
    }
    #[zbus(property)]
    fn last_scan(&self) -> i64 {
        1000
    }
    #[zbus(property)]
    fn wireless_capabilities(&self) -> u32 {
        0x40 | 0x1
    }

    async fn request_scan(
        &self,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.s.lock().unwrap().scans += 1;
        self.last_scan_changed(&emitter).await?;
        Ok(())
    }
}

struct WiredIface {
    s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wired")]
impl WiredIface {
    #[zbus(property)]
    fn carrier(&self) -> bool {
        self.s.lock().unwrap().carrier
    }
    #[zbus(property)]
    fn speed(&self) -> u32 {
        1000
    }
}

struct ApIface {
    s: Shared<NmState>,
    i: usize,
}

impl ApIface {
    fn with<T: Default>(&self, f: impl FnOnce(&FakeAp) -> T) -> T {
        self.s
            .lock()
            .unwrap()
            .aps
            .get(self.i)
            .map(f)
            .unwrap_or_default()
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl ApIface {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.with(|a| a.ssid.clone())
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.with(|a| a.strength)
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        self.with(|a| a.flags)
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        self.with(|a| a.wpa)
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.with(|a| a.rsn)
    }
    #[zbus(property)]
    fn frequency(&self) -> u32 {
        self.with(|a| a.frequency)
    }
}

struct ActiveIface {
    s: Shared<NmState>,
    path: String,
}

impl ActiveIface {
    fn with<T: Default>(&self, f: impl FnOnce(&FakeActive) -> T) -> T {
        let st = self.s.lock().unwrap();
        st.active
            .iter()
            .find(|a| a.path == self.path)
            .map(f)
            .unwrap_or_default()
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl ActiveIface {
    #[zbus(property)]
    fn uuid(&self) -> String {
        self.with(|a| a.uuid.clone())
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.with(|a| a.state)
    }
    #[zbus(property)]
    fn vpn(&self) -> bool {
        self.with(|a| a.vpn)
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
