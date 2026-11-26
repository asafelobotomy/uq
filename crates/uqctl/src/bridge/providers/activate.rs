//! Starting providers when the launcher opens, throttled, and activating a chosen result.

use super::*;

/// When each activatable provider was last started and when its failure was last logged.
#[derive(Debug, Default)]
pub(crate) struct StartThrottle {
    pub(crate) tried: HashMap<String, Instant>,
    pub(crate) logged: HashMap<String, Instant>,
}

pub(crate) fn due(
    map: &mut HashMap<String, Instant>,
    name: &str,
    now: Instant,
    every: Duration,
) -> bool {
    if map
        .get(name)
        .is_some_and(|t| now.saturating_duration_since(*t) < every)
    {
        return false;
    }
    map.insert(name.to_string(), now);
    true
}

impl StartThrottle {
    /// Whether `name` may be started now (at most once per [`START_EVERY`]); records the try.
    pub(crate) fn try_start(&mut self, name: &str, now: Instant) -> bool {
        due(&mut self.tried, name, now, START_EVERY)
    }

    /// Whether a failure of `name` should be logged now (at most once per [`START_LOG_EVERY`]).
    pub(crate) fn should_log(&mut self, name: &str, now: Instant) -> bool {
        due(&mut self.logged, name, now, START_LOG_EVERY)
    }
}

impl Providers {
    /// The bridge's `LauncherOpened` call (look=see>do_ or the app menu opened): start each
    /// switched-on `Activatable=true` provider whose name has no owner and has a session service
    /// file, at most once per 30 s each, without waiting (§2.4.1). Queries still go only to
    /// owned names, with `NO_AUTO_START`.
    pub(crate) async fn launcher_opened(
        self: &Arc<Self>,
        link: &Link,
        args: &[Value],
    ) -> Result<Value, String> {
        if !args.is_empty() {
            return Err("LauncherOpened takes no arguments".into());
        }
        let prefs = provider_prefs(link);
        if provider_enabled(&prefs, "files") {
            self.refresh_files();
        }
        if provider_enabled(&prefs, "settings") {
            self.refresh_settings();
        }
        let defs: Vec<ProviderDef> = lock(&self.defs)
            .current()
            .iter()
            .filter(|d| d.activatable && provider_enabled(&prefs, &d.bus_name))
            .cloned()
            .collect();
        if defs.is_empty() {
            return Ok(Value::Null);
        }
        let Some(conn) = link.connection().await else {
            return Ok(Value::Null);
        };
        let me = self.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = me.stop.cancelled() => {}
                _ = me.start_cold(&conn, defs) => {}
            }
        });
        Ok(Value::Null)
    }

    pub(crate) async fn start_cold(
        self: &Arc<Self>,
        conn: &zbus::Connection,
        defs: Vec<ProviderDef>,
    ) {
        let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
            return;
        };
        let names = match tokio::time::timeout(START_TIMEOUT, dbus.list_activatable_names()).await {
            Ok(Ok(n)) => n,
            _ => return,
        };
        for def in defs {
            if !names.iter().any(|n| n.as_str() == def.bus_name) {
                // Activatable=true without a service file: the key is ignored (§2.4.1).
                if lock(&self.starts).should_log(&def.bus_name, Instant::now()) {
                    eprintln!(
                        "uqctl bridge: provider {}: Activatable=true but no D-Bus service file",
                        def.bus_name
                    );
                }
                continue;
            }
            if owned(conn, &def.bus_name).await
                || !lock(&self.starts).try_start(&def.bus_name, Instant::now())
            {
                continue;
            }
            let (me, conn) = (self.clone(), conn.clone());
            tokio::spawn(async move {
                let name = def.bus_name.as_str();
                let start = async {
                    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
                    let wk = WellKnownName::try_from(name)?;
                    dbus.start_service_by_name(wk, 0)
                        .await
                        .map_err(zbus::Error::from)
                };
                let err = match tokio::time::timeout(START_TIMEOUT, start).await {
                    Ok(Ok(_)) => return,
                    Ok(Err(e)) => e.to_string(),
                    Err(_) => "didn't start within 2 s".to_string(),
                };
                if lock(&me.starts).should_log(name, Instant::now()) {
                    eprintln!("uqctl bridge: provider {name}: couldn't start: {err}");
                }
            });
        }
    }

    /// The bridge's `Activate` call.
    pub(crate) async fn activate(
        &self,
        link: &Link,
        runs: &Mutex<RunState>,
        args: &[Value],
    ) -> Result<Value, String> {
        let [
            Value::String(provider),
            Value::String(id),
            Value::String(action),
        ] = args
        else {
            return Err("Activate takes provider, id and action strings".into());
        };
        // Checked now, not only in `search`: a row shown before the provider was switched off
        // must not act.
        if !provider_enabled(&provider_prefs(link), pref_id(provider)) {
            match provider.as_str() {
                "calc" => lock(&self.state).calc = None,
                "run" => lock(runs).forget(),
                _ => {}
            }
            return Err(format!("{provider} is switched off"));
        }
        match provider.as_str() {
            "calc" => {
                let q = lock(&self.state).calc.clone();
                // Copying the row makes its value `ans` for the next sum.
                match (id.as_str(), q.as_deref().and_then(calc::remember)) {
                    ("calc", Some(result)) => Ok(json!({ "copy": result })),
                    _ => Err("unknown calculator result".into()),
                }
            }
            "run" => launch::launch(link, &[json!({ "run_id": id })], runs).await,
            "web" => {
                let url = lock(&self.state).web.clone();
                if url.as_deref() != Some(id.as_str()) {
                    return Err("that address is no longer shown".into());
                }
                super::super::opener::open("xdg-open", id, Some("web"))?;
                Ok(Value::Null)
            }
            "settings" => {
                let (page, setting) = {
                    let s = lock(&self.settings);
                    let (p, st) = settings_search::target(&s.rows, id).ok_or("unknown setting")?;
                    (p.to_string(), st.to_string())
                };
                let conn = link.connection().await.ok_or("no session bus")?;
                crate::settings_app::open(
                    &conn,
                    &page,
                    &setting,
                    crate::settings_app::spawn_detached,
                )
                .await?;
                Ok(Value::Null)
            }
            p if p.starts_with("files:") => {
                if !lock(&self.state).files.contains(id.as_str()) {
                    return Err("that file is no longer shown".into());
                }
                if !std::path::Path::new(id.as_str()).exists() {
                    return Err("it is no longer there".into());
                }
                super::super::opener::open("xdg-open", id, Some("file"))?;
                Ok(Value::Null)
            }
            _ => {
                if id.is_empty() || id.len() > MAX_ID_BYTES || action.len() > MAX_ACTION_BYTES {
                    return Err("id or action too long".into());
                }
                let defs = lock(&self.defs).current();
                let def = defs
                    .iter()
                    .find(|d| d.bus_name == *provider)
                    .ok_or("unknown provider")?;
                let conn = link.connection().await.ok_or("no session bus")?;
                if !owned(&conn, &def.bus_name).await {
                    return Err(format!("{} is not running", def.name));
                }
                let call = async {
                    let p = proxy(&conn, &def.bus_name).await?;
                    p.call_with_flags::<_, _, ()>(
                        "Activate",
                        MethodFlags::NO_AUTO_START,
                        &(id.as_str(), action.as_str()),
                    )
                    .await
                };
                match tokio::time::timeout(ACTIVATE_TIMEOUT, call).await {
                    Ok(Ok(_)) => Ok(Value::Null),
                    Ok(Err(e)) => {
                        self.log_once(&def.bus_name, &e.to_string());
                        Err(format!("{} failed", def.name))
                    }
                    Err(_) => {
                        self.log_once(&def.bus_name, "Activate timed out");
                        Err(format!("{} did not answer", def.name))
                    }
                }
            }
        }
    }
}
