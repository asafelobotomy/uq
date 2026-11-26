//! Desktop Entry handling shared by the bridge, the autostart runner and the settings pages: the
//! `Exec` parser and writer, the `[Desktop Entry]` reader, safe ids and a line-preserving key
//! setter. Pure text work; nothing here runs a process or touches the disk.

use crate::settings::valid_pinned_id;
use std::path::{Path, PathBuf};
mod exec;
pub use exec::*;
mod keys;
pub use keys::*;

/// Make a string safe to show on one line: control and bidi format characters become spaces,
/// then it is cut to `max` characters.
pub fn clean_display(s: &str, max: usize) -> String {
    s.chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '\u{202A}'..='\u{202E}'
                        | '\u{2066}'..='\u{2069}'
                        | '\u{2028}'
                        | '\u{2029}'
                        | '\u{200E}'
                        | '\u{200F}'
                        | '\u{061C}'
                )
            {
                ' '
            } else {
                c
            }
        })
        .take(max)
        .collect()
}

/// Apply the key-value escapes of the desktop-entry format so a value is one line.
pub fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// Undo the key-value escapes of the desktop-entry format (`\s \n \t \r \\`).
pub(crate) fn unescape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A `;`-separated list, the trailing `;` optional, `\;` a literal one; items are unescaped.
fn split_list(v: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut it = v.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' if it.peek() == Some(&';') => {
                it.next();
                cur.push(';');
            }
            '\\' => {
                cur.push('\\');
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            ';' => items.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    items.push(cur);
    items
        .into_iter()
        .map(|i| unescape_value(&i))
        .filter(|i| !i.is_empty())
        .collect()
}

/// The keys of the `[Desktop Entry]` group that uQ reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub terminal: bool,
    /// `X-TerminalArgExec`: the argument a terminal takes before the command to run.
    pub terminal_arg: Option<String>,
    pub hidden: bool,
    pub only_show_in: Option<Vec<String>>,
    pub not_show_in: Option<Vec<String>>,
    pub dbus_activatable: bool,
    pub path: Option<String>,
    pub wm_class: Option<String>,
    pub gnome_enabled: Option<bool>,
    pub gnome_delay: Option<u32>,
    pub uq_override: bool,
    pub uq_source: Option<String>,
    pub uq_autostart: bool,
    /// `MimeType`: the file types the app opens.
    pub mime_types: Vec<String>,
}

/// Read the `[Desktop Entry]` group; the last occurrence of a key wins.
pub fn read_entry(text: &str) -> Entry {
    let mut e = Entry::default();
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        let s = || Some(unescape_value(v));
        let b = v == "true";
        match k.trim() {
            "Type" => e.kind = Some(v.to_string()),
            "Name" => e.name = s(),
            "Icon" => e.icon = s(),
            "Exec" => e.exec = s(),
            "TryExec" => e.try_exec = s(),
            "Terminal" => e.terminal = b,
            "X-TerminalArgExec" => e.terminal_arg = s(),
            "Hidden" => e.hidden = b,
            "OnlyShowIn" => e.only_show_in = Some(split_list(v)).filter(|l| !l.is_empty()),
            "NotShowIn" => e.not_show_in = Some(split_list(v)).filter(|l| !l.is_empty()),
            "DBusActivatable" => e.dbus_activatable = b,
            "Path" => e.path = s(),
            "StartupWMClass" => e.wm_class = s(),
            "X-GNOME-Autostart-enabled" => e.gnome_enabled = Some(b),
            "X-GNOME-Autostart-Delay" => e.gnome_delay = v.parse().ok(),
            "X-uQ-Override" => e.uq_override = b,
            "X-uQ-Source" => e.uq_source = s(),
            "X-uQ-Autostart" => e.uq_autostart = b,
            "MimeType" => e.mime_types = split_list(v),
            _ => {}
        }
    }
    e
}

/// The desktops uQ answers to in `OnlyShowIn` and `NotShowIn`.
pub const UQ_DESKTOPS: [&str; 2] = ["Hyprland", "uQ"];

/// Whether the entry is for these desktops: `OnlyShowIn` needs an overlap and decides on its
/// own; otherwise `NotShowIn` refuses one.
pub fn shows_in(e: &Entry, desktops: &[&str]) -> bool {
    let hit = |l: &Vec<String>| l.iter().any(|d| desktops.contains(&d.as_str()));
    match (&e.only_show_in, &e.not_show_in) {
        (Some(only), _) => hit(only),
        (None, Some(not)) => !hit(not),
        (None, None) => true,
    }
}

/// `id` with `.desktop` added when it is missing.
pub fn normalise_id(id: &str) -> String {
    if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    }
}

/// A desktop id that is safe to join onto a directory: `valid_pinned_id` once normalised, with
/// no leading `.` and no `..`.
pub fn valid_desktop_id(id: &str) -> bool {
    let n = normalise_id(id);
    valid_pinned_id(&n) && !n.starts_with('.') && !n.contains("..")
}

/// Where a marked override's `X-uQ-Source` may point (R19, F20): exactly `<d>/<id>` for an
/// `applications` folder `d` in `system` that isn't `home`. The bridge and the autostart runner
/// both launch from such a source, with the fix, instead of the override (which already carries
/// it). Pure: the caller checks that the path is a regular file.
pub fn valid_override_source(
    home: Option<&Path>,
    system: &[PathBuf],
    id: &str,
    source: &str,
) -> Option<PathBuf> {
    if !valid_desktop_id(id) {
        return None;
    }
    let path = PathBuf::from(source);
    let file = normalise_id(id);
    system
        .iter()
        .filter(|d| home != Some(d.as_path()))
        .any(|d| d.join(&file) == path)
        .then_some(path)
}

/// A systemd unit-name fragment for an app id: alphanumerics, `_` and `-`, the rest `_`, at most 64.
pub fn unit_id(app_id: &str) -> String {
    app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// The argv that runs `argv` in a transient user scope of its own, `app-uq-<id>-<rand>.scope`
/// in `app.slice`, as the launcher's apps run: there, a scheduler such as scdr counts it as an
/// app (its focus boost, its weight under load) rather than as part of whatever started it.
pub fn scope_argv(app_id: &str, argv: &[String], rand: u32) -> Vec<String> {
    let mut out: Vec<String> = [
        "systemd-run",
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        "--slice=app.slice",
    ]
    .map(String::from)
    .into();
    out.push(format!("--unit=app-uq-{}-{rand:08x}", unit_id(app_id)));
    out.push("--".into());
    out.extend(argv.iter().cloned());
    out
}

/// A small non-cryptographic value: it only keeps scope names apart.
pub fn random_suffix() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut x =
        nanos ^ std::process::id().rotate_left(16) ^ N.fetch_add(0x9e37_79b9, Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^ (x << 5)
}

#[cfg(test)]
mod tests;
