//! The polkit check every method makes before it writes, behind a trait so tests can use a
//! mock authority.
//!
//! The subject is the caller's unique system-bus name (`system-bus-name`), which polkit
//! resolves to the process and session itself; it has none of the PID-reuse races of a
//! `unix-process` subject.

use async_trait::async_trait;
use std::collections::HashMap;
use zbus::zvariant::Value;

/// The polkit actions in `org.uq.settings.policy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `org.uq.settings.power` (`auth_admin_keep`).
    Power,
    /// `org.uq.settings.greeter` (`auth_admin_keep`).
    Greeter,
    /// `org.uq.settings.autologin` (`auth_admin`, never kept).
    AutoLogin,
}

impl Action {
    pub const ALL: [Action; 3] = [Self::Power, Self::Greeter, Self::AutoLogin];

    pub fn id(self) -> &'static str {
        match self {
            Self::Power => "org.uq.settings.power",
            Self::Greeter => "org.uq.settings.greeter",
            Self::AutoLogin => "org.uq.settings.autologin",
        }
    }
}

/// Decides whether a caller may perform an action.
#[async_trait]
pub trait Authority: Send + Sync {
    /// `Ok(true)` only when authorised. `Err` carries why the check itself failed; the caller
    /// treats that as a refusal.
    async fn check(&self, sender: &str, action: Action) -> Result<bool, String>;
}

pub const POLKIT_NAME: &str = "org.freedesktop.PolicyKit1";
pub const POLKIT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
pub const POLKIT_IFACE: &str = "org.freedesktop.PolicyKit1.Authority";
/// `CheckAuthorizationFlags.AllowUserInteraction`.
pub const ALLOW_USER_INTERACTION: u32 = 1;

/// polkit's authority on the bus `conn` is connected to (the system bus in production).
#[derive(Debug, Clone)]
pub struct Polkit {
    conn: zbus::Connection,
}

impl Polkit {
    pub fn new(conn: zbus::Connection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl Authority for Polkit {
    async fn check(&self, sender: &str, action: Action) -> Result<bool, String> {
        let mut subject_details: HashMap<&str, Value<'_>> = HashMap::new();
        subject_details.insert("name", Value::from(sender));
        let subject = ("system-bus-name", subject_details);
        let details: HashMap<&str, &str> = HashMap::new();
        let reply = self
            .conn
            .call_method(
                Some(POLKIT_NAME),
                POLKIT_PATH,
                Some(POLKIT_IFACE),
                "CheckAuthorization",
                &(subject, action.id(), details, ALLOW_USER_INTERACTION, ""),
            )
            .await
            .map_err(|e| format!("polkit check failed: {e}"))?;
        let (authorized, _challenge, _details): (bool, bool, HashMap<String, String>) = reply
            .body()
            .deserialize()
            .map_err(|e| format!("polkit gave an unexpected reply: {e}"))?;
        Ok(authorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_match_the_policy_file() {
        let policy = include_str!("../../../packaging/polkit/org.uq.settings.policy");
        for a in Action::ALL {
            assert!(
                policy.contains(&format!("<action id=\"{}\">", a.id())),
                "{}",
                a.id()
            );
        }
    }
}
