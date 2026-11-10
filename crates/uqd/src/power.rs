//! The power mode, through power-profiles-daemon on the system bus.
//!
//! The current service name (`org.freedesktop.UPower.PowerProfiles`, since 0.20) is tried first,
//! then the older `net.hadess.PowerProfiles`. A profile that can't change the power mode skips it
//! with a notice; it never fails an apply, so every error here is plain data for the caller.

use async_trait::async_trait;
use std::time::Duration;
use zbus::proxy::CacheProperties;

/// How long `Desktop` waits for the power service before giving up.
pub const POWER_TIMEOUT: Duration = Duration::from_secs(3);

/// `(name, path, interface)` of each service, in the order they're tried.
pub const SERVICES: [(&str, &str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
        "org.freedesktop.UPower.PowerProfiles",
    ),
    (
        "net.hadess.PowerProfiles",
        "/net/hadess/PowerProfiles",
        "net.hadess.PowerProfiles",
    ),
];

const PROPERTY: &str = "ActiveProfile";

/// Why the power mode couldn't be read or set. The texts are what `profile::power_notice` takes.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PowerError {
    #[error("power modes aren't available on this computer")]
    Missing,
    /// The inner text is only logged.
    #[error("the computer didn't accept it")]
    Refused(String),
    #[error("the power settings didn't answer")]
    Timeout,
}

#[async_trait]
pub trait PowerProfiles: Send + Sync {
    async fn active(&self) -> Result<String, PowerError>;
    async fn set_active(&self, mode: &str) -> Result<(), PowerError>;
}

/// No power service at all: the default, so tests never touch the real system bus.
pub struct NoPowerProfiles;

#[async_trait]
impl PowerProfiles for NoPowerProfiles {
    async fn active(&self) -> Result<String, PowerError> {
        Err(PowerError::Missing)
    }
    async fn set_active(&self, _mode: &str) -> Result<(), PowerError> {
        Err(PowerError::Missing)
    }
}

/// power-profiles-daemon on the system bus (or on the bus at `address`, for tests).
#[derive(Default)]
pub struct SystemPowerProfiles {
    address: Option<String>,
}

impl SystemPowerProfiles {
    pub fn at(address: impl Into<String>) -> Self {
        SystemPowerProfiles {
            address: Some(address.into()),
        }
    }

    async fn connect(&self) -> Result<zbus::Connection, PowerError> {
        let conn = match &self.address {
            None => zbus::Connection::system().await,
            Some(a) => {
                zbus::connection::Builder::address(a.as_str())
                    .map_err(refused)?
                    .build()
                    .await
            }
        };
        conn.map_err(refused)
    }

    /// Run `f` against each service in turn; only "no such service" moves on to the next one.
    async fn each<T, F, Fut>(&self, f: F) -> Result<T, PowerError>
    where
        F: Fn(zbus::Proxy<'static>) -> Fut,
        Fut: std::future::Future<Output = zbus::Result<T>>,
    {
        let conn = self.connect().await?;
        for (name, path, iface) in SERVICES {
            let proxy = zbus::proxy::Builder::new(&conn)
                .destination(name)
                .and_then(|b| b.path(path))
                .and_then(|b| b.interface(iface))
                .map_err(refused)?
                .cache_properties(CacheProperties::No)
                .build()
                .await
                .map_err(refused)?;
            match f(proxy).await {
                Ok(v) => return Ok(v),
                Err(e) if is_missing(&e) => continue,
                Err(e) => return Err(refused(e)),
            }
        }
        Err(PowerError::Missing)
    }
}

fn refused(e: impl std::fmt::Display) -> PowerError {
    PowerError::Refused(e.to_string())
}

/// Whether the error says the service isn't there (not running and not startable).
fn is_missing(e: &zbus::Error) -> bool {
    use zbus::fdo::Error as Fdo;
    match e {
        zbus::Error::FDO(f) => matches!(**f, Fdo::ServiceUnknown(_) | Fdo::NameHasNoOwner(_)),
        zbus::Error::MethodError(name, _, _) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
        ),
        _ => false,
    }
}

#[async_trait]
impl PowerProfiles for SystemPowerProfiles {
    async fn active(&self) -> Result<String, PowerError> {
        self.each(|p| async move { p.get_property::<String>(PROPERTY).await })
            .await
    }

    async fn set_active(&self, mode: &str) -> Result<(), PowerError> {
        let mode = mode.to_string();
        self.each(|p| {
            let mode = mode.clone();
            async move {
                p.set_property(PROPERTY, mode)
                    .await
                    .map_err(zbus::Error::from)
            }
        })
        .await
    }
}

#[cfg(any(test, feature = "fake"))]
pub mod fake {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A power service in memory. `mode` is `None` when the service is missing.
    pub struct FakePower {
        pub mode: Mutex<Option<String>>,
        pub refuse: AtomicBool,
        pub delay: Mutex<Duration>,
        pub sets: Mutex<Vec<String>>,
    }

    impl FakePower {
        pub fn new(mode: &str) -> Self {
            FakePower {
                mode: Mutex::new(Some(mode.to_string())),
                refuse: AtomicBool::new(false),
                delay: Mutex::new(Duration::ZERO),
                sets: Mutex::new(Vec::new()),
            }
        }

        pub fn missing() -> Self {
            let p = FakePower::new("balanced");
            *p.mode.lock().unwrap() = None;
            p
        }

        async fn wait(&self) {
            let d = *self.delay.lock().unwrap();
            if !d.is_zero() {
                tokio::time::sleep(d).await;
            }
        }
    }

    #[async_trait]
    impl PowerProfiles for FakePower {
        async fn active(&self) -> Result<String, PowerError> {
            self.wait().await;
            self.mode.lock().unwrap().clone().ok_or(PowerError::Missing)
        }

        async fn set_active(&self, mode: &str) -> Result<(), PowerError> {
            self.wait().await;
            let mut cur = self.mode.lock().unwrap();
            if cur.is_none() {
                return Err(PowerError::Missing);
            }
            if self.refuse.load(Ordering::SeqCst) {
                return Err(PowerError::Refused("refused by the fake".into()));
            }
            *cur = Some(mode.to_string());
            self.sets.lock().unwrap().push(mode.to_string());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakePower;
    use super::*;
    use crate::compositor::fake::FakeCompositor;
    use crate::service::Desktop;
    use crate::system::FakeSystem;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use uq_core::paths::Paths;
    use uq_core::test_bus::{Bus, private_bus};

    struct Daemon {
        profile: Arc<Mutex<String>>,
    }

    #[zbus::interface(name = "org.freedesktop.UPower.PowerProfiles")]
    impl Daemon {
        #[zbus(property)]
        fn active_profile(&self) -> String {
            self.profile.lock().unwrap().clone()
        }
        #[zbus(property)]
        fn set_active_profile(&self, v: String) {
            *self.profile.lock().unwrap() = v;
        }
    }

    struct OldDaemon(Daemon);

    #[zbus::interface(name = "net.hadess.PowerProfiles")]
    impl OldDaemon {
        #[zbus(property)]
        fn active_profile(&self) -> String {
            self.0.profile.lock().unwrap().clone()
        }
        #[zbus(property)]
        fn set_active_profile(&self, v: String) {
            *self.0.profile.lock().unwrap() = v;
        }
    }

    /// Serve one of the two names on `bus`; the connection must stay alive.
    async fn serve_one(
        bus: &Bus,
        index: usize,
        mode: &str,
    ) -> (zbus::Connection, Arc<Mutex<String>>) {
        let profile = Arc::new(Mutex::new(mode.to_string()));
        let (name, path, _) = SERVICES[index];
        let b = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(name)
            .unwrap();
        let d = Daemon {
            profile: profile.clone(),
        };
        let b = if index == 0 {
            b.serve_at(path, d).unwrap()
        } else {
            b.serve_at(path, OldDaemon(d)).unwrap()
        };
        (b.build().await.unwrap(), profile)
    }

    #[tokio::test]
    async fn system_client_uses_new_name() {
        let bus = private_bus();
        let (_c, profile) = serve_one(&bus, 0, "balanced").await;
        let client = SystemPowerProfiles::at(bus.address.clone());
        assert_eq!(client.active().await.unwrap(), "balanced");
        client.set_active("performance").await.unwrap();
        assert_eq!(*profile.lock().unwrap(), "performance");
    }

    #[tokio::test]
    async fn system_client_falls_back_to_hadess_name() {
        let bus = private_bus();
        let (_c, profile) = serve_one(&bus, 1, "power-saver").await;
        let client = SystemPowerProfiles::at(bus.address.clone());
        assert_eq!(client.active().await.unwrap(), "power-saver");
        client.set_active("balanced").await.unwrap();
        assert_eq!(*profile.lock().unwrap(), "balanced");
    }

    #[tokio::test]
    async fn system_client_reports_missing_without_either_name() {
        let bus = private_bus();
        let client = SystemPowerProfiles::at(bus.address.clone());
        assert_eq!(client.active().await, Err(PowerError::Missing));
        assert_eq!(
            client.set_active("balanced").await,
            Err(PowerError::Missing)
        );
    }

    #[tokio::test]
    async fn system_client_reports_refused_for_a_bad_value() {
        // A service that answers but rejects the write is `Refused`, not `Missing`.
        struct Strict;
        #[zbus::interface(name = "org.freedesktop.UPower.PowerProfiles")]
        impl Strict {
            #[zbus(property)]
            fn active_profile(&self) -> String {
                "balanced".into()
            }
            #[zbus(property)]
            fn set_active_profile(&self, v: String) -> zbus::fdo::Result<()> {
                Err(zbus::fdo::Error::InvalidArgs(v))
            }
        }
        let bus = private_bus();
        let _c = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(SERVICES[0].0)
            .unwrap()
            .serve_at(SERVICES[0].1, Strict)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = SystemPowerProfiles::at(bus.address.clone());
        assert!(matches!(
            client.set_active("performance").await,
            Err(PowerError::Refused(_))
        ));
    }

    #[tokio::test]
    async fn power_call_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(FakePower::new("balanced"));
        *fake.delay.lock().unwrap() = Duration::from_secs(1);
        let mut paths = Paths::under(dir.path());
        paths.themes_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        let d = Desktop::new(
            paths,
            Arc::new(FakeCompositor::default()),
            Arc::new(FakeSystem::default()),
        )
        .unwrap()
        .with_power(fake)
        .with_power_timeout(Duration::from_millis(50));
        let t = Instant::now();
        assert_eq!(d.power_active().await, Err(PowerError::Timeout));
        assert_eq!(d.power_set("performance").await, Err(PowerError::Timeout));
        assert!(t.elapsed() < Duration::from_millis(500));
    }
}
