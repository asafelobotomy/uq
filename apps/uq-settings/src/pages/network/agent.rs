//! The Network page's NetworkManager secret agent (spec §2.6 "Agents while open", §3). It is
//! registered only while the page is open, and only answers for connections the person started
//! from the page; for any other it says it has no secrets, so another agent (or none) answers.
//! It asks the person through [`Prompts`] and hands what they type straight back to
//! NetworkManager. It stores nothing, so `SaveSecrets` and `DeleteSecrets` have nothing to do.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::json;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use super::nm::{Settings, text};
use super::profile::{kind, name, ov};
use crate::pages::prompt::{Answer, Prompts};

/// NM_SECRET_AGENT_GET_SECRETS_FLAG_ALLOW_INTERACTION.
const ALLOW_INTERACTION: u32 = 0x1;
/// NM_SECRET_AGENT_GET_SECRETS_FLAG_REQUEST_NEW: the last secret was wrong.
const REQUEST_NEW: u32 = 0x2;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent")]
pub enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NoSecrets(String),
    UserCanceled(String),
    AgentCanceled(String),
}

/// What the agent shares with the page.
#[derive(Default)]
pub struct Shared {
    pub prompts: Prompts,
    /// UUIDs of the connections started from the page.
    pub started: Mutex<HashSet<String>>,
}

/// One field the person fills in: the settings key and its label.
fn fields(s: &Settings, setting: &str, hints: &[String]) -> Vec<(&'static str, &'static str)> {
    match setting {
        "802-11-wireless-security" => {
            let km = s
                .get(setting)
                .map(|x| text(x, "key-mgmt"))
                .unwrap_or_default();
            match km.as_str() {
                "wpa-psk" | "sae" => vec![("psk", "Password")],
                "none" => vec![("wep-key0", "Key")],
                _ => Vec::new(),
            }
        }
        "802-1x" => {
            let tls = s
                .get(setting)
                .and_then(|x| super::nm::get::<Vec<String>>(x, "eap"))
                .is_some_and(|m| m.iter().any(|m| m == "tls"));
            if tls || hints.iter().any(|h| h == "private-key-password") {
                vec![("private-key-password", "Private key password")]
            } else {
                vec![("password", "Password")]
            }
        }
        "wireguard" => vec![("private-key", "Private key")],
        "vpn" => vec![("password", "Password")],
        _ => Vec::new(),
    }
}

/// The reply NetworkManager expects: `{setting: {key: value}}`; a VPN plugin's secrets sit in
/// `vpn.secrets` as `a{ss}`.
fn reply(setting: &str, values: HashMap<String, String>) -> Settings {
    let inner: HashMap<String, OwnedValue> = if setting == "vpn" {
        HashMap::from([("secrets".to_string(), ov(values))])
    } else {
        values.into_iter().map(|(k, v)| (k, ov(v))).collect()
    };
    Settings::from([(setting.to_string(), inner)])
}

pub struct SecretAgent {
    pub shared: Arc<Shared>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl SecretAgent {
    async fn get_secrets(
        &self,
        connection: Settings,
        connection_path: OwnedObjectPath,
        setting_name: String,
        hints: Vec<String>,
        flags: u32,
    ) -> Result<Settings, AgentError> {
        let uuid = connection
            .get("connection")
            .map(|c| text(c, "uuid"))
            .unwrap_or_default();
        let ours = self.shared.started.lock().unwrap().contains(&uuid);
        let wanted = fields(&connection, &setting_name, &hints);
        if !ours || flags & ALLOW_INTERACTION == 0 || wanted.is_empty() {
            return Err(AgentError::NoSecrets("No secrets here".into()));
        }
        let what = match kind(&connection) {
            "wifi" | "hotspot" => "Wi-Fi network",
            "wired" => "wired network",
            "vpn" | "wireguard" => "VPN",
            _ => "network",
        };
        let shown = json!({
            "kind": "secret",
            "name": name(&connection),
            "what": what,
            "retry": flags & REQUEST_NEW != 0,
            "fields": wanted.iter().map(|(k, l)| json!({"key": k, "label": l})).collect::<Vec<_>>(),
        });
        let key = format!("{}/{setting_name}", connection_path.as_str());
        match self.shared.prompts.ask(&key, shown).await {
            Some(Answer::Yes(values)) => {
                let keep: HashMap<String, String> = values
                    .into_iter()
                    .filter(|(k, _)| wanted.iter().any(|(w, _)| w == k))
                    .collect();
                Ok(reply(&setting_name, keep))
            }
            Some(Answer::No) => Err(AgentError::UserCanceled("Cancelled".into())),
            None => Err(AgentError::AgentCanceled("No longer asked".into())),
        }
    }

    fn cancel_get_secrets(&self, connection_path: OwnedObjectPath, setting_name: String) {
        let key = format!("{}/{setting_name}", connection_path.as_str());
        self.shared.prompts.cancel(Some(&key));
    }

    /// uQ stores no secrets: NetworkManager keeps the ones the page passes it.
    fn save_secrets(&self, _connection: Settings, _connection_path: OwnedObjectPath) {}

    fn delete_secrets(&self, _connection: Settings, _connection_path: OwnedObjectPath) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::network::profile::{Security, WifiInput, wifi_settings};

    #[test]
    fn asks_for_the_right_field() {
        let wpa = wifi_settings(
            "u",
            "ana",
            &WifiInput {
                ssid: b"x".to_vec(),
                security: Some(Security::Password { sae_only: false }),
                password: "12345678".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            fields(&wpa, "802-11-wireless-security", &[]),
            [("psk", "Password")]
        );
        assert!(fields(&wpa, "ipv4", &[]).is_empty());
        let r = reply("vpn", HashMap::from([("password".into(), "pw".into())]));
        assert!(r["vpn"].contains_key("secrets"));
        let r = reply(
            "802-11-wireless-security",
            HashMap::from([("psk".into(), "pw".into())]),
        );
        assert_eq!(text(&r["802-11-wireless-security"], "psk"), "pw");
    }
}
