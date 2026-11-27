//! The Default apps page's state: each kind with its choice and the apps offered, every file and
//! link type an installed app opens, and the apps by id.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use uq_core::default_apps::{self as da, Handler, KINDS, NATIVE_APPS};

use super::{DefaultAppsState, mimeapps, read_capped};

/// The current choice for `mime` and where it comes from: the desktop's own list
/// (`override`), the person's (`you`) or the system's (`system`).
struct Lists {
    overrides: Vec<String>,
    user: String,
    system: Vec<String>,
}

impl Lists {
    fn current(&self, mime: &str) -> (Option<String>, &'static str) {
        if let Some(id) = self
            .overrides
            .iter()
            .find_map(|t| mimeapps::default_for(t, mime))
        {
            return (Some(id), "override");
        }
        if let Some(id) = mimeapps::default_for(&self.user, mime) {
            return (Some(id), "you");
        }
        if let Some(id) = self
            .system
            .iter()
            .find_map(|t| mimeapps::default_for(t, mime))
        {
            return (Some(id), "system");
        }
        (None, "")
    }
}

/// Which list a type goes under on the File types and Links tabs.
fn group(mime: &str) -> (&'static str, &'static str) {
    if mime.starts_with("x-scheme-handler/") {
        return ("links", "Links");
    }
    match mime.split('/').next().unwrap_or("") {
        "image" => ("images", "Pictures"),
        "video" => ("video", "Videos"),
        "audio" => ("audio", "Music and sound"),
        "text" => ("text", "Text"),
        "inode" => ("folders", "Folders"),
        "font" => ("fonts", "Fonts"),
        "application" => ("documents", "Documents and other files"),
        _ => ("other", "Other"),
    }
}

/// The most types listed; a system with more lists the first ones, by name.
pub const MAX_TYPES: usize = 3000;

pub fn build(page: &DefaultAppsState, handlers: &[Handler]) -> Value {
    let lists = Lists {
        overrides: page.overrides.iter().map(|f| read_capped(f)).collect(),
        user: read_capped(&page.file),
        system: page.system_files.iter().map(|f| read_capped(f)).collect(),
    };
    let native = |id: &str| NATIVE_APPS.contains(&id);
    let mut apps = Map::new();
    for h in handlers {
        apps.insert(
            h.id.clone(),
            json!({"name": h.name, "icon": h.icon, "native": native(&h.id)}),
        );
    }
    let mut native_left = false;
    let kinds: Vec<Value> = KINDS
        .iter()
        .map(|k| {
            let (current, source) = lists.current(k.primary());
            let offered: Vec<Value> = handlers
                .iter()
                .filter(|h| k.offers(h))
                .map(|h| {
                    json!({"id": h.id, "name": h.name, "icon": h.icon,
                    "native": native(&h.id)})
                })
                .collect();
            let pick = k.native_in(handlers).map(|h| h.id.clone());
            if pick.is_some() && pick != current {
                native_left = true;
            }
            json!({"id": k.id, "label": k.label, "current": current, "source": source,
                "apps": offered, "native": pick})
        })
        .collect();

    // Every type some installed app opens, with the apps that open it.
    let mut by_type: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for h in handlers {
        for m in &h.mimes {
            if DefaultAppsState::valid_type(m) {
                let v = by_type.entry(m.as_str()).or_default();
                if !v.contains(&h.id.as_str()) {
                    v.push(h.id.as_str());
                }
            }
        }
    }
    let mut types: Vec<Value> = by_type
        .into_iter()
        .map(|(mime, ids)| {
            let (current, source) = lists.current(mime);
            let label = match da::info::scheme(mime) {
                Some(s) => da::info::describe_scheme(s),
                None => da::info::describe(&page.mime_dirs, mime).unwrap_or_default(),
            };
            let (gid, glabel) = group(mime);
            json!({"mime": mime, "label": label, "group": gid, "group_label": glabel,
                "current": current, "source": source, "apps": ids})
        })
        .collect();
    let key = |v: &Value| {
        let l = v["label"].as_str().unwrap_or("");
        let m = v["mime"].as_str().unwrap_or("");
        (
            v["group"].as_str().unwrap_or("").to_string(),
            if l.is_empty() { m } else { l }.to_lowercase(),
        )
    };
    types.sort_by_key(key);
    types.truncate(MAX_TYPES);

    let override_file = page
        .overrides
        .iter()
        .zip(&lists.overrides)
        .find(|(_, t)| t.contains('='))
        .and_then(|(f, _)| f.file_name()?.to_str().map(str::to_string));
    json!({
        "kinds": kinds,
        "apps": apps,
        "types": types,
        "native_left": native_left,
        "override_file": override_file,
    })
}
