//! The fake NetworkManager's saved connections, their settings and the secret agent manager.

use super::*;

pub(crate) struct SettingsIface {
    pub(crate) s: Shared<NmState>,
    pub(crate) deny: Deny,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl SettingsIface {
    pub(crate) fn list_connections(&self) -> Vec<OwnedObjectPath> {
        self.s
            .lock()
            .unwrap()
            .conns
            .iter()
            .map(|c| OwnedObjectPath::try_from(c.path.as_str()).unwrap())
            .collect()
    }

    pub(crate) async fn add_connection(
        &self,
        connection: Settings,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<OwnedObjectPath, NmError> {
        if self.deny.on() {
            return Err(denied());
        }
        add(&self.s, server, connection).await
    }
}

pub(crate) struct ConnIface {
    pub(crate) s: Shared<NmState>,
    pub(crate) path: String,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl ConnIface {
    pub(crate) fn get_settings(&self) -> Result<Settings, NmError> {
        let st = self.s.lock().unwrap();
        st.conns
            .iter()
            .find(|c| c.path == self.path)
            .map(|c| without_secrets(&c.settings))
            .ok_or_else(|| NmError::UnknownConnection("gone".into()))
    }

    pub(crate) fn update(&self, properties: Settings) -> Result<(), NmError> {
        let mut st = self.s.lock().unwrap();
        let c = st
            .conns
            .iter_mut()
            .find(|c| c.path == self.path)
            .ok_or_else(|| NmError::UnknownConnection("gone".into()))?;
        // Secrets the update leaves out are kept.
        let mut next = properties;
        for (block, key) in SECRETS {
            if key.contains('/') {
                continue;
            }
            if let Some(v) = c.settings.get(block).and_then(|b| b.get(key)) {
                let b = next.entry(block.to_string()).or_default();
                if !b.contains_key(key) {
                    b.insert(key.to_string(), v.try_clone().unwrap());
                }
            }
        }
        c.settings = next;
        Ok(())
    }

    pub(crate) async fn delete(
        &self,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(), NmError> {
        self.s.lock().unwrap().conns.retain(|c| c.path != self.path);
        let _ = server.remove::<ConnIface, _>(self.path.as_str()).await;
        Ok(())
    }
}

pub(crate) struct AgentManagerIface {
    pub(crate) s: Shared<NmState>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AgentManager")]
impl AgentManagerIface {
    pub(crate) fn register_with_capabilities(
        &self,
        _identifier: String,
        _capabilities: u32,
        #[zbus(header)] hdr: Header<'_>,
    ) {
        self.s.lock().unwrap().agent = hdr.sender().map(|s| s.to_string());
    }

    pub(crate) fn unregister(&self) {
        self.s.lock().unwrap().agent = None;
    }
}
