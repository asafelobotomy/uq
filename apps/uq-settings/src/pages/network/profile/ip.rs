//! IP settings: addresses, gateway and DNS, read and written per family.

use super::*;

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

pub(crate) fn parse_address(a: &str, v6: bool) -> Result<(IpAddr, u32), String> {
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
pub(crate) fn dns_list(sec: &HashMap<String, OwnedValue>, v6: bool) -> Vec<String> {
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

pub(crate) fn addresses(sec: &HashMap<String, OwnedValue>) -> Vec<String> {
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

pub(crate) fn ip_json(s: &Settings, v6: bool) -> Json {
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
