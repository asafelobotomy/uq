//! `Explain` rows for environment variables, Apps and scaling and startup apps. Pure: uqd reads
//! `user.lua`, the last rejection and what its file sync found, and calls these; the app's fake
//! uqd calls them too.

use crate::compat::AppFix;
use crate::compat::hypr::{SWITCH_ENV, SwitchKey};
use crate::settings::provenance::find_env_in_user_lua;
use crate::state::State;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// What uqd's file sync found on disk for one browser's flags file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserFile {
    Ours,
    OwnFile,
    None,
    Error(String),
}

/// What uqd's file sync found for one app fix's launcher-file override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideState {
    Written,
    OwnFile,
    NoSource,
    Error(String),
    Off,
}

/// The file sync's last results (filled by uqd, built by hand by the fake uqd).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileReport {
    /// Why the session variables file couldn't be written, if it couldn't.
    pub env_file: Option<String>,
    /// Each installed browser's flags file, by browser name.
    pub browsers: Vec<(String, BrowserFile)>,
    /// Each app fix's override, by desktop id.
    pub overrides: BTreeMap<String, OverrideState>,
    /// Why a hidden startup copy couldn't be written or removed, if it couldn't.
    pub autostart: Option<String>,
}

fn row(source: &str, detail: Value) -> Value {
    json!({ "source": source, "live": null, "stored": null, "detail": detail })
}

fn lua_detail(line: u32, path: &str) -> Value {
    json!({ "line": line, "path": path })
}

/// The first switch that is on and sets variable `name`.
fn switch_setting(state: &State, name: &str) -> Option<SwitchKey> {
    SWITCH_ENV
        .iter()
        .find(|(key, n, _)| *n == name && state.compat.switch(*key))
        .map(|(key, _, _)| *key)
}

/// `Explain("env")`: one row per variable, keyed `vars.<NAME>`, each `{source, live, stored,
/// detail}`. `source` is `error` (the last write was rejected, or the session variables file
/// couldn't be written), `user_lua` (a line of `user.lua` sets it) or `uq`; a `uq` row says
/// which switch replaced the value when one did. After a failure there is also an `env` row.
pub fn explain_env(
    state: &State,
    user_lua: Option<&str>,
    rejected: Option<&str>,
    report: &FileReport,
    path: &str,
) -> Value {
    let failure = rejected.or(report.env_file.as_deref());
    let mut out = Map::new();
    if let Some(message) = failure {
        out.insert("env".into(), row("error", json!({ "message": message })));
    }
    for v in &state.env.vars {
        let found = user_lua.and_then(|t| find_env_in_user_lua(t, &v.name));
        let r = match (failure, found) {
            (Some(message), _) => row("error", json!({ "message": message })),
            (None, Some(line)) => row("user_lua", lua_detail(line, path)),
            (None, None) => match switch_setting(state, &v.name) {
                Some(key) => row("uq", json!({ "replaced_by": key.key() })),
                None => row("uq", json!({})),
            },
        };
        out.insert(format!("vars.{}", v.name), r);
    }
    Value::Object(out)
}

fn browser_detail(browsers: &[(String, BrowserFile)]) -> Value {
    let list: Vec<Value> = browsers
        .iter()
        .map(|(name, file)| match file {
            BrowserFile::Ours => json!({ "name": name, "file": "ours" }),
            BrowserFile::OwnFile => json!({ "name": name, "file": "own_file" }),
            BrowserFile::None => json!({ "name": name, "file": "none" }),
            BrowserFile::Error(m) => json!({ "name": name, "file": "error", "message": m }),
        })
        .collect();
    json!({ "browsers": list })
}

fn app_row(report: &FileReport, fix: &AppFix) -> Value {
    match report.overrides.get(&fix.id).unwrap_or(&OverrideState::Off) {
        OverrideState::Written => row("uq", json!({ "override": "written" })),
        OverrideState::OwnFile => row(
            "uq",
            json!({ "override": "own_file",
                "message": "This app's launcher file isn't uQ's, so uQ left it alone." }),
        ),
        OverrideState::NoSource => row(
            if fix.elsewhere { "error" } else { "uq" },
            json!({ "override": "no_source",
                "message": "uQ couldn't find this app's launcher file." }),
        ),
        OverrideState::Error(m) => row("error", json!({ "override": "error", "message": m })),
        OverrideState::Off => row("uq", json!({ "override": "off" })),
    }
}

/// `Explain("compat")` for every row except `sharp_x11`, which the generic option path gives:
/// one row per other switch (keyed by its section key), one `apps.<id>` row per fix, and a
/// `compat` row after a rejected write. A rejection marks every row `error`.
pub fn explain_compat_extra(
    state: &State,
    user_lua: Option<&str>,
    rejected: Option<&str>,
    report: &FileReport,
    path: &str,
) -> Value {
    let mut out = Map::new();
    let rejection = rejected.map(|m| json!({ "message": m }));
    if let Some(d) = &rejection {
        out.insert("compat".into(), row("error", d.clone()));
    }
    for key in SwitchKey::ALL
        .into_iter()
        .filter(|k| *k != SwitchKey::SharpX11)
    {
        let r = if let Some(d) = &rejection {
            row("error", d.clone())
        } else if key == SwitchKey::ChromiumWayland {
            let failed = report
                .browsers
                .iter()
                .any(|(_, f)| matches!(f, BrowserFile::Error(_)));
            row(
                if failed { "error" } else { "uq" },
                browser_detail(&report.browsers),
            )
        } else {
            let found = user_lua.and_then(|t| {
                SWITCH_ENV
                    .iter()
                    .filter(|(k, _, _)| *k == key)
                    .find_map(|(_, name, _)| find_env_in_user_lua(t, name))
            });
            match found {
                Some(line) => row("user_lua", lua_detail(line, path)),
                None => row("uq", json!({})),
            }
        };
        out.insert(key.key().to_string(), r);
    }
    for fix in &state.compat.apps {
        let r = match &rejection {
            Some(d) => row("error", d.clone()),
            None => app_row(report, fix),
        };
        out.insert(format!("apps.{}", fix.id), r);
    }
    Value::Object(out)
}

/// F6: `sharp_x11` reports `default` while it's off, since uQ renders nothing then. A
/// `user_lua` or `error` row is left alone.
pub fn default_when_off(rows: &mut Value, state: &State) {
    if state.compat.sharp_x11 {
        return;
    }
    if let Some(r) = rows.get_mut("sharp_x11")
        && r["source"] == "uq"
    {
        r["source"] = json!("default");
    }
}

/// `Explain("autostart")`: a `autostart` row with `error` when a hidden copy couldn't be
/// written or removed, else `uq` rows keyed by entry id.
pub fn explain_autostart(state: &State, report: &FileReport) -> Value {
    let mut out = Map::new();
    if let Some(message) = &report.autostart {
        out.insert(
            "autostart".into(),
            row("error", json!({ "message": message })),
        );
        return Value::Object(out);
    }
    let a = &state.autostart;
    for id in a
        .files
        .iter()
        .map(|f| f.id.as_str())
        .chain(a.added.iter().map(|e| e.id()))
    {
        out.insert(id.to_string(), row("uq", json!({})));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests;
