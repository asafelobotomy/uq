//! What the System pages share (spec §2.6, §3, §4): the system bus, how a refused or missing
//! system service reads, and reading a service's properties without hanging the page.
//!
//! The System pages talk to system services (AccountsService, timedated, localed, hostnamed,
//! power-profiles-daemon, UPower, logind) and to `uq-settings-helper`, never to uqd, so they
//! keep working when uqd isn't running. Admin actions use each service's own polkit action;
//! uQ's polkit agent shows the prompt, which can take minutes, so calls get a long timeout.

use std::future::Future;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::OnceCell;
use zbus::DBusError;

/// How long a call that may wait on a polkit prompt can take (Phase 5: at least 10 min).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// How long reading a service's state may take before the page shows it as not answering.
pub const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// The environment variable that points the System pages at another system bus (tests, the
/// screenshot run). Without it, `DBUS_SYSTEM_BUS_ADDRESS` or the standard socket is used.
pub const ADDRESS_ENV: &str = "UQ_SETTINGS_SYSTEM_BUS";

/// The system bus, connected on first use (a page that is never opened never connects). A
/// failed connection is tried again on the next call.
pub struct SystemBus {
    conn: OnceCell<zbus::Connection>,
    address: Option<String>,
}

impl SystemBus {
    /// The real system bus, or the one [`ADDRESS_ENV`] names.
    pub fn system() -> Self {
        Self {
            conn: OnceCell::new(),
            address: std::env::var(ADDRESS_ENV).ok().filter(|a| !a.is_empty()),
        }
    }

    /// The bus at `address` (a private bus in tests).
    #[cfg(test)]
    pub fn at(address: &str) -> Self {
        Self {
            conn: OnceCell::new(),
            address: Some(address.to_string()),
        }
    }

    pub async fn conn(&self) -> zbus::Result<zbus::Connection> {
        self.conn
            .get_or_try_init(|| async {
                let builder = match &self.address {
                    Some(a) => zbus::connection::Builder::address(a.as_str())?,
                    None => zbus::connection::Builder::system()?,
                };
                builder.method_timeout(CALL_TIMEOUT).build().await
            })
            .await
            .cloned()
    }
}

/// What a failed call means for the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// polkit refused or the prompt was cancelled: the control goes back quietly (§4).
    Quiet,
    /// The service isn't installed or running.
    Missing,
    /// Anything else, with the service's own message.
    Shown(String),
}

/// D-Bus error names that mean "not allowed" or "cancelled" at a polkit prompt.
const QUIET: [&str; 9] = [
    "org.freedesktop.DBus.Error.AccessDenied",
    "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired",
    "org.freedesktop.Accounts.Error.PermissionDenied",
    "org.freedesktop.PolicyKit1.Error.Cancelled",
    "org.freedesktop.PolicyKit1.Error.NotAuthorized",
    "org.uq.SettingsHelper1.Error.NotAuthorized",
    "net.hadess.PowerProfiles.Error.AccessDenied",
    "org.freedesktop.NetworkManager.PermissionDenied",
    "org.freedesktop.NetworkManager.Settings.PermissionDenied",
];

/// D-Bus error names that mean the service isn't there.
const MISSING: [&str; 5] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.Spawn.ServiceNotFound",
    "org.freedesktop.DBus.Error.Spawn.ChildExited",
    "org.freedesktop.systemd1.NoSuchUnit",
];

/// The error's D-Bus name, when it has one.
pub fn error_name(e: &zbus::Error) -> Option<String> {
    match e {
        zbus::Error::MethodError(name, _, _) => Some(name.to_string()),
        zbus::Error::FDO(f) => Some(f.name().to_string()),
        _ => None,
    }
}

/// The text a service sent with its error, else the error itself.
pub fn error_text(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(text), _) => text.clone(),
        zbus::Error::FDO(f) => f
            .description()
            .map_or_else(|| f.to_string(), str::to_string),
        other => other.to_string(),
    }
}

pub fn classify(e: &zbus::Error) -> Failure {
    match error_name(e) {
        Some(n) if QUIET.contains(&n.as_str()) => Failure::Quiet,
        Some(n) if MISSING.contains(&n.as_str()) => Failure::Missing,
        _ => Failure::Shown(error_text(e)),
    }
}

/// A plain failure to show on the page.
pub fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

/// The result of an admin call as the page treats it: a refusal or cancel is not an error
/// (the state is read again, so the control shows the old value); a missing service names the
/// package to install.
pub fn settle<T>(r: zbus::Result<T>, service: &Service) -> zbus::Result<()> {
    match r {
        Ok(_) => Ok(()),
        Err(e) => match classify(&e) {
            Failure::Quiet => Ok(()),
            Failure::Missing => Err(failure(service.missing_text())),
            Failure::Shown(text) => Err(failure(text)),
        },
    }
}

/// A system service a page uses: its plain name and the package that provides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Service {
    pub name: &'static str,
    pub package: &'static str,
}

impl Service {
    pub fn missing_text(&self) -> String {
        format!(
            "{} isn't available. Install the {} package.",
            self.name, self.package
        )
    }

    /// `{available: false, name, package, text}` for the page's "Not available" card.
    pub fn missing_json(&self) -> Value {
        json!({
            "available": false,
            "name": self.name,
            "package": self.package,
            "text": self.missing_text(),
        })
    }
}

pub const ACCOUNTS: Service = Service {
    name: "The accounts service",
    package: "accountsservice",
};
pub const TIMEDATE: Service = Service {
    name: "The date and time service",
    package: "systemd",
};
pub const LOCALE: Service = Service {
    name: "The language and keyboard service",
    package: "systemd",
};
pub const HOSTNAME: Service = Service {
    name: "The computer name service",
    package: "systemd",
};
pub const POWER_PROFILES: Service = Service {
    name: "Power modes",
    package: "power-profiles-daemon",
};
pub const UPOWER: Service = Service {
    name: "The battery service",
    package: "upower",
};
pub const HELPER: Service = Service {
    name: "uQ's system settings helper",
    package: "uq",
};

/// Run a read with [`READ_TIMEOUT`]; a service that doesn't answer reads as an error.
pub async fn within<T>(f: impl Future<Output = zbus::Result<T>>) -> zbus::Result<T> {
    match tokio::time::timeout(READ_TIMEOUT, f).await {
        Ok(r) => r,
        Err(_) => Err(failure("The service didn't answer in time.")),
    }
}

/// The state block for a service whose read failed: "Not available" when it is missing,
/// otherwise its message.
pub fn read_error(e: &zbus::Error, service: &Service) -> Value {
    match classify(e) {
        Failure::Missing => service.missing_json(),
        Failure::Quiet => json!({"available": false, "name": service.name,
            "package": service.package, "text": format!("{} refused to answer.", service.name)}),
        Failure::Shown(text) => json!({"available": false, "name": service.name,
            "package": service.package, "text": text}),
    }
}

/// Whether `name` is running or can be started on `conn`.
pub async fn has_service(conn: &zbus::Connection, name: &str) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
        return false;
    };
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
        return false;
    };
    if within(async { Ok(dbus.name_has_owner(bus_name).await?) })
        .await
        .unwrap_or(false)
    {
        return true;
    }
    within(async { Ok(dbus.list_activatable_names().await?) })
        .await
        .is_ok_and(|names| names.iter().any(|n| n.as_str() == name))
}

/// The uid this app runs as, from `/proc/self`.
pub fn my_uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").map_or(u32::MAX, |m| m.uid())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method_error(name: &str, text: &str) -> zbus::Error {
        let msg = zbus::message::Message::method_call("/", "X")
            .unwrap()
            .build(&())
            .unwrap();
        zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(name).unwrap(),
            Some(text.to_string()),
            msg,
        )
    }

    #[test]
    fn polkit_refusals_are_quiet() {
        for name in QUIET {
            assert_eq!(
                classify(&method_error(name, "no")),
                Failure::Quiet,
                "{name}"
            );
        }
        assert_eq!(
            settle(Err::<(), _>(method_error(QUIET[0], "denied")), &TIMEDATE).ok(),
            Some(())
        );
    }

    #[test]
    fn missing_service_names_the_package() {
        let e = method_error("org.freedesktop.DBus.Error.ServiceUnknown", "x");
        assert_eq!(classify(&e), Failure::Missing);
        let err = settle(Err::<(), _>(e), &POWER_PROFILES).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Power modes isn't available. Install the power-profiles-daemon package."
        );
        let block = read_error(
            &method_error("org.freedesktop.DBus.Error.NameHasNoOwner", "x"),
            &ACCOUNTS,
        );
        assert_eq!(block["available"], false);
        assert_eq!(block["package"], "accountsservice");
    }

    #[test]
    fn other_errors_keep_the_service_text() {
        let e = method_error(
            "org.freedesktop.timedate1.AutomaticTimeSyncEnabled",
            "NTP on",
        );
        assert_eq!(classify(&e), Failure::Shown("NTP on".into()));
    }
}
