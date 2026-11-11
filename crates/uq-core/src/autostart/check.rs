//! The rules of the Startup section: one choice or one added entry from the section JSON,
//! each refusal a plain sentence.

use super::{Added, FileChoice, MAX_ARG_CHARS, MAX_ARGV, MAX_DELAY, MAX_NAME_CHARS};
use crate::desktop_entry::{normalise_id, valid_desktop_id};
use crate::settings::{SettingError, check_known, invalid};
use serde_json::{Map, Value};

const APP_REASON: &str = "an app is named by its file name, such as firefox.desktop";
const DELAY_REASON: &str = "a delay is 0 to 600 seconds";
const BOOL_REASON: &str = "expected on or off (true or false)";
const CONTROL: &str = "line breaks, tabs or other control characters";

fn field<'a>(o: &'a Map<String, Value>, f: &str) -> Option<&'a Value> {
    o.get(f).filter(|v| !v.is_null())
}

fn parse_delay(key: &str, v: Option<&Value>) -> Result<Option<u32>, SettingError> {
    match v {
        None => Ok(None),
        Some(d) => d
            .as_u64()
            .filter(|d| *d <= u64::from(MAX_DELAY))
            .map(|d| Some(d as u32))
            .ok_or_else(|| invalid(key, DELAY_REASON)),
    }
}

fn check_delay(key: &str, d: Option<u32>) -> Result<(), SettingError> {
    match d {
        Some(d) if d > MAX_DELAY => Err(invalid(key, DELAY_REASON)),
        _ => Ok(()),
    }
}

pub(super) fn check_choice(key: &str, c: &FileChoice) -> Result<(), SettingError> {
    if !valid_desktop_id(&c.id) {
        return Err(invalid(&format!("{key}.id"), APP_REASON));
    }
    check_delay(&format!("{key}.delay"), c.delay)?;
    if c.enabled.is_none() && c.delay.is_none() {
        return Err(invalid(key, "a startup choice needs on or off, or a delay"));
    }
    Ok(())
}

pub(super) fn parse_choice(key: &str, v: &Value) -> Result<FileChoice, SettingError> {
    let Some(o) = v.as_object() else {
        return Err(invalid(
            key,
            "a choice is a set of fields, not a single value",
        ));
    };
    check_known(key, v, &["id", "enabled", "delay"])?;
    let id = field(o, "id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("{key}.id"), APP_REASON))?;
    let enabled = match field(o, "enabled") {
        None => None,
        Some(b) => Some(
            b.as_bool()
                .ok_or_else(|| invalid(&format!("{key}.enabled"), BOOL_REASON))?,
        ),
    };
    let c = FileChoice {
        id: normalise_id(id),
        enabled,
        delay: parse_delay(&format!("{key}.delay"), field(o, "delay"))?,
    };
    check_choice(key, &c)?;
    Ok(c)
}

/// Every rule of an added entry except its id's, which the caller checks or assigns.
pub(super) fn check_added(key: &str, a: &Added) -> Result<(), SettingError> {
    let name = a.name();
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS {
        return Err(invalid(
            &format!("{key}.name"),
            "a name has 1 to 100 characters",
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid(
            &format!("{key}.name"),
            format!("a name can't contain {CONTROL}"),
        ));
    }
    check_delay(&format!("{key}.delay"), a.delay())?;
    match a {
        Added::App { app, .. } if !valid_desktop_id(app) => {
            Err(invalid(&format!("{key}.app"), APP_REASON))
        }
        Added::App { .. } => Ok(()),
        Added::Command { argv, .. } => check_argv(&format!("{key}.argv"), argv),
    }
}

fn check_argv(key: &str, argv: &[String]) -> Result<(), SettingError> {
    if argv.is_empty() || argv.len() > MAX_ARGV {
        return Err(invalid(key, "a command has 1 to 64 parts"));
    }
    for (j, arg) in argv.iter().enumerate() {
        let k = format!("{key}[{j}]");
        let n = arg.chars().count();
        if n == 0 || n > MAX_ARG_CHARS {
            return Err(invalid(
                &k,
                "each part of a command has 1 to 1024 characters",
            ));
        }
        if arg.chars().any(char::is_control) {
            return Err(invalid(&k, format!("a command can't contain {CONTROL}")));
        }
    }
    if argv[0].contains('=') {
        return Err(invalid(
            &format!("{key}[0]"),
            "a command starts with the program to run, and its name can't contain =",
        ));
    }
    Ok(())
}

/// One added entry from the section JSON. A missing or empty id is left empty for the caller.
pub(super) fn parse_added(key: &str, v: &Value) -> Result<Added, SettingError> {
    let Some(o) = v.as_object() else {
        return Err(invalid(
            key,
            "an entry is a set of fields, not a single value",
        ));
    };
    check_known(key, v, &["id", "name", "app", "argv", "enabled", "delay"])?;
    let at = |f: &str| format!("{key}.{f}");
    let id = match field(o, "id") {
        None => String::new(),
        Some(i) => i
            .as_str()
            .map(String::from)
            .ok_or_else(|| invalid(&at("id"), "an id looks like a-1a2b"))?,
    };
    let name = field(o, "name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&at("name"), "an entry needs a name"))?
        .to_string();
    let enabled = match field(o, "enabled") {
        None => true,
        Some(b) => b
            .as_bool()
            .ok_or_else(|| invalid(&at("enabled"), BOOL_REASON))?,
    };
    let delay = parse_delay(&at("delay"), field(o, "delay"))?;
    let a = match (field(o, "app"), field(o, "argv")) {
        (Some(_), Some(_)) => {
            return Err(invalid(
                key,
                "an entry starts an app or a command, not both",
            ));
        }
        (None, None) => return Err(invalid(key, "an entry needs an app or a command")),
        (Some(app), None) => Added::App {
            id,
            name,
            app: normalise_id(
                app.as_str()
                    .ok_or_else(|| invalid(&at("app"), APP_REASON))?,
            ),
            enabled,
            delay,
        },
        (None, Some(argv)) => {
            let Some(items) = argv.as_array() else {
                return Err(invalid(&at("argv"), "a command is a list of its parts"));
            };
            let argv = items
                .iter()
                .enumerate()
                .map(|(j, a)| {
                    a.as_str().map(String::from).ok_or_else(|| {
                        invalid(
                            &format!("{key}.argv[{j}]"),
                            "each part of a command is text",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Added::Command {
                id,
                name,
                argv,
                enabled,
                delay,
            }
        }
    };
    check_added(key, &a)?;
    Ok(a)
}

/// A list field of the section: absent is empty.
pub(super) fn items<'a>(o: &'a Map<String, Value>, f: &str) -> Result<&'a [Value], SettingError> {
    match field(o, f) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a.as_slice()),
        Some(_) => Err(invalid(&format!("autostart.{f}"), "expected a list")),
    }
}
