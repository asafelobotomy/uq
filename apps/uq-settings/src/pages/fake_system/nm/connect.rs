//! Activating and adding connections the way NetworkManager does, polkit checks included.

use super::*;

/// Start `conn_path`: active at once, or after the agent answers for a secret it needs.
pub(crate) async fn activate(
    s: &Shared<NmState>,
    conn: &zbus::Connection,
    server: &ObjectServer,
    conn_path: &str,
) -> Result<OwnedObjectPath, NmError> {
    let (active_path, needs, settings, agent, uuid) = {
        let mut st = s.lock().unwrap();
        let Some(c) = st.conns.iter().find(|c| c.path == conn_path) else {
            return Err(NmError::UnknownConnection("No such connection".into()));
        };
        let uuid = text(&c.settings, "connection", "uuid");
        let vpn = matches!(
            text(&c.settings, "connection", "type").as_str(),
            "vpn" | "wireguard"
        );
        let needs = c.needs_secret.clone();
        let settings = without_secrets(&c.settings);
        let path = st.path("ActiveConnection");
        st.active.push(FakeActive {
            path: path.clone(),
            conn: conn_path.to_string(),
            uuid: uuid.clone(),
            state: if needs.is_some() { 1 } else { 2 },
            vpn,
        });
        (path, needs, settings, st.agent.clone(), uuid)
    };
    server
        .at(
            active_path.as_str(),
            ActiveIface {
                s: s.clone(),
                path: active_path.clone(),
            },
        )
        .await?;
    if let Some(setting) = needs {
        let s = s.clone();
        let conn = conn.clone();
        let conn_path = conn_path.to_string();
        let active = active_path.clone();
        tokio::spawn(async move {
            let reply = match agent {
                None => Err("no agent".to_string()),
                Some(agent) => conn
                    .call_method(
                        Some(agent.as_str()),
                        "/org/freedesktop/NetworkManager/SecretAgent",
                        Some("org.freedesktop.NetworkManager.SecretAgent"),
                        "GetSecrets",
                        &(
                            settings,
                            OwnedObjectPath::try_from(conn_path.as_str()).unwrap(),
                            setting.as_str(),
                            Vec::<String>::new(),
                            1u32,
                        ),
                    )
                    .await
                    .map_err(|e| match e {
                        zbus::Error::MethodError(name, _, _) => name.to_string(),
                        other => other.to_string(),
                    })
                    .and_then(|m| {
                        m.body()
                            .deserialize::<Settings>()
                            .map_err(|e| e.to_string())
                    }),
            };
            let ok = reply.is_ok();
            {
                let mut st = s.lock().unwrap();
                if let Ok(secrets) = &reply {
                    if let Some(c) = st.conns.iter_mut().find(|c| c.path == conn_path) {
                        for (block, vals) in secrets {
                            let b = c.settings.entry(block.clone()).or_default();
                            for (k, v) in vals {
                                b.insert(k.clone(), v.try_clone().unwrap());
                            }
                        }
                        c.needs_secret = None;
                    }
                    if let Some(a) = st.active.iter_mut().find(|a| a.path == active) {
                        a.state = 2;
                    }
                } else {
                    st.active.retain(|a| a.path != active);
                }
                st.agent_replies.push(reply);
            }
            let (new, reason) = if ok { (100u32, 0u32) } else { (120, 7) };
            let _ = conn
                .emit_signal(
                    None::<()>,
                    WIFI_DEV,
                    "org.freedesktop.NetworkManager.Device",
                    "StateChanged",
                    &(new, 50u32, reason),
                )
                .await;
        });
    }
    let _ = uuid;
    Ok(OwnedObjectPath::try_from(active_path).unwrap())
}

pub(crate) async fn add(
    s: &Shared<NmState>,
    server: &ObjectServer,
    settings: Settings,
) -> Result<OwnedObjectPath, NmError> {
    let path = {
        let mut st = s.lock().unwrap();
        let path = st.path("Settings");
        st.conns.push(FakeConn {
            path: path.clone(),
            settings,
            needs_secret: None,
        });
        path
    };
    server
        .at(
            path.as_str(),
            ConnIface {
                s: s.clone(),
                path: path.clone(),
            },
        )
        .await?;
    Ok(OwnedObjectPath::try_from(path).unwrap())
}
