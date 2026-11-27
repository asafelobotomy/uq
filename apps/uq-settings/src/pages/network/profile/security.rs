//! Wi-Fi security and enterprise (EAP) settings.

use super::*;

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

/// A file for NetworkManager's certificate settings: `file://<path>` and a NUL, as bytes.
pub(crate) fn cert_blob(path: &str) -> Result<Vec<u8>, String> {
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

pub(crate) fn eap_setting(eap: &Eap) -> Result<HashMap<String, OwnedValue>, String> {
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
