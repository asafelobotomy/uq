//! Signal subscriptions: matching each widget's declared signals, following bus owners, and
//! forwarding signals to the shell.

use super::*;

/// What a running signal subscription was made for; a different key restarts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubKey {
    pub(crate) spec: DbusSpec,
    pub(crate) owner: String,
}

pub(crate) async fn owner_of(proxy: &DBusProxy<'_>, name: &str) -> Option<String> {
    let bus_name = zbus::names::BusName::try_from(name).ok()?;
    proxy
        .get_name_owner(bus_name)
        .await
        .ok()
        .map(|o| o.to_string())
}

/// A signal's arguments as a JSON array, through the same type allowlist as calls.
pub(crate) fn signal_args(msg: &zbus::Message) -> Result<Value, DbusJsonError> {
    let body = msg.body();
    if body.len() > MAX_RESULT_JSON {
        return Err(DbusJsonError::ResultTooLarge);
    }
    let sig = body.signature().to_string();
    if body.is_empty() || sig.is_empty() {
        return Ok(json!([]));
    }
    let inner = sig
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(&sig)
        .to_string();
    parse_signature(&inner)?;
    let fields: Structure<'_> = body
        .deserialize()
        .map_err(|e| DbusJsonError::Call(e.to_string()))?;
    let values: Vec<Value> = fields
        .fields()
        .iter()
        .map(value_to_json)
        .collect::<Result<_, _>>()?;
    Ok(Value::Array(values))
}

pub(crate) fn rule(spec: &DbusSpec, props: bool) -> zbus::Result<MatchRule<'static>> {
    let b = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(spec.name.clone())
        .path(spec.path.clone());
    if props {
        b.interface(PROPERTIES)
            .member("PropertiesChanged")
            .arg(0, spec.interface.clone())
            .build()
    } else {
        b.interface(spec.interface.clone()).build()
    }
}

pub(crate) async fn next_msg(s: &mut Option<MessageStream>) -> Option<zbus::Result<zbus::Message>> {
    match s.as_mut() {
        Some(s) => s.next().await,
        None => std::future::pending().await,
    }
}

/// Forward a widget's listed signals as `WidgetSignal` and its listed properties as
/// `WidgetProps`, at most [`PROPS_EVERY`]. Only messages from the name's current owner count.
/// Properties are followed only while `shown` (see [`Widgets::set_shown`]); coming back on
/// screen re-reads them all, so the shell catches up at once.
pub(crate) async fn signal_loop(
    tx: mpsc::Sender<Outbound>,
    conn: zbus::Connection,
    widget: String,
    key: SubKey,
    mut shown: watch::Receiver<bool>,
) {
    let spec = &key.spec;
    let open = |props: bool| {
        let conn = conn.clone();
        async move {
            let r = rule(spec, props).ok()?;
            MessageStream::for_match_rule(r, &conn, std::num::NonZeroUsize::new(64))
                .await
                .ok()
        }
    };
    let want_props = !spec.properties.is_empty();
    let mut on = *shown.borrow_and_update();
    // The sender went away (the bridge is stopping): stop listening for changes.
    let mut shown_closed = false;
    let mut props = if want_props && on {
        open(true).await
    } else {
        None
    };
    let mut sigs = if spec.signals.is_empty() {
        None
    } else {
        open(false).await
    };
    let mut pending = Map::new();
    let mut last: Option<Instant> = None;
    let mut warned = false;
    loop {
        let due = last.map(|t| t + PROPS_EVERY);
        let waiting = !pending.is_empty();
        let flush = async move {
            match due {
                Some(at) if waiting => tokio::time::sleep_until(at).await,
                _ if waiting => {}
                _ => std::future::pending().await,
            }
        };
        let shown_changed = async {
            if shown_closed {
                std::future::pending::<bool>().await
            } else {
                shown.changed().await.is_ok()
            }
        };
        tokio::select! {
            ok = shown_changed => {
                if !ok {
                    shown_closed = true;
                    continue;
                }
                let now = *shown.borrow_and_update();
                if now == on || !want_props {
                    on = now;
                    continue;
                }
                on = now;
                if !on {
                    // Unsubscribe; what was waiting is re-read when it comes back.
                    props = None;
                    pending.clear();
                    continue;
                }
                props = open(true).await;
                if let Ok(Value::Object(all)) =
                    dbus_json::get_properties(&conn, &target(spec), &spec.properties, false).await
                {
                    pending.extend(all);
                    last = None;
                }
            }
            m = next_msg(&mut props) => {
                let Some(Ok(m)) = m else { props = None; continue };
                if m.header().sender().map(|s| s.as_str()) != Some(key.owner.as_str()) {
                    continue;
                }
                let Ok((_, changed, invalidated)) = m
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                else {
                    continue;
                };
                for p in &spec.properties {
                    if let Some(v) = changed.get(p) {
                        match value_to_json(v) {
                            Ok(j) => {
                                pending.insert(p.clone(), j);
                            }
                            Err(e) if !warned => {
                                warned = true;
                                eprintln!("uqctl bridge: widget {widget} property {p}: {e}");
                            }
                            Err(_) => {}
                        }
                    } else if invalidated.iter().any(|i| i == p)
                        && let Ok(j) = dbus_json::get_property(&conn, &target(spec), p, false).await
                    {
                        pending.insert(p.clone(), j);
                    }
                }
            }
            m = next_msg(&mut sigs) => {
                let Some(Ok(m)) = m else { sigs = None; continue };
                let h = m.header();
                if h.sender().map(|s| s.as_str()) != Some(key.owner.as_str()) {
                    continue;
                }
                let Some(member) = h.member().map(|m| m.to_string()) else { continue };
                if !spec.signals.contains(&member) {
                    continue;
                }
                match signal_args(&m) {
                    Ok(args) => {
                        let ev = Outbound::Event {
                            event: "WidgetSignal".into(),
                            data: json!({"widget": widget, "member": member, "args": args}),
                        };
                        if tx.send(ev).await.is_err() {
                            return;
                        }
                    }
                    Err(e) if !warned => {
                        warned = true;
                        eprintln!("uqctl bridge: widget {widget} signal {member}: {e}");
                    }
                    Err(_) => {}
                }
            }
            () = flush => {
                last = Some(Instant::now());
                let props = Value::Object(std::mem::take(&mut pending));
                let ev = Outbound::Event {
                    event: "WidgetProps".into(),
                    data: json!({"widget": widget, "props": props}),
                };
                if tx.send(ev).await.is_err() {
                    return;
                }
            }
        }
        // Nothing left to follow, unless properties are only paused while off screen.
        if props.is_none() && sigs.is_none() && pending.is_empty() && (on || !want_props) {
            return;
        }
    }
}

impl Widgets {
    /// One subscription per visible widget whose `[dbus]` lists properties or signals and whose
    /// name has an owner.
    pub(crate) async fn reconcile_subs(self: &Arc<Self>) {
        let want: HashMap<String, SubKey> = {
            let inner = lock(&self.inner);
            inner
                .widgets
                .values()
                .filter(|k| is_visible(k, &inner.owners))
                .filter_map(|k| {
                    let d = k.manifest.dbus.as_ref()?;
                    if d.properties.is_empty() && d.signals.is_empty() {
                        return None;
                    }
                    let owner = inner.owners.get(&(d.bus, d.name.clone()))?.clone()?;
                    Some((
                        k.manifest.id.clone(),
                        SubKey {
                            spec: d.clone(),
                            owner,
                        },
                    ))
                })
                .collect()
        };
        let start: Vec<(String, SubKey)> = {
            let mut inner = lock(&self.inner);
            inner.subs.retain(|id, (key, h)| {
                let keep = want.get(id) == Some(key);
                if !keep {
                    h.abort();
                }
                keep
            });
            want.into_iter()
                .filter(|(id, _)| !inner.subs.contains_key(id))
                .collect()
        };
        for (id, key) in start {
            let Some(conn) = self.conn(key.spec.bus).get().await else {
                continue;
            };
            let (tx, stop, wid, k) = (self.tx.clone(), self.stop.clone(), id.clone(), key.clone());
            let shown = lock(&self.inner).shown_rx(&id);
            let task = tokio::spawn(async move {
                tokio::select! {
                    _ = stop.cancelled() => {}
                    _ = signal_loop(tx, conn, wid, k, shown) => {}
                }
            });
            let mut inner = lock(&self.inner);
            // A newer reconcile may have started one meanwhile; keep only one.
            if let Some((_, old)) = inner.subs.insert(id, (key, task.abort_handle())) {
                old.abort();
            }
        }
    }

    /// Start following `NameOwnerChanged` on `bus`, once.
    pub(crate) fn ensure_watch(self: &Arc<Self>, bus: Bus) {
        if !lock(&self.inner).watching.insert(bus) {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let stop = me.stop.clone();
            tokio::select! {
                _ = stop.cancelled() => {}
                _ = me.owner_loop(bus) => {}
            }
        });
    }

    pub(crate) async fn owner_loop(self: Arc<Self>, bus: Bus) {
        loop {
            if let Some(conn) = self.conn(bus).get().await
                && let Ok(proxy) = DBusProxy::new(&conn).await
                && let Ok(mut changes) = proxy.receive_name_owner_changed().await
            {
                // Subscribed first, so a change in between is not lost.
                let names: Vec<String> = lock(&self.inner)
                    .owners
                    .keys()
                    .filter(|(b, _)| *b == bus)
                    .map(|(_, n)| n.clone())
                    .collect();
                for n in names {
                    let owner = owner_of(&proxy, &n).await;
                    if let Some(slot) = lock(&self.inner).owners.get_mut(&(bus, n)) {
                        *slot = owner;
                    }
                }
                self.changed().await;
                while let Some(sig) = changes.next().await {
                    let Ok(args) = sig.args() else { continue };
                    let key = (bus, args.name().to_string());
                    let owner = args.new_owner().as_ref().map(|o| o.to_string());
                    let hit = {
                        let mut inner = lock(&self.inner);
                        match inner.owners.get_mut(&key) {
                            Some(slot) if *slot != owner => {
                                *slot = owner;
                                true
                            }
                            _ => false,
                        }
                    };
                    if hit {
                        self.changed().await;
                    }
                }
            }
            self.conn(bus).reset().await;
            tokio::time::sleep(RETRY).await;
        }
    }
}
