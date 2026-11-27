//! The Startup page's session settings: what an app waits for, restore the last session, and
//! the commands run at logout. Each is one `autostart` section write, checked by uq-core first.

use serde_json::{Map, Value, json};
use uq_core::autostart::When;
use uq_core::client::DesktopProxy;
use uq_core::desktop_entry::{ExecError, split_command};

use super::StartupState;
use super::edit::{command_error, failure, find, list_mut, write_section};

fn when_mut(o: &mut Map<String, Value>) -> zbus::Result<&mut Map<String, Value>> {
    o.entry("when")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| failure("the conditions aren't a set"))
}

impl StartupState {
    /// `when` is "network", "power", or `None` to start right away.
    pub(super) async fn set_when(
        &self,
        proxy: &DesktopProxy<'_>,
        id: &str,
        when: Option<&str>,
    ) -> zbus::Result<()> {
        let when = match when {
            None | Some("") => None,
            Some(w) => Some(
                When::parse(w).ok_or_else(|| failure("An app waits for the network or power."))?,
            ),
        };
        write_section(proxy, "autostart", |o| {
            let m = when_mut(o)?;
            match when {
                Some(w) => m.insert(id.to_string(), json!(w.key())),
                None => m.remove(id),
            };
            if m.is_empty() {
                o.remove("when");
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn set_restore(&self, proxy: &DesktopProxy<'_>, on: bool) -> zbus::Result<()> {
        write_section(proxy, "autostart", |o| {
            o.insert("restore".into(), json!(on));
            Ok(())
        })
        .await
    }

    pub(super) async fn add_logout(
        &self,
        proxy: &DesktopProxy<'_>,
        name: &str,
        command: &str,
    ) -> zbus::Result<()> {
        let argv = split_command(command.trim()).map_err(|e| failure(command_error(&e)))?;
        let Some(program) = argv.first() else {
            return Err(failure(command_error(&ExecError::Empty)));
        };
        let name = match name.trim() {
            "" => program.rsplit('/').next().unwrap_or(program),
            typed => typed,
        };
        let entry = json!({"name": name, "argv": argv, "enabled": true});
        write_section(proxy, "autostart", |o| {
            list_mut(o, "logout")?.push(entry);
            Ok(())
        })
        .await
    }

    pub(super) async fn set_logout_enabled(
        &self,
        proxy: &DesktopProxy<'_>,
        id: &str,
        on: bool,
    ) -> zbus::Result<()> {
        write_section(proxy, "autostart", |o| {
            let list = list_mut(o, "logout")?;
            let i = find(list, "id", id).ok_or_else(|| failure("That command was removed."))?;
            list[i]["enabled"] = json!(on);
            Ok(())
        })
        .await
    }

    pub(super) async fn remove_logout(
        &self,
        proxy: &DesktopProxy<'_>,
        id: &str,
    ) -> zbus::Result<()> {
        write_section(proxy, "autostart", |o| {
            let list = list_mut(o, "logout")?;
            let i = find(list, "id", id).ok_or_else(|| failure("That command was removed."))?;
            list.remove(i);
            Ok(())
        })
        .await
    }
}

/// The logout commands as the page shows them: `{id, name, enabled, command_text}`.
pub(super) fn logout_rows(section: &Value) -> Vec<Value> {
    section["logout"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|c| {
            let argv: Vec<String> = c["argv"]
                .as_array()
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect();
            json!({"id": c["id"], "name": c["name"], "enabled": c["enabled"] != false,
                "command_text": uq_core::desktop_entry::quote_command(&argv)})
        })
        .collect()
}
