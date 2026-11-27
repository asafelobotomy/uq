//! A page's D-Bus calls and property reads, routed to the bus its manifest names, and opening an app.

use super::*;

/// Where a D-Bus request of a page goes and how long it may take.
#[derive(Debug)]
pub struct Route {
    pub target: Target,
    pub bus: Bus,
    /// Whether the service may be started by the request.
    pub auto_start: bool,
    /// 2 s, or the method's `[dbus] timeout_ms`.
    pub deadline: Duration,
}

/// Where a D-Bus request of `page` may go: the target, the bus, whether the service may be
/// started, its deadline, and whether `member` is allowed (`property` picks the list).
pub fn dbus_target(
    manifest: Option<PageManifest>,
    member: &str,
    property: bool,
    target_json: &str,
) -> Result<Route, String> {
    let m = manifest.ok_or("this page is not installed any more")?;
    let name = m
        .dbus_name
        .clone()
        .ok_or("this page has no service to talk to (no dbus_name in page.toml)")?;
    let given: Value = if target_json.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(target_json).map_err(|_| "the target must be {path, interface}")?
    };
    let auto_start = m.requires.bus_name.is_none();
    let field = |k: &str| given.get(k).and_then(Value::as_str).map(str::to_string);
    match &m.dbus {
        Some(d) => {
            let other = |k: &str, want: &str| field(k).is_some_and(|v| v != want);
            if other("path", &d.path) || other("interface", &d.interface) {
                return Err(
                    "this page may only use the path and interface in its page.toml".into(),
                );
            }
            let listed = if property { &d.properties } else { &d.methods };
            if !listed.iter().any(|x| x == member) {
                return Err(format!(
                    "{member} is not listed in the page's [dbus] {}",
                    if property { "properties" } else { "methods" }
                ));
            }
            Ok(Route {
                target: Target {
                    name,
                    path: d.path.clone(),
                    interface: d.interface.clone(),
                },
                bus: d.bus,
                auto_start,
                deadline: if property {
                    dbus_json::CALL_DEADLINE
                } else {
                    d.deadline(member)
                },
            })
        }
        None => {
            let (Some(path), Some(interface)) = (field("path"), field("interface")) else {
                return Err("give the object path and interface: {path, interface}".into());
            };
            Ok(Route {
                target: Target {
                    name,
                    path,
                    interface,
                },
                bus: Bus::Session,
                auto_start,
                deadline: dbus_json::CALL_DEADLINE,
            })
        }
    }
}

pub(crate) async fn connection(bus: Bus) -> Result<zbus::Connection, String> {
    static SESSION: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    static SYSTEM: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    let cell = match bus {
        Bus::Session => &SESSION,
        Bus::System => &SYSTEM,
    };
    cell.get_or_try_init(|| async {
        match bus {
            Bus::Session => match crate::session::bus() {
                Some((_, c)) => Ok(c),
                None => zbus::Connection::session().await,
            },
            Bus::System => zbus::Connection::system().await,
        }
    })
    .await
    .cloned()
    .map_err(|e| format!("can't reach the message bus: {e}"))
}

impl PluginHost {
    pub(crate) fn open_app(mut self: Pin<&mut Self>, page: &QString, desktop_id: &QString) -> i32 {
        let id = self.as_mut().next_id();
        let page = page.to_string();
        if plugins::manifest(&page).is_none() {
            self.deliver(id, error("this page is not installed any more"));
            return id;
        }
        let argv = match open_app_argv(&desktop_id.to_string()) {
            Ok(a) => a,
            Err(e) => {
                self.deliver(id, error(e));
                return id;
            }
        };
        let Some(rt) = RUNTIME.get() else {
            self.deliver(id, error("apps can't be opened right now"));
            return id;
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let value = launch(&argv, OPEN_APP_WAIT).await;
            if let Some(e) = value.get("error") {
                eprintln!("uq-settings: page {page}: opening {}: {e}", argv[2]);
            }
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
        id
    }

    pub(crate) fn dbus_call(
        self: Pin<&mut Self>,
        page: &QString,
        method: &QString,
        args_json: &QString,
        target_json: &QString,
    ) -> i32 {
        let args: Result<Value, String> = match args_json.to_string().trim() {
            "" => Ok(json!([])),
            text => serde_json::from_str(text).map_err(|_| "arguments must be a JSON list".into()),
        };
        self.dbus(page, method, target_json, false, args)
    }

    pub(crate) fn dbus_get(
        self: Pin<&mut Self>,
        page: &QString,
        property: &QString,
        target_json: &QString,
    ) -> i32 {
        self.dbus(page, property, target_json, true, Ok(Value::Null))
    }

    pub(crate) fn dbus(
        mut self: Pin<&mut Self>,
        page: &QString,
        member: &QString,
        target_json: &QString,
        property: bool,
        args: Result<Value, String>,
    ) -> i32 {
        let id = self.as_mut().next_id();
        let (page, member) = (page.to_string(), member.to_string());
        let target = dbus_target(
            plugins::manifest(&page),
            &member,
            property,
            &target_json.to_string(),
        );
        let (route, args) = match (target, args) {
            (Ok(route), Ok(args)) => (route, args),
            (Err(e), _) | (_, Err(e)) => {
                self.deliver(id, error(e));
                return id;
            }
        };
        let Some(slot) = Slot::take(&page) else {
            self.deliver(id, error("busy: too many requests at once"));
            return id;
        };
        let Some(rt) = RUNTIME.get() else {
            self.deliver(id, error("the service can't be reached right now"));
            return id;
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let Route {
                target,
                bus,
                auto_start,
                deadline,
            } = route;
            let result = match connection(bus).await {
                Err(e) => Err(e),
                Ok(conn) => if property {
                    dbus_json::get_property(&conn, &target, &member, auto_start).await
                } else {
                    dbus_json::call_within(&conn, &target, &member, &args, auto_start, deadline)
                        .await
                }
                .map_err(|e| e.to_string()),
            };
            drop(slot);
            let value = match result {
                Ok(v) => json!({"value": v}),
                Err(e) => error(e),
            };
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
        id
    }
}
