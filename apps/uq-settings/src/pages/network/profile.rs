//! Connection settings as NetworkManager stores them (`a{sa{sv}}`): building new ones (a Wi-Fi
//! network, a hotspot, a work or school network), changing the parts the page edits (IP
//! addresses, DNS, metered), and summing a saved one up for the page. Pure functions, tested
//! on their own.
//!
//! Secrets typed on the page (a Wi-Fi password, an 802.1x password, a hotspot password) go into
//! these settings and straight to NetworkManager, which stores them; their flags are 0
//! ("NetworkManager keeps it"). uQ never writes them anywhere. A new connection is private to
//! the person who adds it (`connection.permissions = ["user:<name>:"]`) unless they choose
//! "available to all users".

use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use serde_json::{Value as Json, json};
use zbus::wire::{OwnedVariant as OwnedValue, Variant as Value};

use super::nm::{Settings, get, text};
mod ip;
pub use ip::*;
mod security;
pub use security::*;

/// A value for a settings dict.
pub fn ov<'a, T: Into<Value<'a>>>(v: T) -> OwnedValue {
    OwnedValue::try_from(v.into()).expect("settings values hold no file descriptors")
}

/// A random (version 4) UUID for a new connection.
pub fn new_uuid() -> String {
    let mut b = [0u8; 16];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_err()
    {
        // No /dev/urandom: the time and the process id still make it unique on this machine.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        b[..16].copy_from_slice(&(t ^ u128::from(std::process::id()) << 64).to_le_bytes());
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// `connection.permissions`: only `user` can see and use it, unless it is for everyone.
pub fn permissions(user: &str, all_users: bool) -> Vec<String> {
    if all_users || user.is_empty() {
        Vec::new()
    } else {
        vec![format!("user:{user}:")]
    }
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B: u32 = 0x2000;

pub const EAP_METHODS: [&str; 3] = ["peap", "ttls", "tls"];
pub const PHASE2: [&str; 5] = ["mschapv2", "pap", "gtc", "mschap", "chap"];

fn connection_setting(
    id: &str,
    uuid: &str,
    kind: &str,
    user: &str,
    all_users: bool,
    autoconnect: bool,
) -> HashMap<String, OwnedValue> {
    HashMap::from([
        ("id".to_string(), ov(id)),
        ("uuid".to_string(), ov(uuid)),
        ("type".to_string(), ov(kind)),
        ("autoconnect".to_string(), ov(autoconnect)),
        ("permissions".to_string(), ov(permissions(user, all_users))),
    ])
}

/// What the page sends to join a Wi-Fi network it hasn't saved.
#[derive(Debug, Clone, Default)]
pub struct WifiInput {
    pub ssid: Vec<u8>,
    pub security: Option<Security>,
    pub password: String,
    pub all_users: bool,
    pub eap: Option<Eap>,
}

/// Settings for joining a new Wi-Fi network.
pub fn wifi_settings(uuid: &str, user: &str, w: &WifiInput) -> Result<Settings, String> {
    if w.ssid.is_empty() || w.ssid.len() > 32 {
        return Err("A network name is 1 to 32 characters.".into());
    }
    let name = super::nm::ssid_text(&w.ssid);
    let mut s = Settings::from([
        (
            "connection".to_string(),
            connection_setting(&name, uuid, "802-11-wireless", user, w.all_users, true),
        ),
        (
            "802-11-wireless".to_string(),
            HashMap::from([
                ("ssid".to_string(), ov(w.ssid.clone())),
                ("mode".to_string(), ov("infrastructure")),
            ]),
        ),
        (
            "ipv4".to_string(),
            HashMap::from([("method".to_string(), ov("auto"))]),
        ),
        (
            "ipv6".to_string(),
            HashMap::from([("method".to_string(), ov("auto"))]),
        ),
    ]);
    let sec = match w.security.unwrap_or(Security::Open) {
        Security::Open => None,
        Security::Owe => Some(HashMap::from([("key-mgmt".to_string(), ov("owe"))])),
        Security::Wep => {
            if w.password.is_empty() {
                return Err("Type the network's key.".into());
            }
            Some(HashMap::from([
                ("key-mgmt".to_string(), ov("none")),
                ("wep-key0".to_string(), ov(w.password.as_str())),
                ("wep-key-type".to_string(), ov(1u32)),
                ("wep-key-flags".to_string(), ov(0u32)),
            ]))
        }
        Security::Password { sae_only } => {
            let n = w.password.chars().count();
            if !(8..=63).contains(&n)
                && !(n == 64 && w.password.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Err("A Wi-Fi password is 8 to 63 characters.".into());
            }
            Some(HashMap::from([
                (
                    "key-mgmt".to_string(),
                    ov(if sae_only { "sae" } else { "wpa-psk" }),
                ),
                ("psk".to_string(), ov(w.password.as_str())),
                ("psk-flags".to_string(), ov(0u32)),
            ]))
        }
        Security::Enterprise => {
            let eap = w
                .eap
                .as_ref()
                .ok_or("This network needs a user name and password or a certificate.")?;
            s.insert("802-1x".into(), eap_setting(eap)?);
            Some(HashMap::from([("key-mgmt".to_string(), ov("wpa-eap"))]))
        }
    };
    if let Some(sec) = sec {
        s.insert("802-11-wireless-security".into(), sec);
    }
    Ok(s)
}

/// Settings for sharing this computer's connection over Wi-Fi.
pub fn hotspot_settings(
    uuid: &str,
    user: &str,
    name: &str,
    password: &str,
) -> Result<Settings, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 32 {
        return Err("A hotspot name is 1 to 32 characters.".into());
    }
    if !(8..=63).contains(&password.chars().count()) {
        return Err("A hotspot password is 8 to 63 characters.".into());
    }
    Ok(Settings::from([
        (
            "connection".to_string(),
            connection_setting("Hotspot", uuid, "802-11-wireless", user, false, false),
        ),
        (
            "802-11-wireless".to_string(),
            HashMap::from([
                ("ssid".to_string(), ov(name.as_bytes().to_vec())),
                ("mode".to_string(), ov("ap")),
            ]),
        ),
        (
            "802-11-wireless-security".to_string(),
            HashMap::from([
                ("key-mgmt".to_string(), ov("wpa-psk")),
                ("psk".to_string(), ov(password)),
                ("psk-flags".to_string(), ov(0u32)),
                ("proto".to_string(), ov(vec!["rsn"])),
                ("pairwise".to_string(), ov(vec!["ccmp"])),
                ("group".to_string(), ov(vec!["ccmp"])),
            ]),
        ),
        (
            "ipv4".to_string(),
            HashMap::from([("method".to_string(), ov("shared"))]),
        ),
        (
            "ipv6".to_string(),
            HashMap::from([("method".to_string(), ov("ignore"))]),
        ),
    ]))
}

/// The kind of a saved connection, as the page groups them.
pub fn kind(s: &Settings) -> &'static str {
    let ty = s
        .get("connection")
        .map(|c| text(c, "type"))
        .unwrap_or_default();
    match ty.as_str() {
        "802-11-wireless" => {
            let mode = s
                .get("802-11-wireless")
                .map(|w| text(w, "mode"))
                .unwrap_or_default();
            if mode == "ap" { "hotspot" } else { "wifi" }
        }
        "802-3-ethernet" => "wired",
        "wireguard" => "wireguard",
        "vpn" => "vpn",
        "loopback" => "loopback",
        _ => "other",
    }
}

pub fn uuid(s: &Settings) -> String {
    s.get("connection")
        .map(|c| text(c, "uuid"))
        .unwrap_or_default()
}

pub fn name(s: &Settings) -> String {
    s.get("connection")
        .map(|c| text(c, "id"))
        .unwrap_or_default()
}

/// The saved Wi-Fi network's name, as bytes.
pub fn ssid(s: &Settings) -> Vec<u8> {
    s.get("802-11-wireless")
        .and_then(|w| get::<Vec<u8>>(w, "ssid"))
        .unwrap_or_default()
}

fn metered_text(n: i32) -> &'static str {
    match n {
        1 | 3 => "yes",
        2 | 4 => "no",
        _ => "auto",
    }
}

/// Set `connection.metered`.
pub fn set_metered(s: &mut Settings, choice: &str) -> Result<(), String> {
    let n: i32 = match choice {
        "auto" => 0,
        "yes" => 1,
        "no" => 2,
        _ => return Err("Choose Automatic, Yes or No.".into()),
    };
    s.entry("connection".into())
        .or_default()
        .insert("metered".into(), ov(n));
    Ok(())
}

pub fn set_autoconnect(s: &mut Settings, on: bool) {
    s.entry("connection".into())
        .or_default()
        .insert("autoconnect".into(), ov(on));
}

pub const IP_METHODS: [&str; 3] = ["auto", "manual", "disabled"];

fn split_list(s: &str) -> impl Iterator<Item = &str> {
    s.split([',', ' ', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
}

fn wireguard_json(s: &Settings) -> Json {
    let empty = HashMap::new();
    let wg = s.get("wireguard").unwrap_or(&empty);
    let peers = get::<Vec<HashMap<String, OwnedValue>>>(wg, "peers").unwrap_or_default();
    let peer = peers.first();
    json!({
        "endpoint": peer.map(|p| text(p, "endpoint")).unwrap_or_default(),
        "allowed_ips": peer
            .and_then(|p| get::<Vec<String>>(p, "allowed-ips"))
            .unwrap_or_default()
            .join(", "),
        "keepalive": peer.and_then(|p| get::<u32>(p, "persistent-keepalive")).unwrap_or(0),
        "peers": peers.len(),
    })
}

/// A saved connection as the page lists it. Secrets are never part of it.
pub fn summary(path: &str, s: &Settings) -> Json {
    let empty = HashMap::new();
    let c = s.get("connection").unwrap_or(&empty);
    let k = kind(s);
    let perms = get::<Vec<String>>(c, "permissions").unwrap_or_default();
    let vpn_service = s
        .get("vpn")
        .map(|v| text(v, "service-type"))
        .unwrap_or_default();
    let eap = s
        .get("802-1x")
        .and_then(|x| get::<Vec<String>>(x, "eap"))
        .and_then(|m| m.into_iter().next())
        .unwrap_or_default();
    json!({
        "path": path,
        "uuid": text(c, "uuid"),
        "name": text(c, "id"),
        "kind": k,
        "ssid": if k == "wifi" || k == "hotspot" { json!(super::nm::ssid_text(&ssid(s))) } else { Json::Null },
        "autoconnect": get::<bool>(c, "autoconnect").unwrap_or(true),
        "metered": metered_text(get::<i32>(c, "metered").unwrap_or(0)),
        "everyone": perms.is_empty(),
        "ipv4": ip_json(s, false),
        "ipv6": ip_json(s, true),
        "vpn_service": vpn_service.rsplit('.').next().unwrap_or_default(),
        "enterprise": eap,
        "wireguard": if k == "wireguard" { wireguard_json(s) } else { Json::Null },
    })
}

#[cfg(test)]
mod tests;
