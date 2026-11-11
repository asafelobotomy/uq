//! Changing a section (`env`, `autostart` or `compat`): read it, edit it, check it with uq-core's own rules so a refusal reads
//! as a plain reason (F21), and write it back.

use serde_json::{Map, Value, json};
use uq_core::autostart::Autostart;
use uq_core::client::{self, DesktopProxy};
use uq_core::compat::Compat;
use uq_core::desktop_entry::ExecError;
use uq_core::env::Env;
use uq_core::settings::SettingError;

use super::UNCLOSED_QUOTE;
use crate::pages::runner::{ORIGIN, message, parse_json, plain_refusal};

pub(crate) fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

/// The plain reason of a refusal, without the key (F21).
pub(crate) fn reason(e: &SettingError) -> String {
    match e {
        SettingError::Invalid { reason, .. } => reason.clone(),
        other => other.to_string(),
    }
}

/// Why a typed command can't be used, in plain words.
pub(super) fn command_error(e: &ExecError) -> &'static str {
    match e {
        ExecError::Unterminated => UNCLOSED_QUOTE,
        ExecError::SingleQuote => {
            "Put double quotes around a part with spaces; single quotes don't work here."
        }
        ExecError::Empty => "Type the command to run.",
        _ => "This command can't be read.",
    }
}

/// uq-core's check of a whole section, before it is sent.
pub(crate) fn precheck(domain: &str, section: &Value) -> zbus::Result<()> {
    let checked = match domain {
        "env" => Env::from_section(section).map(drop),
        "compat" => Compat::from_section(section).map(drop),
        _ => Autostart::from_section(section, &Autostart::default()).map(drop),
    };
    checked.map_err(|e| failure(reason(&e)))
}

/// Read `domain`'s section, let `edit` change it, check it and write it back with origin
/// `settings`. Call it under the write lock. uqd's own refusal loses its key (F21).
pub(crate) async fn write_section(
    proxy: &DesktopProxy<'_>,
    domain: &str,
    edit: impl FnOnce(&mut Map<String, Value>) -> zbus::Result<()>,
) -> zbus::Result<()> {
    let mut section = parse_json(&proxy.get_section(domain).await?)?;
    let object = section
        .as_object_mut()
        .ok_or_else(|| failure(format!("the {domain} section isn't an object")))?;
    edit(object)?;
    precheck(domain, &section)?;
    proxy
        .set_section(domain, &section.to_string(), ORIGIN)
        .await
        .map_err(|e| {
            if client::no_owner(&e) {
                e
            } else {
                failure(plain_refusal(&message(&e)))
            }
        })
}

pub(crate) fn list_mut<'a>(
    o: &'a mut Map<String, Value>,
    field: &str,
) -> zbus::Result<&'a mut Vec<Value>> {
    o.entry(field)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| failure(format!("the {field} list isn't a list")))
}

pub(crate) fn find(list: &[Value], field: &str, key: &str) -> Option<usize> {
    list.iter().position(|e| e[field] == key)
}

/// The found entry's choice, made when missing.
pub(super) fn choice_mut<'a>(files: &'a mut Vec<Value>, id: &str) -> &'a mut Value {
    let i = find(files, "id", id).unwrap_or_else(|| {
        files.push(json!({"id": id, "enabled": null, "delay": null}));
        files.len() - 1
    });
    &mut files[i]
}

pub(super) fn var_index(vars: &[Value], name: &str) -> zbus::Result<usize> {
    find(vars, "name", name).ok_or_else(|| failure("That variable isn't there any more."))
}
