//! uQ's own apps for the kinds a person never chose (the owner's choice, 2026-11-26): at
//! login, each kind with one of uQ's apps installed that has no choice in the person's lists is
//! given to that app, once. A kind is remembered as done either way, so a later choice, or a
//! reset to the system's app, is never undone at the next login.

use super::{Handler, KINDS, set_defaults};
use crate::mimeapps::default_for;

/// What a fill would change: the person's new `mimeapps.list` (when anything was set) and the
/// kinds now done.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fill {
    pub text: Option<String>,
    pub done: Vec<String>,
    /// The kinds given to one of uQ's apps, as `(kind, app)`.
    pub set: Vec<(&'static str, String)>,
}

/// `user` is the person's `mimeapps.list`, `overrides` their desktop's own lists (which win over
/// it), `done` the kinds already handled at an earlier login.
pub fn fill_native(user: &str, overrides: &[String], apps: &[Handler], done: &[String]) -> Fill {
    let mut out = Fill {
        done: done.to_vec(),
        ..Fill::default()
    };
    let mut text = user.to_string();
    for kind in KINDS.iter().filter(|k| !k.native.is_empty()) {
        if out.done.iter().any(|d| d == kind.id) {
            continue;
        }
        // Not installed yet: try again at a later login.
        let Some(app) = kind.native_in(apps) else {
            continue;
        };
        out.done.push(kind.id.to_string());
        let chosen = std::iter::once(text.as_str())
            .chain(overrides.iter().map(String::as_str))
            .any(|t| default_for(t, kind.primary()).is_some());
        if chosen {
            continue;
        }
        text = set_defaults(&text, &kind.mimes_for(app), &app.id);
        out.set.push((kind.id, app.id.clone()));
    }
    if !out.set.is_empty() {
        out.text = Some(text);
    }
    out
}

/// The kinds in a done file: one id per line, unknown ones dropped.
pub fn parse_done(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| KINDS.iter().any(|k| k.id == *l))
        .map(str::to_string)
        .collect()
}

pub fn write_done(done: &[String]) -> String {
    let mut s =
        String::from("# Kinds uQ has offered its own apps for; see Settings › Default apps.\n");
    for d in done {
        s.push_str(d);
        s.push('\n');
    }
    s
}
