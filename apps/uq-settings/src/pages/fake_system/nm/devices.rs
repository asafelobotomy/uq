//! The fake NetworkManager's devices, access points and active connections.

use super::*;

pub(crate) struct DeviceIface {
    pub(crate) s: Shared<NmState>,
    pub(crate) wifi: bool,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl DeviceIface {
    #[zbus(property)]
    pub(crate) fn interface(&self) -> String {
        if self.wifi { "wlan0" } else { "enp3s0" }.into()
    }
    #[zbus(property)]
    pub(crate) fn device_type(&self) -> u32 {
        if self.wifi { 2 } else { 1 }
    }
    #[zbus(property)]
    pub(crate) fn state(&self) -> u32 {
        let st = self.s.lock().unwrap();
        let mine = |a: &&FakeActive| {
            let kind = st
                .conns
                .iter()
                .find(|c| c.path == a.conn)
                .map(|c| text(&c.settings, "connection", "type"))
                .unwrap_or_default();
            (kind == "802-11-wireless") == self.wifi && (kind != "vpn" && kind != "wireguard")
        };
        match st.active.iter().find(mine) {
            Some(a) if a.state == 2 => 100,
            Some(_) => 50,
            None if !self.wifi && !st.carrier => 20,
            None => 30,
        }
    }
    #[zbus(property)]
    pub(crate) fn active_connection(&self) -> OwnedObjectPath {
        let st = self.s.lock().unwrap();
        let found = st.active.iter().find(|a| {
            st.conns.iter().find(|c| c.path == a.conn).is_some_and(|c| {
                (text(&c.settings, "connection", "type") == "802-3-ethernet") != self.wifi
            })
        });
        OwnedObjectPath::try_from(found.map_or("/", |a| a.path.as_str())).unwrap()
    }
}

pub(crate) fn ap_path(i: usize) -> String {
    format!("/org/freedesktop/NetworkManager/AccessPoint/{i}")
}

pub(crate) struct WirelessIface {
    pub(crate) s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl WirelessIface {
    #[zbus(property)]
    pub(crate) fn access_points(&self) -> Vec<OwnedObjectPath> {
        (0..self.s.lock().unwrap().aps.len())
            .map(|i| OwnedObjectPath::try_from(ap_path(i)).unwrap())
            .collect()
    }
    #[zbus(property)]
    pub(crate) fn last_scan(&self) -> i64 {
        1000
    }
    #[zbus(property)]
    pub(crate) fn wireless_capabilities(&self) -> u32 {
        0x40 | 0x1
    }

    pub(crate) async fn request_scan(
        &self,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.s.lock().unwrap().scans += 1;
        self.last_scan_changed(&emitter).await?;
        Ok(())
    }
}

pub(crate) struct WiredIface {
    pub(crate) s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wired")]
impl WiredIface {
    #[zbus(property)]
    pub(crate) fn carrier(&self) -> bool {
        self.s.lock().unwrap().carrier
    }
    #[zbus(property)]
    pub(crate) fn speed(&self) -> u32 {
        1000
    }
}

pub(crate) struct ApIface {
    pub(crate) s: Shared<NmState>,
    pub(crate) i: usize,
}

impl ApIface {
    pub(crate) fn with<T: Default>(&self, f: impl FnOnce(&FakeAp) -> T) -> T {
        self.s
            .lock()
            .unwrap()
            .aps
            .get(self.i)
            .map(f)
            .unwrap_or_default()
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl ApIface {
    #[zbus(property)]
    pub(crate) fn ssid(&self) -> Vec<u8> {
        self.with(|a| a.ssid.clone())
    }
    #[zbus(property)]
    pub(crate) fn strength(&self) -> u8 {
        self.with(|a| a.strength)
    }
    #[zbus(property)]
    pub(crate) fn flags(&self) -> u32 {
        self.with(|a| a.flags)
    }
    #[zbus(property)]
    pub(crate) fn wpa_flags(&self) -> u32 {
        self.with(|a| a.wpa)
    }
    #[zbus(property)]
    pub(crate) fn rsn_flags(&self) -> u32 {
        self.with(|a| a.rsn)
    }
    #[zbus(property)]
    pub(crate) fn frequency(&self) -> u32 {
        self.with(|a| a.frequency)
    }
}

pub(crate) struct ActiveIface {
    pub(crate) s: Shared<NmState>,
    pub(crate) path: String,
}

impl ActiveIface {
    pub(crate) fn with<T: Default>(&self, f: impl FnOnce(&FakeActive) -> T) -> T {
        let st = self.s.lock().unwrap();
        st.active
            .iter()
            .find(|a| a.path == self.path)
            .map(f)
            .unwrap_or_default()
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl ActiveIface {
    #[zbus(property)]
    pub(crate) fn uuid(&self) -> String {
        self.with(|a| a.uuid.clone())
    }
    #[zbus(property)]
    pub(crate) fn state(&self) -> u32 {
        self.with(|a| a.state)
    }
    #[zbus(property)]
    pub(crate) fn vpn(&self) -> bool {
        self.with(|a| a.vpn)
    }
}
