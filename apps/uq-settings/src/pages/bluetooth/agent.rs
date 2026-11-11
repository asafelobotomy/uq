//! The Bluetooth page's pairing agent (`org.bluez.Agent1`, capability `KeyboardDisplay`). Every
//! question goes to the person through [`Prompts`]; nothing is accepted on their behalf. A code
//! BlueZ only wants shown (the other device types it) is kept in [`Shared::display`] until the
//! pairing ends or BlueZ cancels it.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;

use super::{BLUEZ, DEVICE, Shared};
use crate::pages::prompt::Answer;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
    Canceled(String),
}

pub struct PairingAgent {
    pub shared: Arc<Shared>,
}

/// The device's name as BlueZ has it, else its path's address.
async fn device_name(conn: &zbus::Connection, device: &OwnedObjectPath) -> String {
    let read = async {
        let p = zbus::proxy::Builder::<zbus::Proxy<'static>>::new(conn)
            .destination(BLUEZ)?
            .path(device.clone())?
            .interface(DEVICE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        p.get_property::<String>("Alias").await
    };
    match tokio::time::timeout(std::time::Duration::from_secs(2), read).await {
        Ok(Ok(name)) if !name.is_empty() => name,
        _ => device
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_start_matches("dev_")
            .replace('_', ":"),
    }
}

impl PairingAgent {
    /// Ask `kind` about `device`; Ok with what was typed on Yes.
    async fn ask(
        &self,
        conn: &zbus::Connection,
        device: &OwnedObjectPath,
        kind: &str,
        extra: serde_json::Value,
    ) -> Result<HashMap<String, String>, AgentError> {
        let mut shown = json!({"kind": kind, "device": device_name(conn, device).await});
        if let (Some(o), Some(e)) = (shown.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        match self.shared.prompts.ask(device.as_str(), shown).await {
            Some(Answer::Yes(values)) => Ok(values),
            Some(Answer::No) => Err(AgentError::Rejected("Rejected".into())),
            None => Err(AgentError::Canceled("Canceled".into())),
        }
    }
}

/// A Bluetooth service as words (the common ones).
fn service_text(uuid: &str) -> &'static str {
    match uuid.get(4..8).unwrap_or("").to_ascii_lowercase().as_str() {
        "110a" | "110b" | "110d" => "play audio",
        "1108" | "111e" | "111f" => "make and take calls",
        "1124" | "1812" => "act as a keyboard, mouse or controller",
        "1105" | "1106" => "send files",
        "1116" | "1115" => "share a network connection",
        _ => "use a service",
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl PairingAgent {
    fn release(&self) {
        self.shared.prompts.cancel(None);
    }

    async fn request_pin_code(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> Result<String, AgentError> {
        let v = self.ask(conn, &device, "pin", json!({})).await?;
        let pin = v.get("value").cloned().unwrap_or_default();
        if pin.is_empty() || pin.len() > 16 || !pin.chars().all(|c| c.is_ascii_graphic()) {
            return Err(AgentError::Rejected("Not a PIN".into()));
        }
        Ok(pin)
    }

    async fn display_pin_code(
        &self,
        device: OwnedObjectPath,
        pincode: String,
        #[zbus(connection)] conn: &zbus::Connection,
    ) {
        let name = device_name(conn, &device).await;
        *self.shared.display.lock().unwrap() = Some(json!({"device": name, "code": pincode}));
        self.shared.ping();
    }

    async fn request_passkey(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> Result<u32, AgentError> {
        let v = self.ask(conn, &device, "passkey", json!({})).await?;
        v.get("value")
            .and_then(|p| p.trim().parse::<u32>().ok())
            .filter(|p| *p <= 999_999)
            .ok_or_else(|| AgentError::Rejected("Not a passkey".into()))
    }

    async fn display_passkey(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        entered: u16,
        #[zbus(connection)] conn: &zbus::Connection,
    ) {
        let name = device_name(conn, &device).await;
        *self.shared.display.lock().unwrap() = Some(json!({
            "device": name, "code": format!("{passkey:06}"), "entered": entered,
        }));
        self.shared.ping();
    }

    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> Result<(), AgentError> {
        self.ask(
            conn,
            &device,
            "confirm",
            json!({"code": format!("{passkey:06}")}),
        )
        .await
        .map(|_| ())
    }

    async fn request_authorization(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> Result<(), AgentError> {
        self.ask(conn, &device, "authorize", json!({}))
            .await
            .map(|_| ())
    }

    async fn authorize_service(
        &self,
        device: OwnedObjectPath,
        uuid: String,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> Result<(), AgentError> {
        self.ask(
            conn,
            &device,
            "service",
            json!({"service": service_text(&uuid)}),
        )
        .await
        .map(|_| ())
    }

    fn cancel(&self) {
        self.shared.prompts.cancel(None);
        *self.shared.display.lock().unwrap() = None;
        self.shared.ping();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_in_words() {
        assert_eq!(
            service_text("0000110b-0000-1000-8000-00805f9b34fb"),
            "play audio"
        );
        assert_eq!(service_text("x"), "use a service");
    }
}
