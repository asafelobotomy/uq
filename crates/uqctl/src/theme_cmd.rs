//! `uqctl theme`: show, list, switch, import, export and delete themes.
//!
//! Every message here is for a person, so none names the background service (K6).

use crate::profile_cmd::describe;
use clap::Subcommand;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use uq_core::theme::uqtheme;

/// The origin uqd's history records for uqctl's writes.
const ORIGIN: &str = "uqctl";
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";
const UNDERSTAND: &str = "Got an answer uqctl doesn't understand.";

#[derive(Subcommand, Debug, PartialEq)]
pub enum ThemeCmd {
    /// Show the theme in use and its variant.
    Get,
    /// List theme ids, or with --long also their names and whether they are yours.
    List {
        #[arg(long)]
        long: bool,
    },
    /// e.g. `uqctl theme set rosewood light`
    Set { id: String, variant: String },
    /// Add a theme from a .uqtheme file.
    Import {
        file: PathBuf,
        /// Replace a theme that has the same id.
        #[arg(long)]
        replace: bool,
        /// Also use the window gaps, corners and effects saved in the file.
        #[arg(long)]
        with_look: bool,
    },
    /// Write a theme's .uqtheme file to FILE, or to the screen without one.
    Export {
        id: String,
        file: Option<PathBuf>,
        /// Also save your current window gaps, corners and effects.
        #[arg(long)]
        with_look: bool,
    },
    /// Delete a theme of your own.
    Delete { id: String },
}

#[derive(Deserialize)]
struct Info {
    themes: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    id: String,
    name: String,
    #[serde(default)]
    user: bool,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    missing: bool,
}

#[derive(Deserialize)]
struct Deleted {
    #[serde(default)]
    switched: bool,
    #[serde(default)]
    profiles: Vec<String>,
}

/// Text from the service with its control characters left out, so a name can't carry a terminal
/// escape sequence to the screen.
fn plain(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// The lines `theme list` prints: ids only, or with `long` a marker, the name and "(yours)".
pub fn info_lines(json: &str, long: bool) -> Result<Vec<String>, String> {
    let info: Info = serde_json::from_str(json).map_err(|_| UNDERSTAND.to_string())?;
    Ok(info
        .themes
        .iter()
        .map(|t| {
            let id = plain(&t.id);
            if !long {
                return id;
            }
            let mark = if t.active { '*' } else { ' ' };
            let note = match (t.user, t.missing) {
                (_, true) => "  (its files are gone)",
                (true, false) => "  (yours)",
                (false, false) => "",
            };
            format!("{mark} {id}  {}{note}", plain(&t.name))
        })
        .collect())
}

/// The text of a theme file to import, refused when it is big or isn't text.
pub fn read_import(path: &Path) -> Result<String, String> {
    uqtheme::read_file(path).map_err(|e| plain(&e.to_string()))
}

/// The lines `theme delete` prints.
fn delete_lines(json: &str) -> Result<Vec<String>, String> {
    let d: Deleted = serde_json::from_str(json).map_err(|_| UNDERSTAND.to_string())?;
    let mut lines = Vec::new();
    if d.switched {
        lines.push("Switched to the default theme first.".to_string());
    }
    lines.extend(d.profiles.iter().map(|p| {
        format!(
            "The {} profile uses this theme. Pick another theme for it.",
            plain(p)
        )
    }));
    Ok(lines)
}

pub async fn run(proxy: &zbus::Proxy<'_>, cmd: ThemeCmd) -> Result<(), String> {
    match cmd {
        ThemeCmd::Get => {
            let (id, variant): (String, String) =
                proxy.call("GetTheme", &()).await.map_err(describe)?;
            println!("{} {}", plain(&id), plain(&variant));
        }
        ThemeCmd::List { long } if !long => {
            // Ids only and unchanged, as scripts and the VM checks read them.
            let ids: Vec<String> = proxy.call("ListThemes", &()).await.map_err(describe)?;
            ids.iter().for_each(|id| println!("{}", plain(id)));
        }
        ThemeCmd::List { long } => {
            let json: String = proxy.call("ListThemeInfo", &()).await.map_err(describe)?;
            info_lines(&json, long)?
                .iter()
                .for_each(|l| println!("{l}"));
        }
        ThemeCmd::Set { id, variant } => proxy
            .call::<_, _, ()>("SetTheme", &(id, variant, ORIGIN))
            .await
            .map_err(describe)?,
        ThemeCmd::Import {
            file,
            replace,
            with_look,
        } => {
            let text = read_import(&file)?;
            let id: String = proxy
                .call("ImportTheme", &(text, replace, with_look, ORIGIN))
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
            println!("Added theme: {}", plain(&id));
        }
        ThemeCmd::Export {
            id,
            file,
            with_look,
        } => {
            let text: String = proxy
                .call("ExportTheme", &(id, with_look))
                .await
                .map_err(describe)?;
            match file {
                Some(path) => uq_core::atomic::atomic_write(&path, &text)
                    .map_err(|e| format!("Can't write {}: {e}", path.display()))?,
                None => print!("{text}"),
            }
        }
        ThemeCmd::Delete { id } => {
            let json: String = proxy
                .call("DeleteTheme", &(id, ORIGIN))
                .await
                .map_err(describe)?;
            delete_lines(&json)?.iter().for_each(|l| println!("{l}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
