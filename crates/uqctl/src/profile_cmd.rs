//! `uqctl profile`: list, switch, export and import settings profiles.
//!
//! Every message here is for a person, so none names the background service (K6).

use clap::Subcommand;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use uq_core::profile::{self, Kept, next_after};

const NAME: &str = "org.uq.Desktop1";
const PATH: &str = "/org/uq/Desktop1";
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";

#[derive(Subcommand, Debug, PartialEq)]
pub enum ProfileCmd {
    /// List your profiles; the one that's on is marked with `*`.
    List,
    /// Turn a profile on.
    Apply { id: String },
    /// Go back to normal: turn the profile off.
    Off,
    /// Turn on the next profile; after the last one, go back to normal.
    Next,
    /// Write a profile's file to FILE, or to the screen without one.
    Export { id: String, file: Option<PathBuf> },
    /// Add a profile from a file.
    Import {
        file: PathBuf,
        /// Replace a profile that has the same id.
        #[arg(long)]
        replace: bool,
    },
    /// Delete a profile that isn't on.
    Delete { id: String },
}

#[derive(Deserialize)]
struct Row {
    id: String,
    name: String,
    keys: usize,
    #[serde(default)]
    active: bool,
}

#[derive(Deserialize)]
struct Report {
    active: String,
    #[serde(default)]
    kept: Vec<Kept>,
    #[serde(default)]
    notices: Vec<String>,
}

pub(crate) const NOT_RUNNING: &str = "Can't reach uQ's settings service. Is it running?";
const FAILED: &str = "uQ's settings service couldn't do that.";

fn rows(json: &str) -> Result<Vec<Row>, String> {
    serde_json::from_str(json).map_err(|_| "Got an answer uqctl doesn't understand.".to_string())
}

/// What a failed call says, in plain words.
///
/// A missing service is checked first: the bus's own message for it names D-Bus.
pub(crate) fn describe(err: zbus::Error) -> String {
    match err {
        e if uq_core::client::no_owner(&e) => NOT_RUNNING.into(),
        zbus::Error::MethodError(_, Some(msg), _) => msg,
        zbus::Error::MethodError(_, None, _) => FAILED.into(),
        _ => NOT_RUNNING.into(),
    }
}

/// The lines `profile list` prints.
pub fn profile_lines(json: &str) -> Result<Vec<String>, String> {
    let rows = rows(json)?;
    if rows.is_empty() {
        return Ok(vec![
            "No profiles yet. Make one on the Profiles page of uQ Settings.".into(),
        ]);
    }
    Ok(rows
        .iter()
        .map(|r| {
            let mark = if r.active { '*' } else { ' ' };
            let noun = if r.keys == 1 { "setting" } else { "settings" };
            format!("{mark} {}  {}  ({} {noun})", r.id, r.name, r.keys)
        })
        .collect())
}

/// The lines `profile apply`, `off` and `next` print.
pub fn report_lines(json: &str) -> Result<Vec<String>, String> {
    let r: Report = serde_json::from_str(json)
        .map_err(|_| "Got an answer uqctl doesn't understand.".to_string())?;
    let mut lines = vec![if r.active.is_empty() {
        "Back to normal".to_string()
    } else {
        format!("Profile on: {}", r.active)
    }];
    lines.extend(
        r.kept
            .iter()
            .map(|k| format!("Kept your own value: {}", k.label)),
    );
    lines.extend(r.notices);
    Ok(lines)
}

/// The profile `next` turns on; `None` means "back to normal" (or there are none).
pub fn next_target(list_json: &str) -> Result<Option<String>, String> {
    let rows = rows(list_json)?;
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let active = rows.iter().find(|r| r.active).map(|r| r.id.as_str());
    Ok(next_after(&ids, active))
}

/// The text of a file to import, refused when it is big or isn't text.
pub fn read_import(path: &Path) -> Result<String, String> {
    profile::read_file(path).map_err(|e| e.to_string())
}

/// Connect to the settings service, with a plain message when it isn't there.
pub async fn connect() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|_| NOT_RUNNING.to_string())?;
    zbus::Proxy::new(&conn, NAME, PATH, NAME)
        .await
        .map_err(describe)
}

fn print_lines(lines: Vec<String>) {
    lines.iter().for_each(|l| println!("{l}"));
}

async fn list_json(proxy: &zbus::Proxy<'_>) -> Result<String, String> {
    proxy.call("ListProfiles", &()).await.map_err(describe)
}

pub async fn run(proxy: &zbus::Proxy<'_>, cmd: ProfileCmd) -> Result<(), String> {
    match cmd {
        ProfileCmd::List => print_lines(profile_lines(&list_json(proxy).await?)?),
        ProfileCmd::Apply { id } => apply(proxy, &id).await?,
        ProfileCmd::Off => off(proxy).await?,
        ProfileCmd::Next => {
            let json = list_json(proxy).await?;
            if rows(&json)?.is_empty() {
                return Err(profile_lines(&json)?.remove(0));
            }
            match next_target(&json)? {
                Some(id) => apply(proxy, &id).await?,
                None => off(proxy).await?,
            }
        }
        ProfileCmd::Export { id, file } => {
            let text: String = proxy.call("GetProfile", &(id,)).await.map_err(describe)?;
            match file {
                Some(path) => uq_core::atomic::atomic_write(&path, &text)
                    .map_err(|e| format!("Can't write {}: {e}", path.display()))?,
                None => print!("{text}"),
            }
        }
        ProfileCmd::Import { file, replace } => {
            let text = read_import(&file)?;
            let id: String = proxy
                .call("SetProfile", &(text, replace))
                .await
                .map_err(|e| {
                    let conflict = matches!(&e, zbus::Error::MethodError(n, _, _)
                        if n.as_str() == CONFLICT);
                    let msg = describe(e);
                    if conflict {
                        format!("{msg} (run again with --replace)")
                    } else {
                        msg
                    }
                })?;
            println!("Added profile: {id}");
        }
        ProfileCmd::Delete { id } => proxy
            .call::<_, _, ()>("DeleteProfile", &(id,))
            .await
            .map_err(describe)?,
    }
    Ok(())
}

async fn apply(proxy: &zbus::Proxy<'_>, id: &str) -> Result<(), String> {
    let json: String = proxy.call("ApplyProfile", &(id,)).await.map_err(describe)?;
    print_lines(report_lines(&json)?);
    Ok(())
}

async fn off(proxy: &zbus::Proxy<'_>) -> Result<(), String> {
    let json: String = proxy.call("ProfileOff", &()).await.map_err(describe)?;
    print_lines(report_lines(&json)?);
    Ok(())
}

#[cfg(test)]
mod tests;
