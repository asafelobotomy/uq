//! A fake BlueZ: one adapter (`hci0`), paired headphones with a battery, a keyboard that can be
//! paired with a code to confirm, a speaker that asks for a PIN, a device that doesn't say its
//! name, and the agent manager. Pairing asks the registered agent like BlueZ does; the speaker
//! only turns up while the adapter is looking for devices.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedObjectPath;
use zbus::{ObjectServer, fdo};

use super::{Shared, SysRig};

pub const ADAPTER: &str = "/org/bluez/hci0";
pub const HEADPHONES: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_01";
pub const KEYBOARD: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_02";
pub const NAMELESS: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_03";
pub const SPEAKER: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_04";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub enum BluezError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotReady(String),
    Failed(String),
    AuthenticationFailed(String),
    AuthenticationCanceled(String),
    DoesNotExist(String),
}

#[derive(Debug, Clone, Default)]
pub struct FakeDevice {
    pub name: Option<String>,
    pub icon: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    /// How pairing asks: `confirm` (a code to compare) or `pin` (a PIN to type).
    pub pairing: &'static str,
    /// The PIN the agent gave, when it asked for one.
    pub pin: String,
}

#[derive(Debug, Default)]
pub struct BluezState {
    pub powered: bool,
    pub discoverable: bool,
    pub discovering: bool,
    /// The bus name that asked for discovery (BlueZ stops it when that name leaves).
    pub discovery_owner: Option<String>,
    pub devices: HashMap<String, FakeDevice>,
    /// `(sender, path, capability)` of the registered agent.
    pub agent: Option<(String, String, String)>,
    pub default_agent: Option<String>,
    /// What each pairing's agent call ended with: "ok" or the D-Bus error name.
    pub pair_results: Vec<String>,
    /// Make powering on fail as an rfkill block does.
    pub blocked: bool,
}

struct AgentManager {
    s: Shared<BluezState>,
}

#[zbus::interface(name = "org.bluez.AgentManager1")]
impl AgentManager {
    fn register_agent(
        &self,
        agent: OwnedObjectPath,
        capability: String,
        #[zbus(header)] hdr: Header<'_>,
    ) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        self.s.lock().unwrap().agent = Some((sender, agent.to_string(), capability));
    }

    fn unregister_agent(&self, _agent: OwnedObjectPath) {
        let mut s = self.s.lock().unwrap();
        s.agent = None;
        s.default_agent = None;
    }

    fn request_default_agent(&self, agent: OwnedObjectPath) {
        self.s.lock().unwrap().default_agent = Some(agent.to_string());
    }
}

struct Adapter {
    s: Shared<BluezState>,
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    #[zbus(property)]
    fn alias(&self) -> String {
        "ana-laptop".into()
    }
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.s.lock().unwrap().powered
    }
    #[zbus(property)]
    fn set_powered(&mut self, on: bool) -> fdo::Result<()> {
        let mut s = self.s.lock().unwrap();
        if on && s.blocked {
            return Err(fdo::Error::Failed("Blocked through rfkill".into()));
        }
        s.powered = on;
        Ok(())
    }
    #[zbus(property)]
    fn discoverable(&self) -> bool {
        self.s.lock().unwrap().discoverable
    }
    #[zbus(property)]
    fn set_discoverable(&mut self, on: bool) {
        self.s.lock().unwrap().discoverable = on;
    }
    #[zbus(property)]
    fn discovering(&self) -> bool {
        self.s.lock().unwrap().discovering
    }

    async fn start_discovery(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(), BluezError> {
        {
            let mut s = self.s.lock().unwrap();
            if !s.powered {
                return Err(BluezError::NotReady("Resource Not Ready".into()));
            }
            s.discovering = true;
            s.discovery_owner = hdr.sender().map(|s| s.to_string());
            s.devices.insert(
                SPEAKER.into(),
                FakeDevice {
                    name: Some("Kitchen speaker".into()),
                    icon: "audio-card".into(),
                    pairing: "pin",
                    ..Default::default()
                },
            );
        }
        add_device(server, &self.s, SPEAKER, false).await;
        Ok(())
    }

    fn stop_discovery(&self) {
        let mut s = self.s.lock().unwrap();
        s.discovering = false;
        s.discovery_owner = None;
    }

    async fn remove_device(
        &self,
        device: OwnedObjectPath,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(), BluezError> {
        if self
            .s
            .lock()
            .unwrap()
            .devices
            .remove(device.as_str())
            .is_none()
        {
            return Err(BluezError::DoesNotExist("Does Not Exist".into()));
        }
        let _ = server.remove::<Battery, _>(device.as_str()).await;
        let _ = server.remove::<Device, _>(device.as_str()).await;
        let _ = server.remove::<NamelessDevice, _>(device.as_str()).await;
        Ok(())
    }
}

struct Device {
    s: Shared<BluezState>,
    path: String,
}

impl Device {
    fn with<T: Default>(&self, f: impl FnOnce(&FakeDevice) -> T) -> T {
        self.s
            .lock()
            .unwrap()
            .devices
            .get(&self.path)
            .map(f)
            .unwrap_or_default()
    }
}

/// Ask the registered agent, as BlueZ does during pairing.
async fn ask_agent(
    s: &Shared<BluezState>,
    conn: &zbus::Connection,
    path: &str,
) -> Result<(), BluezError> {
    let (agent, how) = {
        let st = s.lock().unwrap();
        (
            st.agent.clone(),
            st.devices.get(path).map(|d| d.pairing).unwrap_or("confirm"),
        )
    };
    let Some((sender, agent_path, _)) = agent else {
        return Err(BluezError::AuthenticationFailed(
            "Authentication Failed".into(),
        ));
    };
    let device = OwnedObjectPath::try_from(path).unwrap();
    let call = |method: &'static str| {
        let sender = sender.clone();
        let agent_path = agent_path.clone();
        async move {
            match method {
                "RequestPinCode" => {
                    conn.call_method(
                        Some(sender.as_str()),
                        agent_path.as_str(),
                        Some("org.bluez.Agent1"),
                        method,
                        &(device.clone(),),
                    )
                    .await
                }
                _ => {
                    conn.call_method(
                        Some(sender.as_str()),
                        agent_path.as_str(),
                        Some("org.bluez.Agent1"),
                        method,
                        &(device.clone(), 123_456u32),
                    )
                    .await
                }
            }
        }
    };
    let method = if how == "pin" {
        "RequestPinCode"
    } else {
        "RequestConfirmation"
    };
    let r = call(method).await;
    let result = match &r {
        Ok(_) => "ok".to_string(),
        Err(zbus::Error::MethodError(name, _, _)) => name.to_string(),
        Err(e) => e.to_string(),
    };
    s.lock().unwrap().pair_results.push(result.clone());
    match r {
        Ok(reply) => {
            let mut st = s.lock().unwrap();
            let d = st.devices.get_mut(path).unwrap();
            if how == "pin" {
                d.pin = reply.body().deserialize::<String>().unwrap_or_default();
            }
            d.paired = true;
            Ok(())
        }
        Err(_) if result.ends_with("Canceled") => Err(BluezError::AuthenticationCanceled(
            "Authentication Canceled".into(),
        )),
        Err(_) => Err(BluezError::AuthenticationFailed(
            "Authentication Failed".into(),
        )),
    }
}

#[zbus::interface(name = "org.bluez.Device1")]
impl Device {
    #[zbus(property)]
    fn name(&self) -> String {
        self.with(|d| d.name.clone().unwrap_or_default())
    }
    #[zbus(property)]
    fn alias(&self) -> String {
        self.with(|d| d.name.clone().unwrap_or_default())
    }
    #[zbus(property)]
    fn icon(&self) -> String {
        self.with(|d| d.icon.clone())
    }
    #[zbus(property)]
    fn paired(&self) -> bool {
        self.with(|d| d.paired)
    }
    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.with(|d| d.trusted)
    }
    #[zbus(property)]
    fn set_trusted(&mut self, on: bool) {
        if let Some(d) = self.s.lock().unwrap().devices.get_mut(&self.path) {
            d.trusted = on;
        }
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        self.with(|d| d.connected)
    }
    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(ADAPTER).unwrap()
    }

    async fn pair(
        &self,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), BluezError> {
        if self.with(|d| d.paired) {
            return Err(BluezError::Failed("Already Exists".into()));
        }
        ask_agent(&self.s, conn, &self.path).await?;
        self.paired_changed(&emitter).await?;
        Ok(())
    }

    fn cancel_pairing(&self) {}

    async fn connect(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), BluezError> {
        if !self.with(|d| d.paired) {
            return Err(BluezError::Failed("Not paired".into()));
        }
        if let Some(d) = self.s.lock().unwrap().devices.get_mut(&self.path) {
            d.connected = true;
        }
        self.connected_changed(&emitter).await?;
        Ok(())
    }

    fn disconnect(&self) {
        if let Some(d) = self.s.lock().unwrap().devices.get_mut(&self.path) {
            d.connected = false;
        }
    }
}

/// A device that doesn't say its name: BlueZ leaves `Name` out.
struct NamelessDevice;

#[zbus::interface(name = "org.bluez.Device1")]
impl NamelessDevice {
    #[zbus(property)]
    fn alias(&self) -> String {
        "AA-BB-CC-DD-EE-03".into()
    }
    #[zbus(property)]
    fn paired(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(ADAPTER).unwrap()
    }
}

struct Battery;

#[zbus::interface(name = "org.bluez.Battery1")]
impl Battery {
    #[zbus(property)]
    fn percentage(&self) -> u8 {
        80
    }
}

async fn add_device(server: &ObjectServer, s: &Shared<BluezState>, path: &str, battery: bool) {
    server
        .at(
            path,
            Device {
                s: s.clone(),
                path: path.to_string(),
            },
        )
        .await
        .unwrap();
    if battery {
        server.at(path, Battery).await.unwrap();
    }
}

pub struct FakeBluez {
    pub state: Shared<BluezState>,
}

impl FakeBluez {
    pub async fn start(rig: &mut SysRig) -> Self {
        let state: Shared<BluezState> = Arc::new(Mutex::new(BluezState {
            powered: true,
            devices: HashMap::from([
                (
                    HEADPHONES.to_string(),
                    FakeDevice {
                        name: Some("Ana's headphones".into()),
                        icon: "audio-headset".into(),
                        paired: true,
                        trusted: true,
                        connected: true,
                        pairing: "confirm",
                        ..Default::default()
                    },
                ),
                (
                    KEYBOARD.to_string(),
                    FakeDevice {
                        name: Some("<b>Keyboard</b>".into()),
                        icon: "input-keyboard".into(),
                        pairing: "confirm",
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        }));
        let conn = rig.own("org.bluez").await;
        let os = conn.object_server();
        os.at("/", fdo::ObjectManager).await.unwrap();
        os.at("/org/bluez", AgentManager { s: state.clone() })
            .await
            .unwrap();
        os.at(ADAPTER, Adapter { s: state.clone() }).await.unwrap();
        add_device(os, &state, HEADPHONES, true).await;
        add_device(os, &state, KEYBOARD, false).await;
        os.at(NAMELESS, NamelessDevice).await.unwrap();
        Self { state }
    }
}
