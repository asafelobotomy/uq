//! Doing things: joining Wi-Fi, the hotspot, importing WireGuard and answering the agent.

use super::*;

impl NetworkState {
    /// Mark `uuid` as started from the page: the agent may ask for its secrets.
    pub(crate) fn started(&self, uuid: &str) {
        self.shared.started.lock().unwrap().insert(uuid.to_string());
    }

    pub(crate) async fn connect_wifi(
        &self,
        ap: &str,
        device: &str,
        password: String,
        all_users: bool,
        eap: Option<Eap>,
    ) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let ap = OwnedObjectPath::try_from(ap)
            .map_err(|_| failure("That network is gone. Scan again."))?;
        let device =
            OwnedObjectPath::try_from(device).map_err(|_| failure("That Wi-Fi device is gone."))?;
        let a = nm::get_all(&conn, ap.as_str(), nm::IFACE_AP)
            .await
            .map_err(|_| failure("That network is gone. Scan again."))?;
        let ssid = nm::get::<Vec<u8>>(&a, "Ssid").unwrap_or_default();
        let sec = profile::security(
            nm::num(&a, "Flags"),
            nm::num(&a, "WpaFlags"),
            nm::num(&a, "RsnFlags"),
        );
        let nm = NetworkManagerProxy::new(&conn).await?;
        let existing =
            self.saved(&conn).await?.into_iter().find(|s| {
                profile::kind(&s.settings) == "wifi" && profile::ssid(&s.settings) == ssid
            });
        if let Some(s) = existing {
            self.started(&profile::uuid(&s.settings));
            return settle(
                nm.activate_connection(&s.path, &device, &ap).await,
                &NETWORK,
            );
        }
        let uuid = profile::new_uuid();
        let settings = profile::wifi_settings(
            &uuid,
            &self.user,
            &WifiInput {
                ssid,
                security: Some(sec),
                password,
                all_users,
                eap,
            },
        )
        .map_err(failure)?;
        self.started(&uuid);
        settle(
            nm.add_and_activate_connection(&settings, &device, &ap)
                .await,
            &NETWORK,
        )
    }

    pub(crate) async fn first_wifi(
        &self,
        conn: &zbus::Connection,
        need_ap: bool,
    ) -> zbus::Result<OwnedObjectPath> {
        for (path, d) in self.devices(conn).await? {
            if nm::num(&d, "DeviceType") != nm::DEVICE_WIFI {
                continue;
            }
            if need_ap {
                let w = nm::get_all(conn, path.as_str(), nm::IFACE_WIRELESS).await?;
                if nm::num(&w, "WirelessCapabilities") & nm::WIFI_CAP_AP == 0 {
                    continue;
                }
            }
            return Ok(path);
        }
        Err(failure(if need_ap {
            "This computer's Wi-Fi can't share a connection."
        } else {
            "This computer has no Wi-Fi."
        }))
    }

    pub(crate) async fn hotspot(&self, on: bool, name: &str, password: String) -> zbus::Result<()> {
        let conn = self.conn().await?;
        let nm = NetworkManagerProxy::new(&conn).await?;
        let saved = self.saved(&conn).await?;
        let mine = saved
            .into_iter()
            .find(|s| profile::kind(&s.settings) == "hotspot");
        if !on {
            let Some(s) = mine else { return Ok(()) };
            let uuid = profile::uuid(&s.settings);
            for a in self.active(&conn).await? {
                if a.uuid == uuid {
                    settle(nm.deactivate_connection(&a.path).await, &NETWORK)?;
                }
            }
            return Ok(());
        }
        let device = self.first_wifi(&conn, true).await?;
        match mine {
            Some(mut s) => {
                let uuid = profile::uuid(&s.settings);
                if !password.is_empty() {
                    let name = if name.trim().is_empty() {
                        nm::ssid_text(&profile::ssid(&s.settings))
                    } else {
                        name.to_string()
                    };
                    let mut fresh = profile::hotspot_settings(&uuid, &self.user, &name, &password)
                        .map_err(failure)?;
                    for key in ["802-11-wireless", "802-11-wireless-security"] {
                        if let Some(block) = fresh.remove(key) {
                            s.settings.insert(key.into(), block);
                        }
                    }
                    let c: NmConnectionProxy = nm::at(&conn, &s.path).await?;
                    let r = c.update(&s.settings).await;
                    let ok = r.is_ok();
                    settle(r, &NETWORK)?;
                    if !ok {
                        return Ok(());
                    }
                }
                self.started(&uuid);
                settle(
                    nm.activate_connection(&s.path, &device, &nm::root()).await,
                    &NETWORK,
                )
            }
            None => {
                let uuid = profile::new_uuid();
                let settings = profile::hotspot_settings(&uuid, &self.user, name, &password)
                    .map_err(failure)?;
                self.started(&uuid);
                settle(
                    nm.add_and_activate_connection(&settings, &device, &nm::root())
                        .await,
                    &NETWORK,
                )
            }
        }
    }

    pub(crate) async fn import_wireguard(&self, path: &Path, all_users: bool) -> zbus::Result<()> {
        let meta = std::fs::metadata(path)
            .map_err(|e| failure(format!("Can't read {}: {e}", path.display())))?;
        if !meta.is_file() || meta.len() > MAX_IMPORT {
            return Err(failure(format!(
                "{} isn't a WireGuard config file.",
                path.display()
            )));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| failure(format!("Can't read {}: {e}", path.display())))?;
        let cfg = wireguard::parse(&text).map_err(failure)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "WireGuard".into());
        let uuid = profile::new_uuid();
        let settings =
            wireguard::settings(&cfg, &name, &uuid, &self.user, all_users).map_err(failure)?;
        let conn = self.conn().await?;
        let s = NmSettingsProxy::new(&conn).await?;
        let r = s.add_connection(&settings).await;
        let ok = r.is_ok();
        settle(r, &NETWORK)?;
        if ok {
            let mut text = format!("Added {name}. Start it from the list below.");
            if !cfg.left_out.is_empty() {
                text.push_str(&format!(
                    " These lines of the file were left out because only the command-line tool uses them: {}.",
                    cfg.left_out.join(", ")
                ));
            }
            self.notice(&text);
        }
        Ok(())
    }

    /// Answer the question on show; a stale one (already answered or cancelled) does nothing.
    pub(crate) fn answer(&self, id: u64, a: Answer) {
        self.shared.prompts.answer(id, a);
    }
}
