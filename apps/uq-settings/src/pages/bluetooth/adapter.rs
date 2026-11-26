//! Talking to BlueZ: the adapter, devices, pairing and connecting.

use super::*;

impl BluetoothState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system())
    }

    pub fn with(system: SystemBus) -> Self {
        Self {
            system,
            write: Mutex::new(()),
            shared: Arc::new(Shared::default()),
            tasks: StdMutex::new(Vec::new()),
            registered: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(crate) async fn objects(&self, conn: &zbus::Connection) -> zbus::Result<Objects> {
        let om = zbus::fdo::ObjectManagerProxy::builder(conn)
            .destination(BLUEZ)
            .path("/")
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        Ok(om.get_managed_objects().await?)
    }

    /// The first adapter's path.
    pub(crate) async fn adapter(&self, conn: &zbus::Connection) -> zbus::Result<OwnedObjectPath> {
        let objects = self.objects(conn).await?;
        first_adapter(&objects)
            .map(|(p, _)| p.clone())
            .ok_or_else(|| failure("This computer has no Bluetooth adapter."))
    }

    pub(crate) async fn proxy(
        &self,
        conn: &zbus::Connection,
        path: &str,
        iface: &'static str,
    ) -> zbus::Result<zbus::Proxy<'static>> {
        zbus::proxy::Builder::<zbus::Proxy<'static>>::new(conn)
            .destination(BLUEZ)
            .path(path.to_string())
            .interface(iface)
            .cache_properties(CacheProperties::No)
            .build()
            .await
    }

    pub(crate) async fn set(
        &self,
        path: &str,
        iface: &'static str,
        prop: &str,
        on: bool,
    ) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let p = self.proxy(&conn, path, iface).await?;
        p.set_property(prop, on)
            .await
            .map_err(|e| zbus::Error::FDO(Box::new(e)))
    }

    pub(crate) async fn read(&self) -> zbus::Result<Value> {
        let conn = self.system.conn().await?;
        let objects = self.objects(&conn).await?;
        // From the one snapshot: an adapter that went between two reads can't be missing here.
        let Some((adapter, a)) = first_adapter(&objects) else {
            return Ok(json!({"available": true, "adapter": null, "devices": []}));
        };
        let adapter = adapter.clone();
        let working = self.shared.working.lock().unwrap().clone();
        let mut devices = Vec::new();
        for (path, o) in &objects {
            let Some(d) = props(o, DEVICE) else { continue };
            if get::<OwnedObjectPath>(d, "Adapter").as_ref() != Some(&adapter) {
                continue;
            }
            let paired = get::<bool>(d, "Paired").unwrap_or(false);
            let name = get::<String>(d, "Name");
            // A device that doesn't say its name is noise unless it is already paired.
            if name.is_none() && !paired {
                continue;
            }
            let alias = get::<String>(d, "Alias")
                .or(name)
                .unwrap_or_else(|| "Unknown device".into());
            let icon = get::<String>(d, "Icon").unwrap_or_default();
            devices.push(json!({
                "path": path.as_str(),
                "name": alias,
                "icon": if icon.is_empty() { "bluetooth".to_string() } else { icon.clone() },
                "kind": kind_text(&icon),
                "paired": paired,
                "trusted": get::<bool>(d, "Trusted").unwrap_or(false),
                "connected": get::<bool>(d, "Connected").unwrap_or(false),
                "battery": props(o, BATTERY).and_then(|b| get::<u8>(b, "Percentage")),
                "working": working.get(path.as_str()).copied(),
            }));
        }
        devices.sort_by_key(|d| {
            (
                d["connected"] != true,
                d["paired"] != true,
                d["name"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        Ok(json!({
            "available": true,
            "adapter": {
                "path": adapter.as_str(),
                "name": get::<String>(a, "Alias").unwrap_or_default(),
                "powered": get::<bool>(a, "Powered").unwrap_or(false),
                "discoverable": get::<bool>(a, "Discoverable").unwrap_or(false),
                "discovering": get::<bool>(a, "Discovering").unwrap_or(false),
            },
            "devices": devices,
        }))
    }

    /// Run a device call that waits on the device in the background, marking it `what`.
    pub(crate) async fn background(
        &self,
        path: String,
        what: &'static str,
        method: &'static str,
    ) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let shared = self.shared.clone();
        shared.working.lock().unwrap().insert(path.clone(), what);
        let task = async move {
            let r = async {
                zbus::proxy::Builder::<zbus::Proxy<'static>>::new(&conn)
                    .destination(BLUEZ)
                    .path(path.clone())
                    .interface(DEVICE)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?
                    .call_method(method, &())
                    .await
                    .map(|_| ())
            }
            .await;
            if let Err(e) = r
                && let Some(text) = error_words(&e)
            {
                shared.notice(&text);
            }
            shared.working.lock().unwrap().remove(&path);
            *shared.display.lock().unwrap() = None;
            shared.ping();
        };
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|t| !t.is_finished());
        tasks.push(tokio::spawn(task));
        Ok(())
    }

    /// Register the agent with BlueZ, as the default one.
    pub(crate) async fn register(&self, conn: &zbus::Connection) -> bool {
        let r = async {
            let m = self
                .proxy(conn, "/org/bluez", "org.bluez.AgentManager1")
                .await?;
            let path = OwnedObjectPath::try_from(AGENT_PATH).expect("a valid path");
            m.call_method("RegisterAgent", &(&path, "KeyboardDisplay"))
                .await?;
            m.call_method("RequestDefaultAgent", &(&path,)).await?;
            zbus::Result::Ok(())
        };
        let ok = sysbus::within(r).await.is_ok();
        self.registered
            .store(ok, std::sync::atomic::Ordering::SeqCst);
        ok
    }

    pub(crate) async fn follow(&self, changed: &Notify) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        conn.object_server()
            .at(
                AGENT_PATH,
                agent::PairingAgent {
                    shared: self.shared.clone(),
                },
            )
            .await?;
        let devices = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path_namespace("/org/bluez")
            .build()?;
        let added = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path("/")
            .interface("org.freedesktop.DBus.ObjectManager")
            .build()?;
        let mut signals = futures_util::stream::select(
            zbus::MessageStream::for_match_rule(devices, &conn, std::num::NonZeroUsize::new(256))
                .await?,
            zbus::MessageStream::for_match_rule(added, &conn, std::num::NonZeroUsize::new(64))
                .await?,
        );
        let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
        let mut owners = dbus
            .receive_name_owner_changed_with_args(&[(0, BLUEZ)])
            .await?;
        self.register(&conn).await;
        changed.notify_one();
        loop {
            tokio::select! {
                Some(_) = signals.next() => changed.notify_one(),
                Some(owner) = owners.next() => {
                    // BlueZ restarted: it forgot its agents.
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
