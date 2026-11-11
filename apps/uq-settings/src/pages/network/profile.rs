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
use zbus::zvariant::{OwnedValue, Value};

use super::nm::{Settings, get, text};

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

/// How a Wi-Fi network is protected, from an access point's flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Open,
    /// Open but encrypted (OWE, "Enhanced Open").
    Owe,
    /// An old WEP key.
    Wep,
    /// A shared password: `wpa-psk`, or `sae` (WPA3) when the network takes nothing else.
    Password {
        sae_only: bool,
    },
    /// A user name and password or a certificate (802.1x).
    Enterprise,
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B: u32 = 0x2000;

pub fn security(flags: u32, wpa: u32, rsn: u32) -> Security {
    let both = wpa | rsn;
    if both & (KEY_MGMT_8021X | KEY_MGMT_EAP_SUITE_B) != 0 {
        Security::Enterprise
    } else if both & KEY_MGMT_PSK != 0 {
        Security::Password { sae_only: false }
    } else if both & KEY_MGMT_SAE != 0 {
        Security::Password { sae_only: true }
    } else if both & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 {
        Security::Owe
    } else if flags & AP_PRIVACY != 0 {
        Security::Wep
    } else {
        Security::Open
    }
}

impl Security {
    /// The page's word for it.
    pub fn id(self) -> &'static str {
        match self {
            Security::Open => "open",
            Security::Owe => "encrypted",
            Security::Wep => "old",
            Security::Password { .. } => "password",
            Security::Enterprise => "enterprise",
        }
    }
}

/// What an 802.1x (work or school) network asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Eap {
    /// `peap`, `ttls` or `tls`.
    pub method: String,
    pub identity: String,
    pub anonymous_identity: String,
    /// PEAP and TTLS: the password (a secret).
    pub password: String,
    /// PEAP and TTLS: the inner method (`mschapv2`, `pap`, `gtc`, `mschap`, `chap`).
    pub phase2: String,
    /// The certificate authority's file; empty uses the system's certificates.
    pub ca_cert: String,
    /// Only accept a server whose name ends with this (recommended with the system's
    /// certificates).
    pub domain: String,
    /// TLS: the client certificate and private key files, and the key's password (a secret).
    pub client_cert: String,
    pub private_key: String,
    pub key_password: String,
}

pub const EAP_METHODS: [&str; 3] = ["peap", "ttls", "tls"];
pub const PHASE2: [&str; 5] = ["mschapv2", "pap", "gtc", "mschap", "chap"];

/// A file for NetworkManager's certificate settings: `file://<path>` and a NUL, as bytes.
fn cert_blob(path: &str) -> Result<Vec<u8>, String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("{path} isn't a full file path."));
    }
    if !p.is_file() {
        return Err(format!("{path} doesn't exist or isn't a file."));
    }
    let mut out = format!("file://{path}").into_bytes();
    out.push(0);
    Ok(out)
}

fn eap_setting(eap: &Eap) -> Result<HashMap<String, OwnedValue>, String> {
    if !EAP_METHODS.contains(&eap.method.as_str()) {
        return Err("Choose how this network checks who you are.".into());
    }
    if eap.identity.trim().is_empty() {
        return Err("Type your user name for this network.".into());
    }
    let mut s = HashMap::from([
        ("eap".to_string(), ov(vec![eap.method.as_str()])),
        ("identity".to_string(), ov(eap.identity.trim())),
    ]);
    if !eap.anonymous_identity.trim().is_empty() {
        s.insert(
            "anonymous-identity".into(),
            ov(eap.anonymous_identity.trim()),
        );
    }
    if eap.ca_cert.is_empty() {
        s.insert("system-ca-certs".into(), ov(true));
    } else {
        s.insert("ca-cert".into(), ov(cert_blob(&eap.ca_cert)?));
    }
    if !eap.domain.trim().is_empty() {
        s.insert("domain-suffix-match".into(), ov(eap.domain.trim()));
    }
    if eap.method == "tls" {
        if eap.client_cert.is_empty() || eap.private_key.is_empty() {
            return Err("Choose your certificate and private key files.".into());
        }
        s.insert("client-cert".into(), ov(cert_blob(&eap.client_cert)?));
        s.insert("private-key".into(), ov(cert_blob(&eap.private_key)?));
        s.insert("private-key-password".into(), ov(eap.key_password.as_str()));
        s.insert("private-key-password-flags".into(), ov(0u32));
    } else {
        let phase2 = if eap.phase2.is_empty() {
            "mschapv2"
        } else {
            eap.phase2.as_str()
        };
        if !PHASE2.contains(&phase2) {
            return Err("Choose an inner sign-in method this page offers.".into());
        }
        s.insert("phase2-auth".into(), ov(phase2));
        if !eap.password.is_empty() {
            s.insert("password".into(), ov(eap.password.as_str()));
        }
        s.insert("password-flags".into(), ov(0u32));
    }
    Ok(s)
}

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

/// The IP settings of one family as the page edits them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IpInput {
    /// `auto`, `manual` or `disabled`.
    pub method: String,
    /// `address/prefix`, comma-separated.
    pub addresses: String,
    pub gateway: String,
    /// Comma-separated; when set, only these are used.
    pub dns: String,
}

pub const IP_METHODS: [&str; 3] = ["auto", "manual", "disabled"];

fn split_list(s: &str) -> impl Iterator<Item = &str> {
    s.split([',', ' ', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
}

fn parse_address(a: &str, v6: bool) -> Result<(IpAddr, u32), String> {
    let (ip, prefix) = a
        .split_once('/')
        .unwrap_or((a, if v6 { "64" } else { "24" }));
    let ip: IpAddr = ip
        .parse()
        .map_err(|_| format!("{a} isn't an address this page understands."))?;
    if ip.is_ipv6() != v6 {
        return Err(format!(
            "{a} is an {} address.",
            if v6 { "IPv4" } else { "IPv6" }
        ));
    }
    let prefix: u32 = prefix
        .parse()
        .ok()
        .filter(|p| *p <= if v6 { 128 } else { 32 })
        .ok_or_else(|| format!("{a} has a network size this page doesn't understand."))?;
    Ok((ip, prefix))
}

/// Change the `ipv4` (`family` 4) or `ipv6` (6) settings. Legacy keys NetworkManager would
/// prefer over the new ones are taken out.
pub fn apply_ip(s: &mut Settings, family: u8, ip: &IpInput) -> Result<(), String> {
    let v6 = match family {
        4 => false,
        6 => true,
        _ => return Err("Only IPv4 and IPv6 can be set.".into()),
    };
    if !IP_METHODS.contains(&ip.method.as_str()) {
        return Err("Choose Automatic, Manual or Off.".into());
    }
    let addresses: Vec<(IpAddr, u32)> = split_list(&ip.addresses)
        .map(|a| parse_address(a, v6))
        .collect::<Result<_, _>>()?;
    let dns: Vec<IpAddr> = split_list(&ip.dns)
        .map(|d| {
            d.parse::<IpAddr>()
                .ok()
                .filter(|a| a.is_ipv6() == v6)
                .ok_or_else(|| format!("{d} isn't a DNS server address for this kind of network."))
        })
        .collect::<Result<_, _>>()?;
    let gateway = ip.gateway.trim();
    if !gateway.is_empty()
        && gateway
            .parse::<IpAddr>()
            .map_or(true, |g| g.is_ipv6() != v6)
    {
        return Err(format!(
            "{gateway} isn't a gateway address for this kind of network."
        ));
    }
    if ip.method == "manual" && addresses.is_empty() {
        return Err("Manual needs at least one address.".into());
    }
    let key = if v6 { "ipv6" } else { "ipv4" };
    let sec = s.entry(key.into()).or_default();
    for legacy in ["addresses", "dns-data", "address-data", "gateway"] {
        sec.remove(legacy);
    }
    sec.insert("method".into(), ov(ip.method.as_str()));
    if ip.method == "manual" {
        let data: Vec<HashMap<&str, Value>> = addresses
            .iter()
            .map(|(a, p)| {
                HashMap::from([
                    ("address", Value::from(a.to_string())),
                    ("prefix", Value::from(*p)),
                ])
            })
            .collect();
        sec.insert("address-data".into(), ov(data));
        if !gateway.is_empty() {
            sec.insert("gateway".into(), ov(gateway));
        }
    }
    if v6 {
        let raw: Vec<Vec<u8>> = dns
            .iter()
            .filter_map(|a| match a {
                IpAddr::V6(a) => Some(a.octets().to_vec()),
                IpAddr::V4(_) => None,
            })
            .collect();
        sec.insert("dns".into(), ov(raw));
    } else {
        // NetworkManager reads each one as an address in network byte order.
        let raw: Vec<u32> = dns
            .iter()
            .filter_map(|a| match a {
                IpAddr::V4(a) => Some(u32::from_ne_bytes(a.octets())),
                IpAddr::V6(_) => None,
            })
            .collect();
        sec.insert("dns".into(), ov(raw));
    }
    sec.insert("ignore-auto-dns".into(), ov(!dns.is_empty()));
    Ok(())
}

/// The DNS servers a settings block lists, as text.
fn dns_list(sec: &HashMap<String, OwnedValue>, v6: bool) -> Vec<String> {
    if let Some(data) = get::<Vec<String>>(sec, "dns-data") {
        return data;
    }
    if v6 {
        get::<Vec<Vec<u8>>>(sec, "dns")
            .unwrap_or_default()
            .iter()
            .filter_map(|b| <[u8; 16]>::try_from(b.as_slice()).ok())
            .map(|b| Ipv6Addr::from(b).to_string())
            .collect()
    } else {
        get::<Vec<u32>>(sec, "dns")
            .unwrap_or_default()
            .iter()
            .map(|n| Ipv4Addr::from(n.to_ne_bytes()).to_string())
            .collect()
    }
}

fn addresses(sec: &HashMap<String, OwnedValue>) -> Vec<String> {
    get::<Vec<HashMap<String, OwnedValue>>>(sec, "address-data")
        .unwrap_or_default()
        .iter()
        .map(|d| {
            format!(
                "{}/{}",
                text(d, "address"),
                get::<u32>(d, "prefix").unwrap_or(0)
            )
        })
        .collect()
}

fn ip_json(s: &Settings, v6: bool) -> Json {
    let empty = HashMap::new();
    let sec = s.get(if v6 { "ipv6" } else { "ipv4" }).unwrap_or(&empty);
    let method = text(sec, "method");
    let shown = match method.as_str() {
        "manual" => "manual",
        "disabled" | "ignore" => "disabled",
        "" | "auto" | "dhcp" => "auto",
        _ => "other",
    };
    json!({
        "method": shown,
        "raw_method": method,
        "addresses": addresses(sec).join(", "),
        "gateway": text(sec, "gateway"),
        "dns": dns_list(sec, v6).join(", "),
    })
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
mod tests {
    use super::*;

    #[test]
    fn security_from_flags() {
        assert_eq!(security(0, 0, 0), Security::Open);
        assert_eq!(security(1, 0, 0), Security::Wep);
        assert_eq!(
            security(1, 0, KEY_MGMT_PSK),
            Security::Password { sae_only: false }
        );
        assert_eq!(
            security(1, 0, KEY_MGMT_PSK | KEY_MGMT_SAE),
            Security::Password { sae_only: false },
            "WPA2/WPA3 mixed joins with a WPA2 password"
        );
        assert_eq!(
            security(1, 0, KEY_MGMT_SAE),
            Security::Password { sae_only: true }
        );
        assert_eq!(
            security(1, KEY_MGMT_8021X, KEY_MGMT_8021X),
            Security::Enterprise
        );
        assert_eq!(security(0, 0, KEY_MGMT_OWE), Security::Owe);
    }

    #[test]
    fn uuids_are_version_4_and_differ() {
        let a = new_uuid();
        let b = new_uuid();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn a_new_wifi_network_is_private_by_default() {
        let w = WifiInput {
            ssid: b"Cafe".to_vec(),
            security: Some(Security::Password { sae_only: false }),
            password: "correct horse".into(),
            ..Default::default()
        };
        let s = wifi_settings("u-1", "ana", &w).unwrap();
        assert_eq!(kind(&s), "wifi");
        assert_eq!(
            get::<Vec<String>>(&s["connection"], "permissions").unwrap(),
            ["user:ana:"]
        );
        assert_eq!(text(&s["802-11-wireless-security"], "key-mgmt"), "wpa-psk");
        assert_eq!(text(&s["802-11-wireless-security"], "psk"), "correct horse");
        assert_eq!(ssid(&s), b"Cafe");

        let everyone = wifi_settings(
            "u-2",
            "ana",
            &WifiInput {
                all_users: true,
                ..w.clone()
            },
        )
        .unwrap();
        assert!(
            get::<Vec<String>>(&everyone["connection"], "permissions")
                .unwrap()
                .is_empty()
        );

        let short = WifiInput {
            password: "short".into(),
            ..w
        };
        assert!(
            wifi_settings("u", "ana", &short)
                .unwrap_err()
                .contains("8 to 63")
        );
    }

    #[test]
    fn enterprise_networks_need_their_details() {
        let mut w = WifiInput {
            ssid: b"eduroam".to_vec(),
            security: Some(Security::Enterprise),
            ..Default::default()
        };
        assert!(wifi_settings("u", "ana", &w).is_err());
        w.eap = Some(Eap {
            method: "peap".into(),
            identity: "ana@uni.example".into(),
            password: "pw".into(),
            domain: "radius.uni.example".into(),
            ..Default::default()
        });
        let s = wifi_settings("u", "ana", &w).unwrap();
        let x = &s["802-1x"];
        assert_eq!(get::<Vec<String>>(x, "eap").unwrap(), ["peap"]);
        assert_eq!(text(x, "phase2-auth"), "mschapv2");
        assert_eq!(text(x, "password"), "pw");
        assert_eq!(get::<bool>(x, "system-ca-certs"), Some(true));
        assert_eq!(text(x, "domain-suffix-match"), "radius.uni.example");
        assert_eq!(text(&s["802-11-wireless-security"], "key-mgmt"), "wpa-eap");

        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("me.pem");
        std::fs::write(&cert, "x").unwrap();
        let cert = cert.display().to_string();
        w.eap = Some(Eap {
            method: "tls".into(),
            identity: "ana".into(),
            ca_cert: cert.clone(),
            client_cert: cert.clone(),
            private_key: cert.clone(),
            key_password: "kp".into(),
            ..Default::default()
        });
        let s = wifi_settings("u", "ana", &w).unwrap();
        let blob = get::<Vec<u8>>(&s["802-1x"], "client-cert").unwrap();
        assert_eq!(blob, format!("file://{cert}\0").into_bytes());
        assert_eq!(text(&s["802-1x"], "private-key-password"), "kp");
        w.eap.as_mut().unwrap().private_key = "relative.pem".into();
        assert!(
            wifi_settings("u", "ana", &w)
                .unwrap_err()
                .contains("full file path")
        );
    }

    #[test]
    fn hotspot_checks_its_password() {
        let s = hotspot_settings("u", "ana", "uQ laptop", "longenough").unwrap();
        assert_eq!(kind(&s), "hotspot");
        assert_eq!(text(&s["ipv4"], "method"), "shared");
        assert!(hotspot_settings("u", "ana", "x", "short").is_err());
        assert!(hotspot_settings("u", "ana", " ", "longenough").is_err());
    }

    #[test]
    fn ip_settings_round_trip_through_the_summary() {
        let mut s = wifi_settings(
            "u",
            "ana",
            &WifiInput {
                ssid: b"Home".to_vec(),
                ..Default::default()
            },
        )
        .unwrap();
        s.get_mut("ipv4")
            .unwrap()
            .insert("addresses".into(), ov(vec![vec![1u32, 24, 0]]));
        apply_ip(
            &mut s,
            4,
            &IpInput {
                method: "manual".into(),
                addresses: "192.168.1.20/24, 10.0.0.2".into(),
                gateway: "192.168.1.1".into(),
                dns: "1.1.1.1 9.9.9.9".into(),
            },
        )
        .unwrap();
        assert!(
            !s["ipv4"].contains_key("addresses"),
            "the legacy key would win"
        );
        apply_ip(
            &mut s,
            6,
            &IpInput {
                method: "auto".into(),
                dns: "2606:4700:4700::1111".into(),
                ..Default::default()
            },
        )
        .unwrap();
        set_metered(&mut s, "yes").unwrap();
        let j = summary("/x", &s);
        assert_eq!(j["ipv4"]["method"], "manual");
        assert_eq!(j["ipv4"]["addresses"], "192.168.1.20/24, 10.0.0.2/24");
        assert_eq!(j["ipv4"]["gateway"], "192.168.1.1");
        assert_eq!(j["ipv4"]["dns"], "1.1.1.1, 9.9.9.9");
        assert_eq!(j["ipv6"]["method"], "auto");
        assert_eq!(j["ipv6"]["dns"], "2606:4700:4700::1111");
        assert_eq!(j["metered"], "yes");
        assert_eq!(j["everyone"], false);
        assert_eq!(get::<bool>(&s["ipv4"], "ignore-auto-dns"), Some(true));

        let bad = |m: &str, a: &str, d: &str| {
            apply_ip(
                &mut s.clone(),
                4,
                &IpInput {
                    method: m.into(),
                    addresses: a.into(),
                    dns: d.into(),
                    ..Default::default()
                },
            )
            .unwrap_err()
        };
        assert!(bad("manual", "", "").contains("at least one"));
        assert!(bad("manual", "300.1.1.1/24", "").contains("isn't an address"));
        assert!(bad("manual", "fe80::1/64", "").contains("IPv6 address"));
        assert!(bad("manual", "10.0.0.1/40", "").contains("network size"));
        assert!(bad("auto", "", "fe80::1").contains("DNS server"));
        assert!(bad("dhcp", "", "").contains("Choose"));
        assert!(set_metered(&mut s, "sometimes").is_err());
    }
}
