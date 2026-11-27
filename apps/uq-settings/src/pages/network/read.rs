//! Reading NetworkManager: devices, access points, active and saved connections, and following
//! its signals.

use super::*;

/// A device's state in words.
pub(crate) fn state_text(state: u32, carrier: bool) -> &'static str {
    match state {
        nm::STATE_ACTIVATED => "Connected",
        40..=90 => "Connecting…",
        110 => "Disconnecting…",
        nm::STATE_FAILED => "Couldn't connect",
        nm::STATE_UNAVAILABLE if !carrier => "Cable unplugged",
        nm::STATE_UNAVAILABLE => "Not ready",
        10 => "Not managed here",
        _ => "Not connected",
    }
}

/// The band of a frequency in MHz.
pub(crate) fn band(mhz: u32) -> &'static str {
    match mhz {
        2400..=2500 => "2.4 GHz",
        4900..=5900 => "5 GHz",
        5925..=7125 => "6 GHz",
        _ => "",
    }
}

/// One access point as read.
pub(crate) struct Ap {
    pub(crate) path: OwnedObjectPath,
    pub(crate) device: OwnedObjectPath,
    pub(crate) ssid: Vec<u8>,
    pub(crate) strength: u8,
    pub(crate) security: Security,
    pub(crate) frequency: u32,
}

/// One active connection as read.
pub(crate) struct Active {
    pub(crate) path: OwnedObjectPath,
    pub(crate) uuid: String,
    pub(crate) state: u32,
    pub(crate) vpn: bool,
}

/// One saved connection as read.
pub(crate) struct Saved {
    pub(crate) path: OwnedObjectPath,
    pub(crate) settings: Settings,
}

impl NetworkState {
    pub(crate) async fn conn(&self) -> zbus::Result<zbus::Connection> {
        self.system.conn().await
    }

    pub(crate) async fn active(&self, conn: &zbus::Connection) -> zbus::Result<Vec<Active>> {
        let props = nm::get_all(conn, nm::NM_PATH, nm::IFACE_NM).await?;
        let mut out = Vec::new();
        for path in nm::paths(&props, "ActiveConnections") {
            // One may go away between the two reads.
            let Ok(a) = nm::get_all(conn, path.as_str(), nm::IFACE_ACTIVE).await else {
                continue;
            };
            out.push(Active {
                path,
                uuid: nm::text(&a, "Uuid"),
                state: nm::num(&a, "State"),
                vpn: nm::flag(&a, "Vpn"),
            });
        }
        Ok(out)
    }

    pub(crate) async fn saved(&self, conn: &zbus::Connection) -> zbus::Result<Vec<Saved>> {
        let settings = NmSettingsProxy::new(conn).await?;
        let mut out = Vec::new();
        for path in settings.list_connections().await? {
            let c: NmConnectionProxy = nm::at(conn, &path).await?;
            let Ok(s) = c.get_settings().await else {
                continue;
            };
            out.push(Saved { path, settings: s });
        }
        Ok(out)
    }

    pub(crate) async fn find(&self, conn: &zbus::Connection, uuid: &str) -> zbus::Result<Saved> {
        self.saved(conn)
            .await?
            .into_iter()
            .find(|s| profile::uuid(&s.settings) == uuid)
            .ok_or_else(|| failure("That connection is gone. It may have been removed elsewhere."))
    }

    pub(crate) async fn devices(
        &self,
        conn: &zbus::Connection,
    ) -> zbus::Result<Vec<(OwnedObjectPath, Props)>> {
        let nm = NetworkManagerProxy::new(conn).await?;
        let mut out = Vec::new();
        for path in nm.get_devices().await? {
            if let Ok(p) = nm::get_all(conn, path.as_str(), nm::IFACE_DEVICE).await {
                out.push((path, p));
            }
        }
        Ok(out)
    }

    pub(crate) async fn access_points(
        &self,
        conn: &zbus::Connection,
        device: &OwnedObjectPath,
    ) -> zbus::Result<(Props, Vec<Ap>)> {
        let w = nm::get_all(conn, device.as_str(), nm::IFACE_WIRELESS).await?;
        let mut aps = Vec::new();
        for path in nm::paths(&w, "AccessPoints") {
            let Ok(a) = nm::get_all(conn, path.as_str(), nm::IFACE_AP).await else {
                continue;
            };
            aps.push(Ap {
                path,
                device: device.clone(),
                ssid: nm::get::<Vec<u8>>(&a, "Ssid").unwrap_or_default(),
                strength: nm::get::<u8>(&a, "Strength").unwrap_or(0),
                security: profile::security(
                    nm::num(&a, "Flags"),
                    nm::num(&a, "WpaFlags"),
                    nm::num(&a, "RsnFlags"),
                ),
                frequency: nm::num(&a, "Frequency"),
            });
        }
        Ok((w, aps))
    }

    pub(crate) async fn read(&self) -> zbus::Result<Value> {
        let conn = self.conn().await?;
        let props = nm::get_all(&conn, nm::NM_PATH, nm::IFACE_NM).await?;
        let active = self.active(&conn).await?;
        let saved = self.saved(&conn).await?;
        let state_of = |uuid: &str| {
            active
                .iter()
                .find(|a| a.uuid == uuid)
                .map_or("off", |a| match a.state {
                    nm::ACTIVE_ACTIVATED => "on",
                    nm::ACTIVE_ACTIVATING => "starting",
                    _ => "stopping",
                })
        };

        let mut wifi_devices = Vec::new();
        let mut wired = Vec::new();
        let mut aps = Vec::new();
        let mut can_hotspot = false;
        let mut scanned = false;
        for (path, d) in self.devices(&conn).await? {
            let ty = nm::num(&d, "DeviceType");
            let state = nm::num(&d, "State");
            let iface = nm::text(&d, "Interface");
            match ty {
                nm::DEVICE_WIFI => {
                    let (w, mut list) = self.access_points(&conn, &path).await?;
                    can_hotspot |= nm::num(&w, "WirelessCapabilities") & nm::WIFI_CAP_AP != 0;
                    scanned |=
                        nm::get::<i64>(&w, "LastScan").unwrap_or(-1) >= 0 || !list.is_empty();
                    wifi_devices.push(json!({
                        "path": path.as_str(), "name": iface,
                        "state": state_text(state, true),
                    }));
                    aps.append(&mut list);
                }
                nm::DEVICE_ETHERNET => {
                    let carrier = nm::get_all(&conn, path.as_str(), nm::IFACE_WIRED)
                        .await
                        .ok();
                    let has_carrier = carrier.as_ref().is_some_and(|c| nm::flag(c, "Carrier"));
                    let speed = carrier.as_ref().map_or(0, |c| nm::num(c, "Speed"));
                    let ac = nm::path(&d, "ActiveConnection");
                    let using = active.iter().find(|a| a.path == ac).map(|a| a.uuid.clone());
                    wired.push(json!({
                        "path": path.as_str(), "name": iface,
                        "state": state_text(state, has_carrier),
                        "connected": state == nm::STATE_ACTIVATED,
                        "speed": if speed > 0 { format!("{speed} Mb/s") } else { String::new() },
                        "uuid": using,
                    }));
                }
                _ => {}
            }
        }

        // One row per network name: the strongest access point stands for it.
        aps.sort_by_key(|a| std::cmp::Reverse(a.strength));
        let mut networks: Vec<Value> = Vec::new();
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for ap in &aps {
            if ap.ssid.is_empty() || seen.contains(&ap.ssid) {
                continue;
            }
            seen.push(ap.ssid.clone());
            let saved_uuid = saved
                .iter()
                .find(|s| {
                    profile::kind(&s.settings) == "wifi" && profile::ssid(&s.settings) == ap.ssid
                })
                .map(|s| profile::uuid(&s.settings));
            let state = saved_uuid.as_deref().map_or("off", state_of);
            networks.push(json!({
                "ssid": nm::ssid_text(&ap.ssid),
                "strength": ap.strength,
                "bars": match ap.strength { 0..=24 => 1, 25..=49 => 2, 50..=74 => 3, _ => 4 },
                "security": ap.security.id(),
                "band": band(ap.frequency),
                "ap": ap.path.as_str(),
                "device": ap.device.as_str(),
                "saved": saved_uuid,
                "state": state,
            }));
        }
        networks.sort_by_key(|n| {
            (
                n["state"] == "off",
                n["saved"].is_null(),
                std::cmp::Reverse(n["strength"].as_u64().unwrap_or(0)),
            )
        });

        let mut list = Vec::new();
        let mut hotspot = json!({"can": can_hotspot, "state": "off", "uuid": null, "name": ""});
        for s in &saved {
            let k = profile::kind(&s.settings);
            if k == "loopback" {
                continue;
            }
            let mut j = profile::summary(s.path.as_str(), &s.settings);
            let st = state_of(&profile::uuid(&s.settings));
            j["state"] = json!(st);
            if k == "hotspot" && (hotspot["uuid"].is_null() || st != "off") {
                hotspot["uuid"] = j["uuid"].clone();
                hotspot["state"] = json!(st);
                hotspot["name"] = j["ssid"].clone();
            }
            list.push(j);
        }
        list.sort_by_key(|j| {
            (
                j["state"] == "off",
                j["kind"].as_str().unwrap_or("").to_string(),
                j["name"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        let vpn_on = active
            .iter()
            .any(|a| a.vpn && a.state == nm::ACTIVE_ACTIVATED);

        Ok(json!({
            "available": true,
            "wifi": {
                "enabled": nm::flag(&props, "WirelessEnabled"),
                "hardware": nm::get::<bool>(&props, "WirelessHardwareEnabled").unwrap_or(true),
                "devices": wifi_devices,
                "networks": networks,
                "scanned": scanned,
            },
            "wired": wired,
            "saved": list,
            "vpn_on": vpn_on,
            "openvpn": self.openvpn(),
            "hotspot": hotspot,
        }))
    }

    /// Register the secret agent on `conn` (once its object is served).
    pub(crate) async fn register(&self, conn: &zbus::Connection) -> bool {
        let ok = async {
            let m = AgentManagerProxy::new(conn).await?;
            m.register_with_capabilities(nm::AGENT_ID, 0).await
        };
        let ok = sysbus::within(ok).await.is_ok();
        self.registered
            .store(ok, std::sync::atomic::Ordering::SeqCst);
        ok
    }

    /// Serve the agent, register it, and follow NetworkManager's signals while the page is
    /// open. A device that fails while something started here was connecting shows why.
    pub(crate) async fn follow(&self, changed: &Notify) -> zbus::Result<()> {
        let conn = self.conn().await?;
        conn.object_server()
            .at(
                nm::AGENT_PATH,
                agent::SecretAgent {
                    shared: self.shared.clone(),
                },
            )
            .await?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path_namespace(nm::NM_PATH)
            .build()?;
        let mut signals =
            zbus::MessageStream::for_match_rule(rule, &conn, std::num::NonZeroUsize::new(256))
                .await?;
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let mut owners = dbus
            .receive_name_owner_changed_with_args(&[(0, nm::NM)])
            .await?;
        self.register(&conn).await;
        changed.notify_one();
        loop {
            tokio::select! {
                Some(msg) = signals.next() => {
                    let Ok(msg) = msg else { continue };
                    let h = msg.header();
                    if h.member().is_some_and(|m| m.as_str() == "StateChanged")
                        && h.interface().is_some_and(|i| i.as_str() == nm::IFACE_DEVICE)
                        && let Ok((new, _old, reason)) = msg.body().deserialize::<(u32, u32, u32)>()
                        && new == nm::STATE_FAILED
                        && !self.shared.started.lock().unwrap().is_empty()
                    {
                        self.notice(&format!("Couldn't connect: {}", reason_text(reason)));
                    }
                    changed.notify_one();
                }
                Some(owner) = owners.next() => {
                    // NetworkManager restarted: it forgot its agents.
                    if owner.args().is_ok_and(|a| a.new_owner().is_some()) {
                        self.register(&conn).await;
                    } else {
                        self.registered.store(false, std::sync::atomic::Ordering::SeqCst);
                    }
                    changed.notify_one();
                }
                else => return Ok(()),
            }
        }
    }
}
