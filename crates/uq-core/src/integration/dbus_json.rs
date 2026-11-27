//! JSON ↔ D-Bus values for integration calls (Phase 4 spec §2.2 limits, §6).
//!
//! Only `b y n q i u x t d s o as a{sv} a{ss}` and arrays of the basic types cross the boundary;
//! anything else is an error, in both directions. Argument types come from the target's
//! introspection data, never from the caller. Arguments are at most 16 KiB of JSON, results at
//! most 256 KiB, and a call has a 2 s deadline.
//!
//! [`call`] and [`get_property`] / [`get_properties`] are the deadline-bound calls the bridge
//! (widgets) and `uq-settings` (pages) make on behalf of integration QML.

use std::time::Duration;

use serde_json::{Map, Number, Value as Json};
use zbus::names::{BusName, InterfaceName, MemberName};
use zbus::wire::{
    Array, Dict, ObjectPath, OwnedVariant as OwnedValue, Signature, Structure, Variant as Value,
};
mod call;
pub use call::*;

/// The most JSON a caller may send as arguments.
pub const MAX_ARGS_JSON: usize = 16 * 1024;
/// The most JSON a result may become.
pub const MAX_RESULT_JSON: usize = 256 * 1024;
/// Every integration D-Bus call ends within this.
pub const CALL_DEADLINE: Duration = Duration::from_secs(2);
/// How deep `a{sv}` may nest (variants inside variants).
const MAX_DEPTH: usize = 4;

const BASIC: &str = "bynqiuxtdso";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DbusJsonError {
    #[error("unsupported D-Bus type {0:?}")]
    Unsupported(String),
    #[error("{0}")]
    BadValue(String),
    #[error("arguments are larger than 16 KiB")]
    ArgsTooLarge,
    #[error("the result is larger than 256 KiB")]
    ResultTooLarge,
    #[error("no answer within 2 s")]
    Deadline,
    #[error("{0} is not running")]
    NotRunning(String),
    #[error("{0} is not allowed")]
    NotAllowed(String),
    #[error("{0}")]
    Call(String),
}

fn bad(msg: impl Into<String>) -> DbusJsonError {
    DbusJsonError::BadValue(msg.into())
}

/// One allowed complete type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    /// One of `b y n q i u x t d s o`.
    Basic(char),
    /// `a` of a basic type (`as` included).
    Array(char),
    /// `a{sv}`
    DictSv,
    /// `a{ss}`
    DictSs,
}

impl Ty {
    pub fn signature(&self) -> String {
        match self {
            Ty::Basic(c) => c.to_string(),
            Ty::Array(c) => format!("a{c}"),
            Ty::DictSv => "a{sv}".into(),
            Ty::DictSs => "a{ss}".into(),
        }
    }
}

/// Split a signature into allowed complete types; any other type is an error.
pub fn parse_signature(sig: &str) -> Result<Vec<Ty>, DbusJsonError> {
    let mut out = Vec::new();
    let mut rest = sig;
    while !rest.is_empty() {
        let (ty, len) = if rest.starts_with("a{sv}") {
            (Ty::DictSv, 5)
        } else if rest.starts_with("a{ss}") {
            (Ty::DictSs, 5)
        } else {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some('a'), Some(c)) if BASIC.contains(c) => (Ty::Array(c), 2),
                (Some(c), _) if BASIC.contains(c) => (Ty::Basic(c), 1),
                _ => {
                    let end = rest.char_indices().nth(8).map_or(rest.len(), |(i, _)| i);
                    return Err(DbusJsonError::Unsupported(rest[..end].to_string()));
                }
            }
        };
        out.push(ty);
        rest = &rest[len..];
    }
    Ok(out)
}

fn sig(s: &str) -> Signature {
    // Only called with the fixed, valid signatures of this module.
    Signature::try_from(s).unwrap_or(Signature::Unit)
}

fn int<T: TryFrom<i64> + TryFrom<u64>>(c: char, v: &Json) -> Result<T, DbusJsonError> {
    let n = v
        .as_number()
        .ok_or_else(|| bad(format!("expected a whole number for {c:?}, got {v}")))?;
    let out = if let Some(i) = n.as_i64() {
        T::try_from(i).ok()
    } else if let Some(u) = n.as_u64() {
        T::try_from(u).ok()
    } else {
        None
    };
    out.ok_or_else(|| bad(format!("{n} is not a whole number in the range of {c:?}")))
}

fn basic_to_value(c: char, v: &Json) -> Result<Value<'static>, DbusJsonError> {
    Ok(match c {
        'b' => Value::Bool(
            v.as_bool()
                .ok_or_else(|| bad(format!("expected true or false, got {v}")))?,
        ),
        'y' => Value::U8(int(c, v)?),
        'n' => Value::I16(int(c, v)?),
        'q' => Value::U16(int(c, v)?),
        'i' => Value::I32(int(c, v)?),
        'u' => Value::U32(int(c, v)?),
        'x' => Value::I64(int(c, v)?),
        't' => Value::U64(int(c, v)?),
        'd' => Value::F64(
            v.as_f64()
                .ok_or_else(|| bad(format!("expected a number, got {v}")))?,
        ),
        's' => Value::from(text(v)?.to_string()),
        'o' => Value::ObjectPath(
            ObjectPath::try_from(text(v)?.to_string())
                .map_err(|_| bad(format!("{v} is not an object path")))?,
        ),
        other => return Err(DbusJsonError::Unsupported(other.to_string())),
    })
}

fn text(v: &Json) -> Result<&str, DbusJsonError> {
    let s = v
        .as_str()
        .ok_or_else(|| bad(format!("expected a string, got {v}")))?;
    if s.contains('\0') {
        return Err(bad("a string may not hold a NUL character"));
    }
    Ok(s)
}

/// A JSON value as the given allowed type.
pub fn json_to_value(ty: Ty, v: &Json) -> Result<Value<'static>, DbusJsonError> {
    match ty {
        Ty::Basic(c) => basic_to_value(c, v),
        Ty::Array(c) => {
            let items = v
                .as_array()
                .ok_or_else(|| bad(format!("expected an array, got {v}")))?;
            let mut arr = Array::new(&sig(&c.to_string()));
            for item in items {
                arr.append(basic_to_value(c, item)?)
                    .map_err(|e| bad(e.to_string()))?;
            }
            Ok(Value::Array(arr))
        }
        Ty::DictSs => {
            let obj = v
                .as_object()
                .ok_or_else(|| bad(format!("expected an object, got {v}")))?;
            let mut dict = Dict::new(&Signature::Str, &Signature::Str);
            for (k, val) in obj {
                dict.append(Value::from(k.clone()), Value::from(text(val)?.to_string()))
                    .map_err(|e| bad(e.to_string()))?;
            }
            Ok(Value::Dict(dict))
        }
        Ty::DictSv => dict_sv(v, 0),
    }
}

fn dict_sv(v: &Json, depth: usize) -> Result<Value<'static>, DbusJsonError> {
    if depth >= MAX_DEPTH {
        return Err(bad("a{sv} nests too deeply"));
    }
    let obj = v
        .as_object()
        .ok_or_else(|| bad(format!("expected an object, got {v}")))?;
    let mut dict = Dict::new(&Signature::Str, &Signature::Variant);
    for (k, val) in obj {
        let inner = variant_from_json(val, depth + 1)?;
        dict.append(Value::from(k.clone()), Value::Variant(Box::new(inner)))
            .map_err(|e| bad(e.to_string()))?;
    }
    Ok(Value::Dict(dict))
}

/// The value inside a `v` of an `a{sv}`, typed from the JSON: `true` → `b`, a whole number →
/// `x` (or `t` above `i64`), another number → `d`, a string → `s`, an array of one kind → `ab`,
/// `ax`, `ad` or `as` (empty: `as`), an object → `a{sv}`. `null` and mixed arrays are errors.
fn variant_from_json(v: &Json, depth: usize) -> Result<Value<'static>, DbusJsonError> {
    let scalar = |v: &Json| -> Option<char> {
        match v {
            Json::Bool(_) => Some('b'),
            Json::Number(n) if n.is_i64() => Some('x'),
            Json::Number(n) if n.is_u64() => Some('t'),
            Json::Number(_) => Some('d'),
            Json::String(_) => Some('s'),
            _ => None,
        }
    };
    match v {
        Json::Object(_) => dict_sv(v, depth),
        Json::Array(items) => {
            let kinds: Vec<Option<char>> = items.iter().map(scalar).collect();
            let kind = match kinds.first() {
                None => 's',
                Some(None) => return Err(bad("an array in a{sv} must hold plain values")),
                Some(Some(first)) => {
                    let numeric = |k: &Option<char>| matches!(k, Some('x' | 't' | 'd'));
                    if kinds.iter().all(|k| k == &Some(*first)) && *first != 't' {
                        *first
                    } else if kinds.iter().all(numeric) {
                        'd'
                    } else {
                        return Err(bad("an array in a{sv} must hold one kind of value"));
                    }
                }
            };
            json_to_value(Ty::Array(kind), v)
        }
        other => match scalar(other) {
            Some(c) => basic_to_value(c, other),
            None => Err(bad("null can't be sent over D-Bus")),
        },
    }
}

/// The caller's JSON array of arguments as values for `signature`. The count must match.
pub fn json_to_args(signature: &str, args: &Json) -> Result<Vec<Value<'static>>, DbusJsonError> {
    let types = parse_signature(signature)?;
    let empty = Vec::new();
    let items = match args {
        Json::Null => &empty,
        Json::Array(a) => a,
        _ => return Err(bad("arguments must be a JSON array")),
    };
    if items.len() != types.len() {
        return Err(bad(format!(
            "expected {} argument(s) ({signature}), got {}",
            types.len(),
            items.len()
        )));
    }
    types
        .into_iter()
        .zip(items)
        .map(|(t, v)| json_to_value(t, v))
        .collect()
}

fn number(f: f64) -> Result<Json, DbusJsonError> {
    Number::from_f64(f)
        .map(Json::Number)
        .ok_or_else(|| bad(format!("{f} can't be shown as JSON")))
}

/// A D-Bus value of an allowed type as JSON; any other type is an error.
pub fn value_to_json(v: &Value<'_>) -> Result<Json, DbusJsonError> {
    to_json(v, 0)
}

fn to_json(v: &Value<'_>, depth: usize) -> Result<Json, DbusJsonError> {
    let unsupported = || DbusJsonError::Unsupported(v.value_signature().to_string());
    Ok(match v {
        Value::Bool(b) => Json::Bool(*b),
        Value::U8(n) => Json::from(*n),
        Value::I16(n) => Json::from(*n),
        Value::U16(n) => Json::from(*n),
        Value::I32(n) => Json::from(*n),
        Value::U32(n) => Json::from(*n),
        Value::I64(n) => Json::from(*n),
        Value::U64(n) => Json::from(*n),
        Value::F64(f) => number(*f)?,
        Value::Str(s) => Json::String(s.as_str().to_string()),
        Value::ObjectPath(p) => Json::String(p.as_str().to_string()),
        Value::Array(a) => {
            let elem = a.element_signature().to_string();
            if elem.len() != 1 || !BASIC.contains(elem.as_str()) {
                return Err(unsupported());
            }
            Json::Array(
                a.inner()
                    .iter()
                    .map(|e| to_json(e, depth))
                    .collect::<Result<_, _>>()?,
            )
        }
        Value::Dict(d) => {
            let ty = d.signature().to_string();
            let variant_values = match ty.as_str() {
                "a{sv}" => true,
                "a{ss}" => false,
                _ => return Err(unsupported()),
            };
            if depth >= MAX_DEPTH {
                return Err(bad("a{sv} nests too deeply"));
            }
            let mut map = Map::new();
            for (k, val) in d.iter() {
                let Value::Str(key) = k else {
                    return Err(unsupported());
                };
                let val = match (variant_values, val) {
                    (true, Value::Variant(inner)) => to_json(inner, depth + 1)?,
                    (false, Value::Str(s)) => Json::String(s.as_str().to_string()),
                    _ => return Err(unsupported()),
                };
                map.insert(key.as_str().to_string(), val);
            }
            Json::Object(map)
        }
        // A variant only appears as an a{sv} value (unwrapped above) or a property value
        // (unwrapped when read); a bare or nested one is refused.
        _ => return Err(unsupported()),
    })
}

/// The input signature of `method` on `interface` in an introspection document.
pub fn method_in_signature(
    xml: &str,
    interface: &str,
    method: &str,
) -> Result<String, DbusJsonError> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut in_iface, mut in_method, mut found) = (false, false, false);
    let mut sig = String::new();
    let attr = |e: &quick_xml::events::BytesStart<'_>, name: &str| -> Option<String> {
        e.try_get_attribute(name).ok().flatten().and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        })
    };
    loop {
        let ev = reader
            .read_event()
            .map_err(|e| DbusJsonError::Call(format!("bad introspection data: {e}")))?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let open = matches!(ev, Event::Start(_));
                match e.name().as_ref() {
                    "interface" => in_iface = open && attr(e, "name").as_deref() == Some(interface),
                    "method" if in_iface && attr(e, "name").as_deref() == Some(method) => {
                        // `<method name="M"/>` takes no arguments and has no end tag.
                        in_method = open;
                        found = true;
                    }
                    "arg"
                        if in_method && attr(e, "direction").as_deref().unwrap_or("in") == "in" =>
                    {
                        sig.push_str(&attr(e, "type").unwrap_or_default());
                    }
                    _ => {}
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "interface" => in_iface = false,
                "method" => in_method = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    if !found {
        return Err(DbusJsonError::Call(format!(
            "{interface}.{method} is not in the introspection data"
        )));
    }
    Ok(sig)
}

#[cfg(test)]
#[path = "dbus_json_tests.rs"]
mod tests;
