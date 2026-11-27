//! The History line for a Startup change.

use crate::desktop_entry::{clean_display, valid_desktop_id};
use crate::env;
use serde_json::Value;

const MAX_NAMED: usize = 3;

/// A found entry as History names it: its id without `.desktop`, or "an app" for an id that
/// isn't one (a hand-edited journal).
fn file_name(id: &str) -> String {
    if valid_desktop_id(id) {
        env::shown(id.strip_suffix(".desktop").unwrap_or(id))
    } else {
        "an app".to_string()
    }
}

/// An added entry's name, on one line; "an entry" when there is nothing to show.
fn added_name(v: &Value) -> String {
    let name = v.get("name").and_then(Value::as_str).unwrap_or("");
    let clean = clean_display(name, 100);
    let clean = clean.trim();
    if clean.is_empty() {
        "an entry".to_string()
    } else {
        env::shown(clean)
    }
}

fn list<'a>(v: &'a Value, f: &str) -> Vec<(&'a str, &'a Value)> {
    v.get(f)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| Some((e.get("id")?.as_str()?, e)))
        .collect()
}

fn delay_line(name: &str, after: &Value) -> String {
    match after.get("delay").and_then(Value::as_u64) {
        Some(d) => format!("Start delay for {name} set to {d} seconds"),
        None => format!("Start delay for {name} removed"),
    }
}

/// What changed in one entry, in summary words: on/off first, then the delay, then start hidden.
fn entry_lines(name: &str, before: &Value, after: &Value, out: &mut Vec<String>) {
    let on = |v: &Value| v.get("enabled").and_then(Value::as_bool);
    let start = out.len();
    if on(before) != on(after) {
        out.push(match on(after) {
            Some(true) => format!("Startup app {name} turned on"),
            Some(false) => format!("Startup app {name} turned off"),
            None => format!("Startup app {name} set back to its default"),
        });
    }
    let delay = |v: &'_ Value| v.get("delay").filter(|d| !d.is_null()).cloned();
    if delay(before) != delay(after) {
        out.push(delay_line(name, after));
    }
    let hide = |v: &Value| v.get("minimize").and_then(Value::as_bool).unwrap_or(false);
    if hide(before) != hide(after) {
        out.push(if hide(after) {
            format!("Startup app {name} set to start hidden")
        } else {
            format!("Startup app {name} set to start normally")
        });
    }
    if out.len() == start && before != after {
        out.push(format!("Startup app {name} changed"));
    }
}

/// "Startup app nm-applet turned off", "Start delay for Syncthing set to 10 seconds",
/// "Startup app Backup added" or "… removed": up to three, then ", and N more". Entries are
/// named, never shown as JSON or file names.
pub fn summary(before: &Value, after: &Value) -> String {
    let mut lines = Vec::new();
    let null = Value::Null;
    let (bf, af) = (list(before, "files"), list(after, "files"));
    for (id, a) in &af {
        let b = bf.iter().find(|(i, _)| i == id).map_or(&null, |(_, b)| *b);
        entry_lines(&file_name(id), b, a, &mut lines);
    }
    for (id, b) in &bf {
        if !af.iter().any(|(i, _)| i == id) {
            entry_lines(&file_name(id), b, &null, &mut lines);
        }
    }
    let (ba, aa) = (list(before, "added"), list(after, "added"));
    for (id, a) in &aa {
        match ba.iter().find(|(i, _)| i == id) {
            None => lines.push(format!("Startup app {} added", added_name(a))),
            Some((_, b)) => entry_lines(&added_name(a), b, a, &mut lines),
        }
    }
    for (id, b) in &ba {
        if !aa.iter().any(|(i, _)| i == id) {
            lines.push(format!("Startup app {} removed", added_name(b)));
        }
    }
    session_lines(before, after, &mut lines);
    if lines.is_empty() {
        let same_order = ba.iter().map(|(i, _)| i).eq(aa.iter().map(|(i, _)| i));
        return if same_order {
            "Startup apps".to_string()
        } else {
            "Startup apps reordered".to_string()
        };
    }
    let mut line = lines[..lines.len().min(MAX_NAMED)].join(", ");
    if lines.len() > MAX_NAMED {
        line.push_str(&format!(", and {} more", lines.len() - MAX_NAMED));
    }
    line
}

/// Restore the last session, the conditions and the commands at logout.
fn session_lines(before: &Value, after: &Value, out: &mut Vec<String>) {
    let restore = |v: &Value| v.get("restore").and_then(Value::as_bool).unwrap_or(false);
    if restore(before) != restore(after) {
        out.push(if restore(after) {
            "Restore last session turned on".to_string()
        } else {
            "Restore last session turned off".to_string()
        });
    }
    let when = |v: &Value| v.get("when").cloned().unwrap_or(Value::Null);
    let (bw, aw) = (when(before), when(after));
    let name = |id: &str| {
        let named = list(after, "added")
            .into_iter()
            .chain(list(before, "added"))
            .find(|(i, _)| *i == id)
            .map(|(_, v)| added_name(v));
        named.unwrap_or_else(|| file_name(id))
    };
    if let Some(m) = aw.as_object() {
        for (id, w) in m {
            if bw.get(id) != Some(w) {
                let what = match w.as_str() {
                    Some("network") => "waits for the network",
                    Some("power") => "starts only on mains power",
                    _ => "has a new condition",
                };
                out.push(format!("Startup app {} {what}", name(id)));
            }
        }
    }
    if let Some(m) = bw.as_object() {
        for id in m.keys() {
            if aw.get(id).is_none() {
                out.push(format!("Startup app {} starts without waiting", name(id)));
            }
        }
    }
    let (bl, al) = (list(before, "logout"), list(after, "logout"));
    for (id, a) in &al {
        match bl.iter().find(|(i, _)| i == id) {
            None => out.push(format!("Command at logout {} added", added_name(a))),
            Some((_, b)) if b != a => {
                out.push(format!("Command at logout {} changed", added_name(a)))
            }
            _ => {}
        }
    }
    for (id, b) in &bl {
        if !al.iter().any(|(i, _)| i == id) {
            out.push(format!("Command at logout {} removed", added_name(b)));
        }
    }
}
