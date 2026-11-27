//! Calls a widget makes: methods, properties and declared commands, one at a time per widget,
//! within limits.

use super::*;

/// Held while a widget's D-Bus call runs; frees its slot on drop.
pub(crate) struct CallSlot<'a> {
    pub(crate) w: &'a Widgets,
    pub(crate) id: String,
}

impl Drop for CallSlot<'_> {
    fn drop(&mut self) {
        let mut inner = lock(&self.w.inner);
        if let Some(n) = inner.in_flight.get_mut(&self.id) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                inner.in_flight.remove(&self.id);
            }
        }
    }
}

pub(crate) fn dbus_err(e: DbusJsonError) -> String {
    e.to_string()
}

/// Run `f` within the 2 s call deadline (connecting and the activatable check included).
pub(crate) async fn bounded(
    f: impl std::future::Future<Output = Result<Value, String>>,
) -> Result<Value, String> {
    tokio::time::timeout(CALL_DEADLINE, f)
        .await
        .unwrap_or_else(|_| Err(DbusJsonError::Deadline.to_string()))
}

pub(crate) fn target(spec: &DbusSpec) -> Target {
    Target {
        name: spec.name.clone(),
        path: spec.path.clone(),
        interface: spec.interface.clone(),
    }
}

impl Widgets {
    /// Take one of the widget's 8 call slots, or `busy`.
    pub(crate) fn take_slot(&self, id: &str) -> Result<CallSlot<'_>, String> {
        let mut inner = lock(&self.inner);
        let n = inner.in_flight.entry(id.to_string()).or_insert(0);
        if *n >= MAX_IN_FLIGHT {
            return Err("busy".into());
        }
        *n += 1;
        Ok(CallSlot {
            w: self,
            id: id.to_string(),
        })
    }

    /// The `[dbus]` target, the connection, and whether the call may start the service (§2.2:
    /// only without `requires.bus_name`, and only if the name is activatable).
    pub(crate) async fn target(
        &self,
        m: &WidgetManifest,
    ) -> Result<(DbusSpec, zbus::Connection, bool), String> {
        let spec = m
            .dbus
            .clone()
            .ok_or_else(|| format!("widget {} has no [dbus] table", m.id))?;
        let conn = self
            .conn(spec.bus)
            .get()
            .await
            .ok_or("the bus is not reachable")?;
        let auto_start = m.requires.bus_name.is_none() && activatable(&conn, &spec.name).await;
        Ok((spec, conn, auto_start))
    }

    /// `WidgetShown [id, shown]`: whether any of the widget's slots is on screen. While none
    /// is, its properties aren't followed; when one is again, the current values are sent.
    pub(crate) fn set_shown(&self, args: &[Value]) -> Result<Value, String> {
        let [Value::String(id), Value::Bool(on)] = args else {
            return Err("WidgetShown takes a widget id and true or false".into());
        };
        let mut inner = lock(&self.inner);
        let tx = inner
            .shown
            .entry(id.clone())
            .or_insert_with(|| watch::channel(true).0);
        tx.send_if_modified(|v| std::mem::replace(v, *on) != *on);
        Ok(Value::Null)
    }

    /// `WidgetCall [id, method, args]`.
    pub(crate) async fn call(&self, args: &[Value]) -> Result<Value, String> {
        let (id, method, call_args) = match args {
            [Value::String(id), Value::String(m)] => (id, m, json!([])),
            [Value::String(id), Value::String(m), a @ Value::Array(_)] => (id, m, a.clone()),
            _ => return Err("WidgetCall takes a widget id, a method and an argument list".into()),
        };
        let m = self.visible(id)?;
        let listed = m
            .dbus
            .as_ref()
            .is_some_and(|d| d.methods.iter().any(|x| x == method));
        if !listed {
            return Err(format!("{method} is not one of widget {id}'s methods"));
        }
        let _slot = self.take_slot(id)?;
        bounded(async {
            let (spec, conn, auto) = self.target(&m).await?;
            dbus_json::call(&conn, &target(&spec), method, &call_args, auto)
                .await
                .map_err(dbus_err)
        })
        .await
    }

    /// `WidgetGet [id]`: every listed property as one object.
    pub(crate) async fn get(&self, args: &[Value]) -> Result<Value, String> {
        let [Value::String(id)] = args else {
            return Err("WidgetGet takes a widget id".into());
        };
        let m = self.visible(id)?;
        let _slot = self.take_slot(id)?;
        bounded(async {
            let (spec, conn, auto) = self.target(&m).await?;
            if spec.properties.is_empty() {
                return Ok(json!({}));
            }
            dbus_json::get_properties(&conn, &target(&spec), &spec.properties, auto)
                .await
                .map_err(dbus_err)
        })
        .await
    }

    /// `WidgetRun [id, command, args]`: `{code, output}` (or `{code, error, output}` for a
    /// non-zero exit).
    pub(crate) async fn run_command(&self, args: &[Value]) -> Result<Value, String> {
        let (id, cmd, extra) = match args {
            [Value::String(id), Value::String(c)] => (id, c, Vec::new()),
            [Value::String(id), Value::String(c), Value::Array(a)] => {
                let a: Option<Vec<String>> =
                    a.iter().map(|v| v.as_str().map(str::to_owned)).collect();
                (id, c, a.ok_or("arguments must be strings")?)
            }
            _ => {
                return Err(
                    "WidgetRun takes a widget id, a command id and a list of strings".into(),
                );
            }
        };
        let m = self.visible(id)?;
        let spec = command::find(&m.commands, cmd).map_err(|e| e.to_string())?;
        if spec.confirm.is_some() {
            // The bar has no confirmation dialog; such a command belongs on a settings page.
            return Err(format!(
                "{cmd} asks for confirmation, which widgets can't show"
            ));
        }
        let _token = self.busy.try_acquire(id, cmd).map_err(|_| "busy")?;
        match self.runner.run(spec, &extra).await {
            Ok(r) => {
                if r.code != 0 && !r.stderr_head.is_empty() {
                    eprintln!(
                        "uqctl bridge: widget {id} command {cmd} exited {}: {}",
                        r.code, r.stderr_head
                    );
                }
                Ok(r.to_json())
            }
            Err(CommandError::Busy) => Err("busy".into()),
            Err(e) => {
                eprintln!("uqctl bridge: widget {id} command {cmd}: {e}");
                Err(e.to_string())
            }
        }
    }
}
