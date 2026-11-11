//! Fake power-profiles-daemon (its current bus name), UPower with one battery, and logind's
//! running lid and power-key actions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::fdo;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use super::{Deny, Shared, SysRig, access_denied};

#[derive(Debug, Clone)]
pub struct PowerState {
    pub profile: String,
    pub profiles: Vec<String>,
    pub degraded: String,
    pub on_battery: bool,
    pub lid: bool,
    pub logind: [String; 3],
}

#[derive(Debug, Clone)]
pub struct FakeBattery {
    pub state: u32,
    pub percentage: f64,
    pub time_to_empty: i64,
    pub time_to_full: i64,
    pub capacity: f64,
}

struct PpdIface {
    s: Shared<PowerState>,
    deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.UPower.PowerProfiles")]
impl PpdIface {
    #[zbus(property)]
    fn active_profile(&self) -> String {
        self.s.lock().unwrap().profile.clone()
    }
    #[zbus(property)]
    fn set_active_profile(&mut self, p: String) -> fdo::Result<()> {
        if self.deny.on() {
            return Err(access_denied());
        }
        let mut s = self.s.lock().unwrap();
        if !s.profiles.contains(&p) {
            return Err(fdo::Error::InvalidArgs(format!(
                "Invalid profile name '{p}'"
            )));
        }
        s.profile = p;
        Ok(())
    }
    #[zbus(property)]
    fn profiles(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.s
            .lock()
            .unwrap()
            .profiles
            .iter()
            .map(|p| {
                HashMap::from([
                    (
                        "Profile".to_string(),
                        OwnedValue::try_from(Value::from(p.as_str())).unwrap(),
                    ),
                    (
                        "Driver".to_string(),
                        OwnedValue::try_from(Value::from("platform_profile")).unwrap(),
                    ),
                ])
            })
            .collect()
    }
    #[zbus(property)]
    fn performance_degraded(&self) -> String {
        self.s.lock().unwrap().degraded.clone()
    }
}

struct UPowerIface {
    s: Shared<PowerState>,
    has_battery: bool,
}

#[zbus::interface(name = "org.freedesktop.UPower")]
impl UPowerIface {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.s.lock().unwrap().on_battery
    }
    #[zbus(property)]
    fn lid_is_present(&self) -> bool {
        self.s.lock().unwrap().lid
    }
    fn enumerate_devices(&self) -> Vec<OwnedObjectPath> {
        let mut out = vec![
            OwnedObjectPath::try_from("/org/freedesktop/UPower/devices/line_power_AC").unwrap(),
        ];
        if self.has_battery {
            out.push(
                OwnedObjectPath::try_from("/org/freedesktop/UPower/devices/battery_BAT0").unwrap(),
            );
        }
        out
    }
}

struct DeviceIface {
    battery: Option<Shared<FakeBattery>>,
}

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl DeviceIface {
    #[zbus(property, name = "Type")]
    fn kind(&self) -> u32 {
        if self.battery.is_some() { 2 } else { 1 }
    }
    #[zbus(property)]
    fn power_supply(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn is_present(&self) -> bool {
        self.battery.is_some()
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.battery.as_ref().map_or(0, |b| b.lock().unwrap().state)
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        self.battery
            .as_ref()
            .map_or(0.0, |b| b.lock().unwrap().percentage)
    }
    #[zbus(property)]
    fn time_to_empty(&self) -> i64 {
        self.battery
            .as_ref()
            .map_or(0, |b| b.lock().unwrap().time_to_empty)
    }
    #[zbus(property)]
    fn time_to_full(&self) -> i64 {
        self.battery
            .as_ref()
            .map_or(0, |b| b.lock().unwrap().time_to_full)
    }
    #[zbus(property)]
    fn capacity(&self) -> f64 {
        self.battery
            .as_ref()
            .map_or(0.0, |b| b.lock().unwrap().capacity)
    }
    #[zbus(property)]
    fn vendor(&self) -> String {
        "ACME".into()
    }
    #[zbus(property)]
    fn model(&self) -> String {
        "BAT-1".into()
    }
}

struct LogindIface {
    s: Shared<PowerState>,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl LogindIface {
    #[zbus(property)]
    fn handle_lid_switch(&self) -> String {
        self.s.lock().unwrap().logind[0].clone()
    }
    #[zbus(property)]
    fn handle_lid_switch_external_power(&self) -> String {
        self.s.lock().unwrap().logind[1].clone()
    }
    #[zbus(property)]
    fn handle_power_key(&self) -> String {
        self.s.lock().unwrap().logind[2].clone()
    }
}

pub struct FakePower {
    pub state: Shared<PowerState>,
    pub battery: Shared<FakeBattery>,
    pub deny: Deny,
}

impl FakePower {
    /// power-profiles-daemon, UPower (with a battery when `battery`) and logind. `with_ppd`
    /// false leaves power-profiles-daemon out, as on a machine without it.
    pub async fn start(rig: &mut SysRig, with_ppd: bool, battery: bool) -> Self {
        let state = Arc::new(Mutex::new(PowerState {
            profile: "balanced".into(),
            profiles: vec![
                "power-saver".into(),
                "balanced".into(),
                "performance".into(),
            ],
            degraded: String::new(),
            on_battery: battery,
            lid: battery,
            logind: ["suspend".into(), "suspend".into(), "poweroff".into()],
        }));
        let bat = Arc::new(Mutex::new(FakeBattery {
            state: 2,
            percentage: 81.0,
            time_to_empty: 3 * 3600 + 20 * 60,
            time_to_full: 0,
            capacity: 92.5,
        }));
        let deny = Deny::default();
        if with_ppd {
            let c = rig.own("org.freedesktop.UPower.PowerProfiles").await;
            c.object_server()
                .at(
                    "/org/freedesktop/UPower/PowerProfiles",
                    PpdIface {
                        s: state.clone(),
                        deny: deny.clone(),
                    },
                )
                .await
                .unwrap();
        }
        let c = rig.own("org.freedesktop.UPower").await;
        let os = c.object_server();
        os.at(
            "/org/freedesktop/UPower",
            UPowerIface {
                s: state.clone(),
                has_battery: battery,
            },
        )
        .await
        .unwrap();
        os.at(
            "/org/freedesktop/UPower/devices/line_power_AC",
            DeviceIface { battery: None },
        )
        .await
        .unwrap();
        if battery {
            os.at(
                "/org/freedesktop/UPower/devices/battery_BAT0",
                DeviceIface {
                    battery: Some(bat.clone()),
                },
            )
            .await
            .unwrap();
        }
        let c = rig.own("org.freedesktop.login1").await;
        c.object_server()
            .at("/org/freedesktop/login1", LogindIface { s: state.clone() })
            .await
            .unwrap();
        Self {
            state,
            battery: bat,
            deny,
        }
    }
}
