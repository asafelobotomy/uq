//! WireGuard: turning a `wg-quick` config file into NetworkManager settings (import), and
//! changing a saved one's peer (edit). The private and preshared keys go into the settings
//! and straight to NetworkManager; they are never shown or written anywhere by uQ.
//!
//! `PreUp`/`PostUp`/`PreDown`/`PostDown` and `Table`/`SaveConfig` lines are `wg-quick`'s own and
//! have no NetworkManager equivalent: the import lists them as left out instead of failing.

use std::collections::HashMap;
use std::net::IpAddr;

use zbus::zvariant::{OwnedValue, Value};

use super::nm::{Settings, get, text};
use super::profile::{IpInput, apply_ip, ov, permissions};

/// One `[Peer]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peer {
    pub public_key: String,
    pub preshared_key: String,
    pub allowed_ips: Vec<String>,
    pub endpoint: String,
    pub keepalive: u32,
}

/// A parsed config file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub private_key: String,
    pub addresses: Vec<String>,
    pub dns: Vec<String>,
    pub listen_port: u32,
    pub mtu: u32,
    pub peers: Vec<Peer>,
    /// Lines NetworkManager can't do (`PostUp` …), by key.
    pub left_out: Vec<String>,
}

/// Whether `k` looks like a WireGuard key: 32 bytes in base64.
pub fn is_key(k: &str) -> bool {
    k.len() == 44
        && k.ends_with('=')
        && k[..43]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
}

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse a `wg-quick` file. Errors name the line.
pub fn parse(text: &str) -> Result<Config, String> {
    #[derive(PartialEq)]
    enum Section {
        None,
        Interface,
        Peer,
    }
    let mut c = Config::default();
    let mut section = Section::None;
    let mut seen_interface = false;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let at = n + 1;
        if line.eq_ignore_ascii_case("[interface]") {
            section = Section::Interface;
            seen_interface = true;
            continue;
        }
        if line.eq_ignore_ascii_case("[peer]") {
            section = Section::Peer;
            c.peers.push(Peer::default());
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("Line {at} isn't \"name = value\"."));
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        let bad = |what: &str| format!("Line {at}: {what}");
        match (&section, key.as_str()) {
            (Section::Interface, "privatekey") => {
                if !is_key(value) {
                    return Err(bad("the private key isn't a WireGuard key."));
                }
                c.private_key = value.to_string();
            }
            (Section::Interface, "address") => c.addresses.extend(list(value)),
            (Section::Interface, "dns") => c.dns.extend(list(value)),
            (Section::Interface, "listenport") => {
                c.listen_port = value.parse().map_err(|_| bad("the port isn't a number."))?;
            }
            (Section::Interface, "mtu") => {
                c.mtu = value.parse().map_err(|_| bad("the MTU isn't a number."))?;
            }
            (
                Section::Interface,
                "preup" | "postup" | "predown" | "postdown" | "table" | "saveconfig" | "fwmark",
            ) => {
                if !c.left_out.contains(&key) {
                    c.left_out.push(key.clone());
                }
            }
            (Section::Peer, k) => {
                let p = c.peers.last_mut().expect("a [Peer] section has a peer");
                match k {
                    "publickey" => {
                        if !is_key(value) {
                            return Err(bad("the public key isn't a WireGuard key."));
                        }
                        p.public_key = value.to_string();
                    }
                    "presharedkey" => {
                        if !is_key(value) {
                            return Err(bad("the preshared key isn't a WireGuard key."));
                        }
                        p.preshared_key = value.to_string();
                    }
                    "allowedips" => p.allowed_ips.extend(list(value)),
                    "endpoint" => p.endpoint = value.to_string(),
                    "persistentkeepalive" => {
                        p.keepalive = if value.eq_ignore_ascii_case("off") {
                            0
                        } else {
                            value
                                .parse()
                                .map_err(|_| bad("keepalive isn't a number of seconds."))?
                        };
                    }
                    _ => return Err(bad(&format!("\"{k}\" isn't a WireGuard peer setting."))),
                }
            }
            (Section::Interface, k) => {
                return Err(bad(&format!("\"{k}\" isn't a WireGuard setting.")));
            }
            (Section::None, _) => return Err(bad("settings must follow [Interface] or [Peer].")),
        }
    }
    if !seen_interface || c.private_key.is_empty() {
        return Err("The file has no [Interface] with a PrivateKey.".into());
    }
    if c.peers.is_empty() {
        return Err("The file has no [Peer].".into());
    }
    if let Some(p) = c.peers.iter().find(|p| p.public_key.is_empty()) {
        let _ = p;
        return Err("A [Peer] has no PublicKey.".into());
    }
    Ok(c)
}

/// The network device name NetworkManager creates: at most 15 safe characters.
pub fn interface_name(name: &str) -> String {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .take(15)
        .collect();
    if n.is_empty() || n == "." || n == ".." {
        "wg0".into()
    } else {
        n
    }
}

fn peer_dict(p: &Peer) -> HashMap<&'static str, Value<'static>> {
    let mut d = HashMap::from([
        ("public-key", Value::from(p.public_key.clone())),
        ("allowed-ips", Value::from(p.allowed_ips.clone())),
    ]);
    if !p.endpoint.is_empty() {
        d.insert("endpoint", Value::from(p.endpoint.clone()));
    }
    if !p.preshared_key.is_empty() {
        d.insert("preshared-key", Value::from(p.preshared_key.clone()));
        d.insert("preshared-key-flags", Value::from(0u32));
    }
    if p.keepalive > 0 {
        d.insert("persistent-keepalive", Value::from(p.keepalive));
    }
    d
}

/// NetworkManager settings for an imported file. `name` comes from the file name.
pub fn settings(
    c: &Config,
    name: &str,
    uuid: &str,
    user: &str,
    all_users: bool,
) -> Result<Settings, String> {
    let mut wg = HashMap::from([
        ("private-key".to_string(), ov(c.private_key.as_str())),
        ("private-key-flags".to_string(), ov(0u32)),
        (
            "peers".to_string(),
            ov(c.peers.iter().map(peer_dict).collect::<Vec<_>>()),
        ),
    ]);
    if c.listen_port > 0 {
        wg.insert("listen-port".into(), ov(c.listen_port));
    }
    if c.mtu > 0 {
        wg.insert("mtu".into(), ov(c.mtu));
    }
    let mut s = Settings::from([
        (
            "connection".to_string(),
            HashMap::from([
                ("id".to_string(), ov(name)),
                ("uuid".to_string(), ov(uuid)),
                ("type".to_string(), ov("wireguard")),
                ("interface-name".to_string(), ov(interface_name(name))),
                ("autoconnect".to_string(), ov(false)),
                ("permissions".to_string(), ov(permissions(user, all_users))),
            ]),
        ),
        ("wireguard".to_string(), wg),
    ]);
    let (v4, v6): (Vec<&String>, Vec<&String>) = c
        .addresses
        .iter()
        .partition(|a| !a.split('/').next().unwrap_or("").contains(':'));
    // DNS lines may hold search domains as well as servers.
    let (servers, search): (Vec<&String>, Vec<&String>) =
        c.dns.iter().partition(|d| d.parse::<IpAddr>().is_ok());
    let join = |v: &[&String]| v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",");
    let dns4: Vec<&String> = servers
        .iter()
        .copied()
        .filter(|d| !d.contains(':'))
        .collect();
    let dns6: Vec<&String> = servers
        .iter()
        .copied()
        .filter(|d| d.contains(':'))
        .collect();
    for (family, addrs, dns) in [(4u8, &v4, &dns4), (6u8, &v6, &dns6)] {
        let method = if addrs.is_empty() {
            "disabled"
        } else {
            "manual"
        };
        apply_ip(
            &mut s,
            family,
            &IpInput {
                method: method.into(),
                addresses: join(addrs),
                gateway: String::new(),
                dns: if addrs.is_empty() {
                    String::new()
                } else {
                    join(dns)
                },
            },
        )?;
    }
    if !search.is_empty() {
        let key = if v4.is_empty() { "ipv6" } else { "ipv4" };
        s.get_mut(key).expect("set above").insert(
            "dns-search".into(),
            ov(search.iter().map(|d| d.to_string()).collect::<Vec<_>>()),
        );
    }
    Ok(s)
}

/// What the page edits on a saved WireGuard connection: its first peer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerEdit {
    pub endpoint: String,
    /// Comma-separated.
    pub allowed_ips: String,
    pub keepalive: u32,
}

/// Change the first peer of a saved connection's settings, keeping its other keys (the
/// preshared key is a secret NetworkManager keeps; it isn't in what `GetSettings` returns).
pub fn edit_peer(s: &mut Settings, e: &PeerEdit) -> Result<(), String> {
    let allowed = list(&e.allowed_ips);
    if allowed.is_empty() {
        return Err("List at least one address range to send through the VPN.".into());
    }
    for a in &allowed {
        let ip = a.split('/').next().unwrap_or("");
        if ip.parse::<IpAddr>().is_err() {
            return Err(format!("{a} isn't an address range."));
        }
    }
    if e.keepalive > 65535 {
        return Err("Keepalive is 0 to 65535 seconds.".into());
    }
    let wg = s
        .get_mut("wireguard")
        .ok_or("This isn't a WireGuard connection.")?;
    let mut peers = get::<Vec<HashMap<String, OwnedValue>>>(wg, "peers").unwrap_or_default();
    let first = peers
        .first_mut()
        .ok_or("This connection has no peer to change.")?;
    if text(first, "public-key").is_empty() {
        return Err("This connection's peer has no public key.".into());
    }
    if e.endpoint.trim().is_empty() {
        first.remove("endpoint");
    } else {
        first.insert("endpoint".into(), ov(e.endpoint.trim()));
    }
    first.insert("allowed-ips".into(), ov(allowed));
    if e.keepalive == 0 {
        first.remove("persistent-keepalive");
    } else {
        first.insert("persistent-keepalive".into(), ov(e.keepalive));
    }
    wg.insert("peers".into(), ov(peers));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SAMPLE: &str = "\
# from the VPN provider
[Interface]
PrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=
Address = 10.2.0.2/32, fd00:2::2/128
DNS = 10.2.0.1, vpn.example
PostUp = iptables -A FORWARD -i %i -j ACCEPT

[Peer]
PublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=
PresharedKey = /UwcSPg38hW/D9Y3tcS1FOV0K1wuURMbS0sesJEP5ak=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.com:51820
PersistentKeepalive = 25
";

    #[test]
    fn parses_a_provider_file() {
        let c = parse(SAMPLE).unwrap();
        assert_eq!(c.addresses, ["10.2.0.2/32", "fd00:2::2/128"]);
        assert_eq!(c.dns, ["10.2.0.1", "vpn.example"]);
        assert_eq!(c.left_out, ["postup"]);
        let p = &c.peers[0];
        assert_eq!(p.allowed_ips, ["0.0.0.0/0", "::/0"]);
        assert_eq!(p.endpoint, "vpn.example.com:51820");
        assert_eq!(p.keepalive, 25);
        assert!(!p.preshared_key.is_empty());
    }

    #[test]
    fn bad_files_name_the_line() {
        assert!(
            parse("[Peer]\nPublicKey = x\n")
                .unwrap_err()
                .contains("Line 2")
        );
        assert!(parse("PrivateKey = a\n").unwrap_err().contains("follow"));
        assert!(
            parse("[Interface]\nAddress = 10.0.0.1/24\n")
                .unwrap_err()
                .contains("PrivateKey")
        );
        let no_peer = "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\n";
        assert!(parse(no_peer).unwrap_err().contains("[Peer]"));
        assert!(
            parse(&format!("{no_peer}Colour = red\n"))
                .unwrap_err()
                .contains("Line 3")
        );
        assert!(
            parse(&format!("{no_peer}just words\n"))
                .unwrap_err()
                .contains("name = value")
        );
    }

    #[test]
    fn settings_hold_the_keys_and_addresses() {
        let c = parse(SAMPLE).unwrap();
        let s = settings(&c, "work vpn!", "u-wg", "ana", false).unwrap();
        assert_eq!(text(&s["connection"], "type"), "wireguard");
        assert_eq!(text(&s["connection"], "interface-name"), "workvpn");
        assert_eq!(
            get::<Vec<String>>(&s["connection"], "permissions").unwrap(),
            ["user:ana:"]
        );
        assert_eq!(text(&s["wireguard"], "private-key"), c.private_key);
        let j = super::super::profile::summary("/c", &s);
        assert_eq!(j["kind"], "wireguard");
        assert_eq!(j["ipv4"]["addresses"], "10.2.0.2/32");
        assert_eq!(j["ipv4"]["dns"], "10.2.0.1");
        assert_eq!(j["ipv6"]["addresses"], "fd00:2::2/128");
        assert_eq!(j["wireguard"]["endpoint"], "vpn.example.com:51820");
        assert_eq!(j["wireguard"]["allowed_ips"], "0.0.0.0/0, ::/0");
        assert_eq!(j["wireguard"]["keepalive"], 25);
        assert_eq!(
            get::<Vec<String>>(&s["ipv4"], "dns-search").unwrap(),
            ["vpn.example"]
        );
    }

    #[test]
    fn edit_changes_the_first_peer() {
        let c = parse(SAMPLE).unwrap();
        let mut s = settings(&c, "wg", "u", "ana", true).unwrap();
        edit_peer(
            &mut s,
            &PeerEdit {
                endpoint: "203.0.113.5:51820".into(),
                allowed_ips: "10.2.0.0/16".into(),
                keepalive: 0,
            },
        )
        .unwrap();
        let j = super::super::profile::summary("/c", &s);
        assert_eq!(j["wireguard"]["endpoint"], "203.0.113.5:51820");
        assert_eq!(j["wireguard"]["allowed_ips"], "10.2.0.0/16");
        assert_eq!(j["wireguard"]["keepalive"], 0);
        assert!(
            edit_peer(
                &mut s,
                &PeerEdit {
                    allowed_ips: "nope".into(),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(edit_peer(&mut s, &PeerEdit::default()).is_err());
    }

    #[test]
    fn interface_names_are_safe() {
        assert_eq!(
            interface_name("a very long name for a vpn"),
            "averylongnamefo"
        );
        assert_eq!(interface_name("../"), "wg0");
        assert_eq!(interface_name("ü"), "wg0");
    }
}
