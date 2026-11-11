//! Fake system services for the System pages' tests, on a private bus that stands in for the
//! system bus (spec §5): AccountsService, timedated, localed, hostnamed, power-profiles-daemon,
//! UPower and logind, plus the real `uq-settings-helper` service with a mock polkit authority
//! and its files under a temporary root.
//!
//! Each fake keeps its state behind an `Arc<Mutex<…>>` the test can read and change, and a
//! `deny` switch that makes every admin call fail the way polkit refusals do.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use uq_core::test_bus::{Bus, connect, private_bus};

use super::sysbus::SystemBus;

mod accounts;
mod helper;
mod power;
mod systemd;

pub use accounts::{FakeAccounts, FakeUser};
pub use helper::FakeHelper;
pub use power::{FakeBattery, FakePower};
pub use systemd::{FakeHostname, FakeLocale, FakeTimedate};

/// A private "system" bus and the fakes started on it.
pub struct SysRig {
    pub bus: Bus,
    /// The fakes' connections, held so their names stay owned.
    conns: Vec<zbus::Connection>,
}

impl SysRig {
    pub fn new() -> Self {
        Self {
            bus: private_bus(),
            conns: Vec::new(),
        }
    }

    /// The bus the code under test uses.
    pub fn system(&self) -> SystemBus {
        SystemBus::at(&self.bus.address)
    }

    /// Hold `conn` for the rig's lifetime.
    pub fn keep(&mut self, conn: zbus::Connection) {
        self.conns.push(conn);
    }

    /// A new connection that owns `name`, for one fake.
    pub async fn own(&mut self, name: &str) -> zbus::Connection {
        let c = connect(&self.bus).await;
        c.request_name(name).await.unwrap();
        self.conns.push(c.clone());
        c
    }
}

/// Shared by the fakes: whether admin calls are refused, as a cancelled polkit prompt is.
#[derive(Clone, Default)]
pub struct Deny(Arc<AtomicBool>);

impl Deny {
    pub fn set(&self, on: bool) {
        self.0.store(on, Ordering::SeqCst);
    }

    pub fn on(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// `org.freedesktop.DBus.Error.AccessDenied`, as systemd's services answer a refused prompt.
pub fn access_denied() -> zbus::fdo::Error {
    zbus::fdo::Error::AccessDenied("Access denied".into())
}

pub type Shared<T> = Arc<Mutex<T>>;
