//! Calling a method or reading properties over D-Bus with JSON in and out, within a deadline.

use super::*;

// ---------------------------------------------------------------------------------------------
// Calls

/// Where a call goes: one name, path and interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    pub path: String,
    pub interface: String,
}

pub(crate) fn call_err(e: zbus::Error) -> DbusJsonError {
    match e {
        zbus::Error::MethodError(name, Some(msg), _) => {
            DbusJsonError::Call(format!("{name}: {msg}"))
        }
        zbus::Error::MethodError(name, None, _) => DbusJsonError::Call(name.to_string()),
        other => DbusJsonError::Call(other.to_string()),
    }
}

pub(crate) async fn deadline<T>(
    f: impl std::future::Future<Output = Result<T, DbusJsonError>>,
) -> Result<T, DbusJsonError> {
    deadline_of(CALL_DEADLINE, f).await
}

pub(crate) async fn deadline_of<T>(
    within: Duration,
    f: impl std::future::Future<Output = Result<T, DbusJsonError>>,
) -> Result<T, DbusJsonError> {
    tokio::time::timeout(within, f)
        .await
        .unwrap_or(Err(DbusJsonError::Deadline))
}

pub(crate) fn check_result(v: Json) -> Result<Json, DbusJsonError> {
    if serde_json::to_string(&v).map_or(usize::MAX, |s| s.len()) > MAX_RESULT_JSON {
        return Err(DbusJsonError::ResultTooLarge);
    }
    Ok(v)
}

pub(crate) fn check_names(t: &Target) -> Result<(), DbusJsonError> {
    BusName::try_from(t.name.as_str())
        .map_err(|_| DbusJsonError::NotAllowed(format!("bus name {:?}", t.name)))?;
    ObjectPath::try_from(t.path.as_str())
        .map_err(|_| DbusJsonError::NotAllowed(format!("object path {:?}", t.path)))?;
    InterfaceName::try_from(t.interface.as_str())
        .map_err(|_| DbusJsonError::NotAllowed(format!("interface {:?}", t.interface)))?;
    Ok(())
}

pub(crate) async fn ensure_running(
    conn: &zbus::Connection,
    name: &str,
) -> Result<(), DbusJsonError> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await.map_err(call_err)?;
    let bus_name = BusName::try_from(name).map_err(|e| DbusJsonError::Call(e.to_string()))?;
    if dbus
        .name_has_owner(bus_name)
        .await
        .map_err(|e| call_err(e.into()))?
    {
        Ok(())
    } else {
        Err(DbusJsonError::NotRunning(name.to_string()))
    }
}

/// Call `method` on `target` with the JSON array `args`, typed from the target's introspection
/// data, within [`CALL_DEADLINE`]. No outputs is `null`, one output is that value, several are a
/// JSON array. With `auto_start` false, a name without an owner is an error and nothing is
/// started (§2.2: `NO_AUTO_START`).
pub async fn call(
    conn: &zbus::Connection,
    target: &Target,
    method: &str,
    args: &Json,
    auto_start: bool,
) -> Result<Json, DbusJsonError> {
    call_within(conn, target, method, args, auto_start, CALL_DEADLINE).await
}

/// [`call`] with its own deadline instead of [`CALL_DEADLINE`]: a page's `[dbus] timeout_ms`
/// for a method that may wait for a password prompt.
pub async fn call_within(
    conn: &zbus::Connection,
    target: &Target,
    method: &str,
    args: &Json,
    auto_start: bool,
    within: Duration,
) -> Result<Json, DbusJsonError> {
    if serde_json::to_string(args).map_or(usize::MAX, |s| s.len()) > MAX_ARGS_JSON {
        return Err(DbusJsonError::ArgsTooLarge);
    }
    check_names(target)?;
    MemberName::try_from(method)
        .map_err(|_| DbusJsonError::NotAllowed(format!("method {method:?}")))?;
    deadline_of(within, async {
        if !auto_start {
            ensure_running(conn, &target.name).await?;
        }
        let xml: String = conn
            .call_method(
                Some(target.name.as_str()),
                target.path.as_str(),
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
            .map_err(call_err)?
            .body()
            .deserialize()
            .map_err(call_err)?;
        let signature = method_in_signature(&xml, &target.interface, method)?;
        let values = json_to_args(&signature, args)?;
        let reply = if values.is_empty() {
            conn.call_method(
                Some(target.name.as_str()),
                target.path.as_str(),
                Some(target.interface.as_str()),
                method,
                &(),
            )
            .await
        } else {
            let mut body = zbus::wire::StructureBuilder::new();
            for v in values {
                body = body.append_field(v);
            }
            let body = body.build().map_err(|e| bad(e.to_string()))?;
            conn.call_method(
                Some(target.name.as_str()),
                target.path.as_str(),
                Some(target.interface.as_str()),
                method,
                &body,
            )
            .await
        }
        .map_err(call_err)?;
        reply_to_json(&reply)
    })
    .await
}

/// A method reply's body as JSON (`null`, one value, or an array of several).
pub fn reply_to_json(reply: &zbus::Message) -> Result<Json, DbusJsonError> {
    let body = reply.body();
    if body.len() > MAX_RESULT_JSON {
        return Err(DbusJsonError::ResultTooLarge);
    }
    let sig = body.signature().to_string();
    if body.is_empty() || sig.is_empty() {
        return Ok(Json::Null);
    }
    let sig = sig
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(&sig)
        .to_string();
    parse_signature(&sig)?;
    let fields: Structure<'_> = body.deserialize().map_err(call_err)?;
    let mut values: Vec<Json> = fields
        .fields()
        .iter()
        .map(value_to_json)
        .collect::<Result<_, _>>()?;
    check_result(if values.len() == 1 {
        values.remove(0)
    } else {
        Json::Array(values)
    })
}

/// Read one property of `target` within [`CALL_DEADLINE`].
pub async fn get_property(
    conn: &zbus::Connection,
    target: &Target,
    property: &str,
    auto_start: bool,
) -> Result<Json, DbusJsonError> {
    check_names(target)?;
    MemberName::try_from(property)
        .map_err(|_| DbusJsonError::NotAllowed(format!("property {property:?}")))?;
    deadline(async {
        if !auto_start {
            ensure_running(conn, &target.name).await?;
        }
        let reply = conn
            .call_method(
                Some(target.name.as_str()),
                target.path.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(target.interface.as_str(), property),
            )
            .await
            .map_err(call_err)?;
        let v: OwnedValue = reply.body().deserialize().map_err(call_err)?;
        check_result(value_to_json(&v)?)
    })
    .await
}

/// Read the listed properties of `target` (`GetAll`, filtered) as one JSON object, within
/// [`CALL_DEADLINE`]. A listed property of a type outside the allowlist is an error.
pub async fn get_properties(
    conn: &zbus::Connection,
    target: &Target,
    properties: &[String],
    auto_start: bool,
) -> Result<Json, DbusJsonError> {
    check_names(target)?;
    deadline(async {
        if !auto_start {
            ensure_running(conn, &target.name).await?;
        }
        let reply = conn
            .call_method(
                Some(target.name.as_str()),
                target.path.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "GetAll",
                &(target.interface.as_str(),),
            )
            .await
            .map_err(call_err)?;
        if reply.body().len() > MAX_RESULT_JSON {
            return Err(DbusJsonError::ResultTooLarge);
        }
        let all: std::collections::HashMap<String, OwnedValue> =
            reply.body().deserialize().map_err(call_err)?;
        let mut map = Map::new();
        for p in properties {
            if let Some(v) = all.get(p) {
                map.insert(p.clone(), value_to_json(v)?);
            }
        }
        check_result(Json::Object(map))
    })
    .await
}
