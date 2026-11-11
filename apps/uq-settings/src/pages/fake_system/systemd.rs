//! Fake timedated, localed and hostnamed.

use std::sync::{Arc, Mutex};

use zbus::fdo;

use super::{Deny, Shared, SysRig, access_denied};

#[derive(Debug, Clone)]
pub struct TimedateState {
    pub timezone: String,
    pub ntp: bool,
    pub can_ntp: bool,
    pub synced: bool,
    pub time_usec: u64,
    pub zones: Vec<String>,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.timedate1")]
pub enum TimedateError {
    #[zbus(error)]
    ZBus(zbus::Error),
    AutomaticTimeSyncEnabled(String),
}

impl From<fdo::Error> for TimedateError {
    fn from(e: fdo::Error) -> Self {
        Self::ZBus(zbus::Error::FDO(Box::new(e)))
    }
}

struct TimedateIface {
    s: Shared<TimedateState>,
    deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.timedate1")]
impl TimedateIface {
    #[zbus(property)]
    fn timezone(&self) -> String {
        self.s.lock().unwrap().timezone.clone()
    }
    #[zbus(property, name = "NTP")]
    fn ntp(&self) -> bool {
        self.s.lock().unwrap().ntp
    }
    #[zbus(property, name = "CanNTP")]
    fn can_ntp(&self) -> bool {
        self.s.lock().unwrap().can_ntp
    }
    #[zbus(property, name = "NTPSynchronized")]
    fn ntp_synchronized(&self) -> bool {
        self.s.lock().unwrap().synced
    }
    #[zbus(property, name = "TimeUSec")]
    fn time_usec(&self) -> u64 {
        self.s.lock().unwrap().time_usec
    }
    fn list_timezones(&self) -> Vec<String> {
        self.s.lock().unwrap().zones.clone()
    }
    fn set_timezone(&mut self, tz: String, _interactive: bool) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        let mut s = self.s.lock().unwrap();
        if !s.zones.contains(&tz) {
            return Err(fdo::Error::InvalidArgs(format!(
                "Invalid or not installed time zone '{tz}'"
            )));
        }
        s.timezone = tz;
        Ok(())
    }
    #[zbus(name = "SetNTP")]
    fn set_ntp(&mut self, on: bool, _interactive: bool) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        self.s.lock().unwrap().ntp = on;
        Ok(())
    }
    fn set_time(&mut self, usec: i64, relative: bool, _i: bool) -> Result<(), TimedateError> {
        if self.deny.on() {
            return Err(access_denied().into());
        }
        let mut s = self.s.lock().unwrap();
        if s.ntp {
            return Err(TimedateError::AutomaticTimeSyncEnabled(
                "Automatic time synchronization is enabled".into(),
            ));
        }
        s.time_usec = if relative {
            (s.time_usec as i64 + usec) as u64
        } else {
            usec as u64
        };
        Ok(())
    }
}

pub struct FakeTimedate {
    pub state: Shared<TimedateState>,
    pub deny: Deny,
}

impl FakeTimedate {
    pub async fn start(rig: &mut SysRig) -> Self {
        let state = Arc::new(Mutex::new(TimedateState {
            timezone: "Europe/London".into(),
            ntp: true,
            can_ntp: true,
            synced: true,
            time_usec: 1_700_000_000_000_000,
            zones: vec![
                "America/New_York".into(),
                "Europe/Berlin".into(),
                "Europe/London".into(),
                "UTC".into(),
            ],
        }));
        let deny = Deny::default();
        let c = rig.own("org.freedesktop.timedate1").await;
        c.object_server()
            .at(
                "/org/freedesktop/timedate1",
                TimedateIface {
                    s: state.clone(),
                    deny: deny.clone(),
                },
            )
            .await
            .unwrap();
        Self { state, deny }
    }
}

#[derive(Debug, Clone)]
pub struct LocaleState {
    pub locale: Vec<String>,
    pub keymap: String,
}

struct LocaleIface {
    s: Shared<LocaleState>,
    deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.locale1")]
impl LocaleIface {
    #[zbus(property)]
    fn locale(&self) -> Vec<String> {
        self.s.lock().unwrap().locale.clone()
    }
    #[zbus(property, name = "VConsoleKeymap")]
    fn vconsole_keymap(&self) -> String {
        self.s.lock().unwrap().keymap.clone()
    }
    #[zbus(property, name = "VConsoleKeymapToggle")]
    fn vconsole_keymap_toggle(&self) -> String {
        String::new()
    }
    fn set_locale(&mut self, locale: Vec<String>, _interactive: bool) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        if locale.iter().any(|l| !l.contains('=')) {
            return Err(fdo::Error::InvalidArgs("Locale data is invalid".into()));
        }
        self.s.lock().unwrap().locale = locale;
        Ok(())
    }
    #[zbus(name = "SetVConsoleKeyboard")]
    fn set_vconsole_keyboard(
        &mut self,
        keymap: String,
        _toggle: String,
        _convert: bool,
        _interactive: bool,
    ) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        self.s.lock().unwrap().keymap = keymap;
        Ok(())
    }
}

pub struct FakeLocale {
    pub state: Shared<LocaleState>,
    pub deny: Deny,
}

impl FakeLocale {
    pub async fn start(rig: &mut SysRig) -> Self {
        let state = Arc::new(Mutex::new(LocaleState {
            locale: vec!["LANG=en_GB.UTF-8".into()],
            keymap: "uk".into(),
        }));
        let deny = Deny::default();
        let c = rig.own("org.freedesktop.locale1").await;
        c.object_server()
            .at(
                "/org/freedesktop/locale1",
                LocaleIface {
                    s: state.clone(),
                    deny: deny.clone(),
                },
            )
            .await
            .unwrap();
        Self { state, deny }
    }
}

#[derive(Debug, Clone)]
pub struct HostnameState {
    pub static_hostname: String,
    pub kernel: String,
    pub os: String,
    pub vendor: String,
    pub model: String,
}

struct HostnameIface {
    s: Shared<HostnameState>,
    deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.hostname1")]
impl HostnameIface {
    #[zbus(property)]
    fn hostname(&self) -> String {
        self.s.lock().unwrap().static_hostname.clone()
    }
    #[zbus(property)]
    fn static_hostname(&self) -> String {
        self.s.lock().unwrap().static_hostname.clone()
    }
    #[zbus(property)]
    fn kernel_release(&self) -> String {
        self.s.lock().unwrap().kernel.clone()
    }
    #[zbus(property)]
    fn operating_system_pretty_name(&self) -> String {
        self.s.lock().unwrap().os.clone()
    }
    #[zbus(property)]
    fn hardware_vendor(&self) -> String {
        self.s.lock().unwrap().vendor.clone()
    }
    #[zbus(property)]
    fn hardware_model(&self) -> String {
        self.s.lock().unwrap().model.clone()
    }
    fn set_static_hostname(&mut self, name: String, _interactive: bool) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        if name.contains(' ') {
            return Err(fdo::Error::InvalidArgs(format!(
                "Invalid static hostname '{name}'"
            )));
        }
        self.s.lock().unwrap().static_hostname = name;
        Ok(())
    }
}

pub struct FakeHostname {
    pub state: Shared<HostnameState>,
    pub deny: Deny,
}

impl FakeHostname {
    pub async fn start(rig: &mut SysRig) -> Self {
        let state = Arc::new(Mutex::new(HostnameState {
            static_hostname: "desk".into(),
            kernel: "6.18.1-arch1-1".into(),
            os: "Arch Linux".into(),
            vendor: "Framework".into(),
            model: "Laptop 13".into(),
        }));
        let deny = Deny::default();
        let c = rig.own("org.freedesktop.hostname1").await;
        c.object_server()
            .at(
                "/org/freedesktop/hostname1",
                HostnameIface {
                    s: state.clone(),
                    deny: deny.clone(),
                },
            )
            .await
            .unwrap();
        Self { state, deny }
    }
}
