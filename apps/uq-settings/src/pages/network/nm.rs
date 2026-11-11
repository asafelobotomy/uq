//! NetworkManager on the system bus: its names, the proxies the Network page calls, and
//! reading an object's properties in one `GetAll`.

use std::collections::HashMap;

use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

pub const NM: &str = "org.freedesktop.NetworkManager";
pub const NM_PATH: &str = "/org/freedesktop/NetworkManager";
/// Where NetworkManager looks for a secret agent's object.
pub const AGENT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
/// The name this app registers its secret agent under.
pub const AGENT_ID: &str = "org.uq.Settings.network";

pub const IFACE_NM: &str = "org.freedesktop.NetworkManager";
pub const IFACE_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
pub const IFACE_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
pub const IFACE_WIRED: &str = "org.freedesktop.NetworkManager.Device.Wired";
pub const IFACE_AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
pub const IFACE_ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";

/// NMDeviceType values the page shows.
pub const DEVICE_ETHERNET: u32 = 1;
pub const DEVICE_WIFI: u32 = 2;
/// NMDeviceState values.
pub const STATE_UNAVAILABLE: u32 = 20;
pub const STATE_ACTIVATED: u32 = 100;
pub const STATE_FAILED: u32 = 120;
/// NM_WIFI_DEVICE_CAP_AP: the Wi-Fi card can be a hotspot.
pub const WIFI_CAP_AP: u32 = 0x40;
/// NMActiveConnectionState.
pub const ACTIVE_ACTIVATING: u32 = 1;
pub const ACTIVE_ACTIVATED: u32 = 2;

pub type Props = HashMap<String, OwnedValue>;
/// A connection's settings, `a{sa{sv}}`.
pub type Settings = HashMap<String, HashMap<String, OwnedValue>>;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
pub trait NetworkManager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(allow_interactive_auth)]
    fn activate_connection(
        &self,
        connection: &OwnedObjectPath,
        device: &OwnedObjectPath,
        specific_object: &OwnedObjectPath,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn add_and_activate_connection(
        &self,
        connection: &Settings,
        device: &OwnedObjectPath,
        specific_object: &OwnedObjectPath,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
    #[zbus(allow_interactive_auth)]
    fn deactivate_connection(&self, active: &OwnedObjectPath) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
pub trait NmSettings {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(allow_interactive_auth)]
    fn add_connection(&self, connection: &Settings) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NmConnection {
    /// The settings without their secrets.
    fn get_settings(&self) -> zbus::Result<Settings>;
    /// Replaces the settings; secrets left out are kept by NetworkManager.
    #[zbus(allow_interactive_auth)]
    fn update(&self, properties: &Settings) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn delete(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NmWireless {
    fn request_scan(&self, options: HashMap<&str, zbus::zvariant::Value<'_>>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AgentManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/AgentManager"
)]
pub trait AgentManager {
    fn register_with_capabilities(&self, identifier: &str, capabilities: u32) -> zbus::Result<()>;
    fn unregister(&self) -> zbus::Result<()>;
}

/// A proxy at `path` that reads properties afresh.
pub async fn at<'a, P>(conn: &zbus::Connection, path: &OwnedObjectPath) -> zbus::Result<P>
where
    P: From<zbus::Proxy<'a>> + zbus::proxy::Defaults,
{
    zbus::proxy::Builder::<P>::new(conn)
        .path(path.clone())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// Every property of `iface` on NetworkManager's object at `path`.
pub async fn get_all(conn: &zbus::Connection, path: &str, iface: &str) -> zbus::Result<Props> {
    let p = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(NM)?
        .path(path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let iface = zbus::names::InterfaceName::try_from(iface)?;
    Ok(p.get_all(iface).await?)
}

/// `props[key]` as `T`, when it is there and has that type.
pub fn get<T>(props: &HashMap<String, OwnedValue>, key: &str) -> Option<T>
where
    T: TryFrom<OwnedValue>,
{
    T::try_from(props.get(key)?.try_clone().ok()?).ok()
}

pub fn text(props: &HashMap<String, OwnedValue>, key: &str) -> String {
    get::<String>(props, key).unwrap_or_default()
}

pub fn num(props: &HashMap<String, OwnedValue>, key: &str) -> u32 {
    get::<u32>(props, key).unwrap_or(0)
}

pub fn flag(props: &HashMap<String, OwnedValue>, key: &str) -> bool {
    get::<bool>(props, key).unwrap_or(false)
}

pub fn path(props: &HashMap<String, OwnedValue>, key: &str) -> OwnedObjectPath {
    get::<OwnedObjectPath>(props, key).unwrap_or_else(root)
}

pub fn paths(props: &HashMap<String, OwnedValue>, key: &str) -> Vec<OwnedObjectPath> {
    get::<Vec<OwnedObjectPath>>(props, key).unwrap_or_default()
}

/// `/`, NetworkManager's "none" for an object path.
pub fn root() -> OwnedObjectPath {
    OwnedObjectPath::try_from("/").expect("/ is a path")
}

/// A Wi-Fi network's name as people read it: UTF-8 when it is, else with the odd bytes
/// replaced. An empty name is a hidden network.
pub fn ssid_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
