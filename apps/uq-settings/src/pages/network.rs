//! The Network page (spec §2.6, §3): Wi-Fi (on/off, scan, join, forget), saved connections and
//! their IP, DNS and metered settings, wired connections, a hotspot, VPNs (WireGuard imported
//! from a config file and edited here; OpenVPN and other plugin VPNs listed, started and
//! stopped) and work or school (802.1x: PEAP, TTLS, TLS) Wi-Fi. Everything goes through
//! NetworkManager on the system bus; its own polkit actions guard the changes.
//!
//! **Secrets.** A password typed on the page goes into the settings sent to NetworkManager and
//! nowhere else; uQ never logs or keeps one. While the page is open it is also a NetworkManager
//! secret agent ([`agent`]), but only for connections started from the page: when one of those
//! needs a password (a wrong one, a saved network without one), the page asks for it. New
//! connections are private to the person who adds them unless they choose "available to all
//! users".
//!
//! Network names and device names come from the air and from the system: the page shows them
//! through UqLabel (plain text) only.

mod agent;
pub(crate) mod nm;
pub(crate) mod profile;
pub(crate) mod wireguard;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use zbus::wire::OwnedObjectPath;

use super::prompt::Answer;
use super::runner::PageLogic;
use super::sysbus::{self, Service, SystemBus, failure, settle};
use nm::{
    AgentManagerProxy, NetworkManagerProxy, NmConnectionProxy, NmSettingsProxy, NmWirelessProxy,
    Props, Settings,
};
use profile::{Eap, IpInput, Security, WifiInput};
mod actions;
mod read;

pub const NETWORK: Service = Service {
    name: "Network settings",
    package: "networkmanager",
};

/// Where NetworkManager's VPN plugins announce themselves (under the page's root).
const VPN_PLUGINS: &str = "usr/lib/NetworkManager/VPN";
/// The biggest WireGuard file the page reads.
const MAX_IMPORT: u64 = 64 * 1024;

pub const SAVED_NOTE: &str = "Saved. It applies the next time this connection starts.";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetWifi(bool),
    Scan,
    /// Join the network an access point belongs to (a saved one is started again).
    ConnectWifi {
        ap: String,
        device: String,
        password: String,
        all_users: bool,
        eap: Option<Eap>,
    },
    /// Join a network that doesn't show its name.
    ConnectHidden {
        ssid: String,
        /// `open` or `password`.
        security: String,
        password: String,
        all_users: bool,
    },
    Activate {
        uuid: String,
    },
    Deactivate {
        uuid: String,
    },
    Forget {
        uuid: String,
    },
    SetAutoconnect {
        uuid: String,
        on: bool,
    },
    SetMetered {
        uuid: String,
        metered: String,
    },
    SetEveryone {
        uuid: String,
        on: bool,
    },
    SetIp {
        uuid: String,
        family: u8,
        ip: IpInput,
    },
    ImportWireGuard {
        path: PathBuf,
        all_users: bool,
    },
    EditWireGuard {
        uuid: String,
        peer: wireguard::PeerEdit,
    },
    Hotspot {
        on: bool,
        name: String,
        password: String,
    },
    /// The person's answer to the password question on show.
    AnswerSecret {
        id: u64,
        values: HashMap<String, String>,
    },
    CancelSecret {
        id: u64,
    },
}

pub struct NetworkState {
    system: SystemBus,
    root: PathBuf,
    user: String,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
    shared: Arc<agent::Shared>,
    registered: std::sync::atomic::AtomicBool,
}

impl Drop for NetworkState {
    fn drop(&mut self) {
        // A question still on show is dropped: NetworkManager hears "cancelled".
        self.shared.prompts.close();
    }
}

/// The name of the account this app runs as, from `/etc/passwd`, else `$USER`.
pub fn my_user_name() -> String {
    let uid = sysbus::my_uid().to_string();
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|p| {
            p.lines().find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 2 && f[2] == uid).then(|| f[0].to_string())
            })
        })
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default()
}

/// Why a device failed, in words (NMDeviceStateReason).
pub fn reason_text(reason: u32) -> String {
    match reason {
        7 => "a password is needed and none was given.".into(),
        8..=10 => "the network didn't accept the password or settings.".into(),
        11 => "the network didn't answer in time.".into(),
        5 | 6 => "the network didn't give this computer an address.".into(),
        53 => "the network is out of range.".into(),
        n => format!("something went wrong (code {n})."),
    }
}

impl NetworkState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system(), PathBuf::from("/"), my_user_name())
    }

    pub fn with(system: SystemBus, root: PathBuf, user: String) -> Self {
        Self {
            system,
            root,
            user,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
            shared: Arc::new(agent::Shared::default()),
            registered: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    /// Whether NetworkManager's OpenVPN plugin is installed.
    fn openvpn(&self) -> bool {
        self.root
            .join(VPN_PLUGINS)
            .join("nm-openvpn-service.name")
            .is_file()
    }

    /// Read a saved connection, let `edit` change it, and save it back.
    async fn edit(
        &self,
        uuid: &str,
        edit: impl FnOnce(&mut Settings) -> Result<(), String>,
    ) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let mut saved = self.find(&conn, uuid).await?;
        edit(&mut saved.settings).map_err(failure)?;
        let c: NmConnectionProxy = nm::at(&conn, &saved.path).await?;
        let r = c.update(&saved.settings).await;
        let ok = r.is_ok();
        settle(r, &NETWORK)?;
        if ok
            && self
                .active(&conn)
                .await
                .unwrap_or_default()
                .iter()
                .any(|a| a.uuid == uuid)
        {
            self.notice(SAVED_NOTE);
        }
        Ok(())
    }
}

impl PageLogic for NetworkState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::AnswerSecret { id, values } => {
                self.answer(id, Answer::Yes(values));
                Ok(())
            }
            Op::CancelSecret { id } => {
                self.answer(id, Answer::No);
                Ok(())
            }
            Op::SetWifi(on) => {
                let conn = self.conn().await?;
                let nm = NetworkManagerProxy::new(&conn).await?;
                let r = nm
                    .inner()
                    .set_property("WirelessEnabled", on)
                    .await
                    .map_err(|e| zbus::Error::FDO(Box::new(e)));
                settle(r, &NETWORK)
            }
            Op::Scan => {
                let conn = self.conn().await?;
                for (path, d) in self.devices(&conn).await? {
                    if nm::num(&d, "DeviceType") == nm::DEVICE_WIFI {
                        let w: NmWirelessProxy = nm::at(&conn, &path).await?;
                        // "Scanning not allowed" right after a scan is not worth a message.
                        let _ = w.request_scan(HashMap::new()).await;
                    }
                }
                Ok(())
            }
            Op::ConnectWifi {
                ap,
                device,
                password,
                all_users,
                eap,
            } => {
                self.connect_wifi(&ap, &device, password, all_users, eap)
                    .await
            }
            Op::ConnectHidden {
                ssid,
                security,
                password,
                all_users,
            } => {
                let conn = self.conn().await?;
                let device = self.first_wifi(&conn, false).await?;
                let sec = match security.as_str() {
                    "open" => Security::Open,
                    "password" => Security::Password { sae_only: false },
                    _ => return Err(failure("Choose Open or Password.")),
                };
                let uuid = profile::new_uuid();
                let mut settings = profile::wifi_settings(
                    &uuid,
                    &self.user,
                    &WifiInput {
                        ssid: ssid.trim().as_bytes().to_vec(),
                        security: Some(sec),
                        password,
                        all_users,
                        eap: None,
                    },
                )
                .map_err(failure)?;
                settings
                    .get_mut("802-11-wireless")
                    .expect("set by wifi_settings")
                    .insert("hidden".into(), profile::ov(true));
                self.started(&uuid);
                let nm = NetworkManagerProxy::new(&conn).await?;
                settle(
                    nm.add_and_activate_connection(&settings, &device, &nm::root())
                        .await,
                    &NETWORK,
                )
            }
            Op::Activate { uuid } => {
                let conn = self.conn().await?;
                let s = self.find(&conn, &uuid).await?;
                self.started(&uuid);
                let nm = NetworkManagerProxy::new(&conn).await?;
                settle(
                    nm.activate_connection(&s.path, &nm::root(), &nm::root())
                        .await,
                    &NETWORK,
                )
            }
            Op::Deactivate { uuid } => {
                let conn = self.conn().await?;
                let nm = NetworkManagerProxy::new(&conn).await?;
                for a in self.active(&conn).await? {
                    if a.uuid == uuid {
                        settle(nm.deactivate_connection(&a.path).await, &NETWORK)?;
                    }
                }
                Ok(())
            }
            Op::Forget { uuid } => {
                let conn = self.conn().await?;
                let s = self.find(&conn, &uuid).await?;
                let c: NmConnectionProxy = nm::at(&conn, &s.path).await?;
                settle(c.delete().await, &NETWORK)
            }
            Op::SetAutoconnect { uuid, on } => {
                self.edit(&uuid, |s| {
                    profile::set_autoconnect(s, on);
                    Ok(())
                })
                .await
            }
            Op::SetMetered { uuid, metered } => {
                self.edit(&uuid, |s| profile::set_metered(s, &metered))
                    .await
            }
            Op::SetEveryone { uuid, on } => {
                let user = self.user.clone();
                self.edit(&uuid, |s| {
                    s.entry("connection".into()).or_default().insert(
                        "permissions".into(),
                        profile::ov(profile::permissions(&user, on)),
                    );
                    Ok(())
                })
                .await
            }
            Op::SetIp { uuid, family, ip } => {
                self.edit(&uuid, |s| profile::apply_ip(s, family, &ip))
                    .await
            }
            Op::ImportWireGuard { path, all_users } => {
                self.import_wireguard(&path, all_users).await
            }
            Op::EditWireGuard { uuid, peer } => {
                self.edit(&uuid, |s| wireguard::edit_peer(s, &peer)).await
            }
            Op::Hotspot { on, name, password } => self.hotspot(on, &name, password).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let mut state = match sysbus::within(self.read()).await {
            Ok(v) => v,
            Err(e) => sysbus::read_error(&e, &NETWORK),
        };
        let (seq, text) = self.notice.lock().unwrap().clone();
        state["notice"] = json!({"seq": seq, "text": text});
        state["secret"] = self.shared.prompts.current();
        state["agent"] = json!(self.registered.load(std::sync::atomic::Ordering::SeqCst));
        Ok(state)
    }

    async fn watch(&self, changed: Arc<Notify>) {
        self.shared.prompts.follow(changed.clone());
        if let Err(e) = self.follow(&changed).await {
            eprintln!(
                "uq-settings: can't follow network changes: {}",
                sysbus::error_text(&e)
            );
        }
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests;
