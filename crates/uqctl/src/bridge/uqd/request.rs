//! The shell's requests to uqd: parsed from the bridge call, sent over `org.uq.Desktop1`, and
//! the reply as JSON.

use super::*;

/// A validated call, ready to send.
pub(crate) enum Request {
    GetTheme,
    SetTheme(String, String),
    ListThemes,
    ListThemeInfo,
    GetLayouts,
    SetLayout(String, String),
    GetSchedule,
    SetSchedule(String),
    Reload,
    GetSettings,
    SetSetting(String, String),
    GetLaunchCounts,
    RecordLaunch(String),
    ListProfiles,
    ListOutputs,
    ApplyProfile(String),
    ProfileOff,
    PickDone(String, String),
    /// Replace `shell.bar` (the bar's layout, edited from the bar's own menu), keeping the
    /// rest of the `shell` section as uqd has it.
    SetBarLayout(String),
    /// The bridge's own calls; the shell can't make them (`handle()` doesn't pass them on;
    /// `GetWelcome` reaches the shell only inside `Hello` and `WelcomeStatus`).
    GetSection(String),
    ShellReady,
    GetWelcome,
}

pub(crate) fn string_arg(args: &[Value], i: usize, what: &str) -> Result<String, String> {
    args.get(i)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("argument {} ({what}) must be a string", i + 1))
}

pub(crate) fn arity(call: &str, args: &[Value], n: usize) -> Result<(), String> {
    if args.len() == n {
        Ok(())
    } else {
        Err(format!("{call} takes {n} argument(s), got {}", args.len()))
    }
}

impl Request {
    pub(crate) fn parse(call: &str, args: &[Value]) -> Result<Request, String> {
        let r = match call {
            "GetTheme" => (0, Request::GetTheme),
            "ListThemes" => (0, Request::ListThemes),
            "ListThemeInfo" => (0, Request::ListThemeInfo),
            "GetLayouts" => (0, Request::GetLayouts),
            "GetSchedule" => (0, Request::GetSchedule),
            "Reload" => (0, Request::Reload),
            "GetSettings" => (0, Request::GetSettings),
            "GetLaunchCounts" => (0, Request::GetLaunchCounts),
            "ListProfiles" => (0, Request::ListProfiles),
            "ListOutputs" => (0, Request::ListOutputs),
            "ProfileOff" => (0, Request::ProfileOff),
            "ShellReady" => (0, Request::ShellReady),
            "GetWelcome" => (0, Request::GetWelcome),
            "GetSection" => {
                arity(call, args, 1)?;
                (1, Request::GetSection(string_arg(args, 0, "domain")?))
            }
            "SetTheme" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetTheme(
                        string_arg(args, 0, "name")?,
                        string_arg(args, 1, "variant")?,
                    ),
                )
            }
            "SetLayout" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetLayout(
                        string_arg(args, 0, "workspace")?,
                        string_arg(args, 1, "layout")?,
                    ),
                )
            }
            "SetSchedule" => {
                arity(call, args, 1)?;
                // The schedule is a JSON object (a pre-encoded JSON string is accepted too).
                let json = match &args[0] {
                    Value::String(s) => s.clone(),
                    v @ Value::Object(_) => v.to_string(),
                    _ => return Err("argument 1 (schedule) must be an object".into()),
                };
                (1, Request::SetSchedule(json))
            }
            "SetSetting" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::SetSetting(string_arg(args, 0, "key")?, args[1].to_string()),
                )
            }
            "RecordLaunch" => {
                arity(call, args, 1)?;
                (1, Request::RecordLaunch(string_arg(args, 0, "desktop id")?))
            }
            "PickDone" => {
                arity(call, args, 2)?;
                (
                    2,
                    Request::PickDone(
                        string_arg(args, 0, "token")?,
                        string_arg(args, 1, "outcome")?,
                    ),
                )
            }
            "SetBarLayout" => {
                arity(call, args, 1)?;
                if !args[0].is_object() {
                    return Err("argument 1 (layout) must be an object".into());
                }
                (1, Request::SetBarLayout(args[0].to_string()))
            }
            "ApplyProfile" => {
                arity(call, args, 1)?;
                (1, Request::ApplyProfile(string_arg(args, 0, "profile id")?))
            }
            other => return Err(format!("unknown call {other}")),
        };
        if r.0 == 0 {
            arity(call, args, 0)?;
        }
        Ok(r.1)
    }

    pub(crate) async fn send(self, conn: &Connection) -> Result<Value, String> {
        let proxy = zbus::Proxy::new(conn, NAME, PATH, IFACE)
            .await
            .map_err(describe)?;
        match self {
            Request::GetTheme => {
                let (n, v): (String, String) =
                    proxy.call("GetTheme", &()).await.map_err(describe)?;
                Ok(json!([n, v]))
            }
            Request::ListThemes => {
                let ids: Vec<String> = proxy.call("ListThemes", &()).await.map_err(describe)?;
                Ok(json!(ids))
            }
            Request::ListThemeInfo => {
                let s: String = proxy.call("ListThemeInfo", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad theme list: {e}"))
            }
            Request::GetLayouts => {
                let m: HashMap<String, String> =
                    proxy.call("GetLayouts", &()).await.map_err(describe)?;
                Ok(json!(m))
            }
            Request::GetSchedule => {
                let s: String = proxy.call("GetSchedule", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad schedule: {e}"))
            }
            Request::GetSettings => {
                let s: String = proxy.call("GetSettings", &()).await.map_err(describe)?;
                let v: Value =
                    serde_json::from_str(&s).map_err(|e| format!("bad settings: {e}"))?;
                Ok(if v.is_object() {
                    v
                } else {
                    Value::Object(Map::new())
                })
            }
            Request::GetLaunchCounts => {
                let m: HashMap<String, u32> =
                    proxy.call("GetLaunchCounts", &()).await.map_err(describe)?;
                Ok(json!(m))
            }
            Request::Reload => unit(proxy.call("Reload", &()).await),
            Request::SetTheme(a, b) => unit(proxy.call("SetTheme", &(a, b, ORIGIN)).await),
            Request::SetLayout(a, b) => unit(proxy.call("SetLayout", &(a, b, ORIGIN)).await),
            Request::SetSchedule(j) => unit(proxy.call("SetSchedule", &(j, ORIGIN)).await),
            Request::SetSetting(k, j) => unit(proxy.call("SetSetting", &(k, j, ORIGIN)).await),
            Request::PickDone(t, o) => unit(proxy.call("PickDone", &(t, o)).await),
            Request::RecordLaunch(id) => unit(proxy.call("RecordLaunch", &(id,)).await),
            // Profile writes take no origin: uqd journals them as `profile:<id>`.
            Request::ListProfiles => {
                let s: String = proxy.call("ListProfiles", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad profile list: {e}"))
            }
            Request::ListOutputs => {
                let s: String = proxy.call("ListOutputs", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad output list: {e}"))
            }
            Request::ApplyProfile(id) => {
                report(proxy.call("ApplyProfile", &(id,)).await.map_err(describe)?)
            }
            Request::ProfileOff => report(proxy.call("ProfileOff", &()).await.map_err(describe)?),
            Request::GetSection(d) => {
                let s: String = proxy.call("GetSection", &(d,)).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad section: {e}"))
            }
            Request::SetBarLayout(bar) => {
                let s: String = proxy
                    .call("GetSection", &("shell",))
                    .await
                    .map_err(describe)?;
                let mut shell: Value =
                    serde_json::from_str(&s).map_err(|e| format!("bad section: {e}"))?;
                let bar: Value = serde_json::from_str(&bar).map_err(|e| e.to_string())?;
                let Some(obj) = shell.as_object_mut() else {
                    return Err("bad section: not an object".into());
                };
                obj.insert("bar".into(), bar);
                unit(
                    proxy
                        .call("SetSection", &("shell", shell.to_string(), ORIGIN))
                        .await,
                )
            }
            Request::ShellReady => unit(proxy.call("ShellReady", &()).await),
            Request::GetWelcome => {
                let s: String = proxy.call("GetWelcome", &()).await.map_err(describe)?;
                serde_json::from_str(&s).map_err(|e| format!("bad welcome status: {e}"))
            }
        }
    }
}

/// uqd's profile report (`{active, kept, notices}`), parsed.
pub(crate) fn report(s: String) -> Result<Value, String> {
    serde_json::from_str(&s).map_err(|e| format!("bad profile report: {e}"))
}

pub(crate) fn unit(r: zbus::Result<()>) -> Result<Value, String> {
    r.map(|()| Value::Null).map_err(describe)
}

/// D-Bus error to the `err` text: uqd's own message when it gave one, "not running" when the
/// name has no owner.
pub(crate) fn describe(err: zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, msg, _) => {
            if matches!(
                name.as_str(),
                "org.freedesktop.DBus.Error.ServiceUnknown"
                    | "org.freedesktop.DBus.Error.NameHasNoOwner"
            ) {
                DOWN.into()
            } else {
                msg.unwrap_or_else(|| name.to_string())
            }
        }
        zbus::Error::FDO(e) => match *e {
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
                DOWN.into()
            }
            other => other.to_string(),
        },
        other => other.to_string(),
    }
}
