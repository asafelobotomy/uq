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
mod tests {
    use super::*;
    use crate::env::Var;

    const PATH: &str = "/home/u/user.lua";

    fn state_with_env(vars: &[(&str, &str)]) -> State {
        let mut st = State::default();
        st.env.vars = vars
            .iter()
            .map(|(n, v)| Var {
                name: n.to_string(),
                value: v.to_string(),
            })
            .collect();
        st
    }

    #[test]
    fn env_row_user_lua_line() {
        let st = state_with_env(&[("FOO", "x"), ("BAR", "y")]);
        let lua = "-- top\n\nhl.env(\"FOO\", \"x\")\n-- hl.env(\"BAR\", \"y\")\n";
        let v = explain_env(&st, Some(lua), None, &FileReport::default(), PATH);
        assert_eq!(v["vars.FOO"]["source"], "user_lua");
        assert_eq!(v["vars.FOO"]["detail"], json!({"line": 3, "path": PATH}));
        assert_eq!(v["vars.BAR"]["source"], "uq");
        assert_eq!(v["vars.BAR"]["live"], Value::Null);
        assert_eq!(v["vars.BAR"]["stored"], Value::Null);
        assert!(v.get("env").is_none());
    }

    #[test]
    fn env_row_replaced_by_switch() {
        let mut st = state_with_env(&[("QT_QPA_PLATFORM", "xcb"), ("FOO", "x")]);
        st.compat.qt_wayland = true;
        let v = explain_env(&st, None, None, &FileReport::default(), PATH);
        assert_eq!(v["vars.QT_QPA_PLATFORM"]["source"], "uq");
        assert_eq!(
            v["vars.QT_QPA_PLATFORM"]["detail"],
            json!({"replaced_by": "qt_wayland"})
        );
        assert_eq!(v["vars.FOO"]["detail"], json!({}));
        st.compat.qt_wayland = false;
        let v = explain_env(&st, None, None, &FileReport::default(), PATH);
        assert_eq!(v["vars.QT_QPA_PLATFORM"]["detail"], json!({}));
    }

    #[test]
    fn env_rejected_gives_section_row() {
        let v = explain_env(
            &State::default(),
            None,
            Some("bad"),
            &FileReport::default(),
            PATH,
        );
        assert_eq!(
            v,
            json!({"env": {"source": "error", "live": null, "stored": null,
                "detail": {"message": "bad"}}})
        );
        let st = state_with_env(&[("FOO", "x")]);
        let lua = "hl.env(\"FOO\", \"x\")\n";
        let v = explain_env(&st, Some(lua), Some("bad"), &FileReport::default(), PATH);
        assert_eq!(v["vars.FOO"]["source"], "error");
        assert_eq!(v["env"]["source"], "error");
    }

    #[test]
    fn env_file_error_reported() {
        let st = state_with_env(&[("FOO", "x")]);
        let report = FileReport {
            env_file: Some("Permission denied".into()),
            ..FileReport::default()
        };
        let v = explain_env(&st, None, None, &report, PATH);
        assert_eq!(v["vars.FOO"]["source"], "error");
        assert_eq!(
            v["vars.FOO"]["detail"],
            json!({"message": "Permission denied"})
        );
    }

    #[test]
    fn compat_switch_user_lua() {
        let mut st = State::default();
        st.compat.electron_wayland = true;
        st.compat.qt_wayland = true;
        let lua = "x = 1\nhl.env('ELECTRON_OZONE_PLATFORM_HINT', 'x11')\n";
        let v = explain_compat_extra(&st, Some(lua), None, &FileReport::default(), PATH);
        assert_eq!(v["electron_wayland"]["source"], "user_lua");
        assert_eq!(
            v["electron_wayland"]["detail"],
            json!({"line": 2, "path": PATH})
        );
        assert_eq!(v["qt_wayland"]["source"], "uq");
        assert_eq!(v["steam_scale"]["source"], "uq");
        assert!(v.get("sharp_x11").is_none());
        assert!(v.get("compat").is_none());
    }

    #[test]
    fn compat_browser_details() {
        let report = FileReport {
            browsers: vec![
                ("Chromium".into(), BrowserFile::Ours),
                ("Brave".into(), BrowserFile::OwnFile),
                ("Vivaldi".into(), BrowserFile::None),
            ],
            ..FileReport::default()
        };
        let v = explain_compat_extra(&State::default(), None, None, &report, PATH);
        assert_eq!(v["chromium_wayland"]["source"], "uq");
        assert_eq!(
            v["chromium_wayland"]["detail"],
            json!({"browsers": [
                {"name": "Chromium", "file": "ours"},
                {"name": "Brave", "file": "own_file"},
                {"name": "Vivaldi", "file": "none"},
            ]})
        );
        let report = FileReport {
            browsers: vec![("Brave".into(), BrowserFile::Error("Read-only".into()))],
            ..FileReport::default()
        };
        let v = explain_compat_extra(&State::default(), None, None, &report, PATH);
        assert_eq!(v["chromium_wayland"]["source"], "error");
        assert_eq!(
            v["chromium_wayland"]["detail"]["browsers"][0],
            json!({"name": "Brave", "file": "error", "message": "Read-only"})
        );
    }

    #[test]
    fn compat_app_override_states() {
        let mut st = State::default();
        let mut elsewhere = AppFix::new("e.desktop");
        elsewhere.elsewhere = true;
        st.compat.apps = vec![
            AppFix::new("w.desktop"),
            AppFix::new("o.desktop"),
            AppFix::new("n.desktop"),
            elsewhere,
            AppFix::new("x.desktop"),
            AppFix::new("f.desktop"),
        ];
        let mut report = FileReport::default();
        report
            .overrides
            .insert("w.desktop".into(), OverrideState::Written);
        report
            .overrides
            .insert("o.desktop".into(), OverrideState::OwnFile);
        report
            .overrides
            .insert("n.desktop".into(), OverrideState::NoSource);
        report
            .overrides
            .insert("e.desktop".into(), OverrideState::NoSource);
        report
            .overrides
            .insert("x.desktop".into(), OverrideState::Error("Disk full".into()));
        let v = explain_compat_extra(&st, None, None, &report, PATH);
        let row = |id: &str| &v[format!("apps.{id}")];
        assert_eq!(row("w.desktop")["source"], "uq");
        assert_eq!(row("w.desktop")["detail"], json!({"override": "written"}));
        assert_eq!(row("o.desktop")["source"], "uq");
        assert_eq!(row("o.desktop")["detail"]["override"], "own_file");
        assert_eq!(row("n.desktop")["source"], "uq");
        assert_eq!(row("n.desktop")["detail"]["override"], "no_source");
        assert_eq!(row("e.desktop")["source"], "error");
        assert_eq!(row("e.desktop")["detail"]["override"], "no_source");
        assert!(row("e.desktop")["detail"]["message"].is_string());
        assert_eq!(row("x.desktop")["source"], "error");
        assert_eq!(
            row("x.desktop")["detail"],
            json!({"override": "error", "message": "Disk full"})
        );
        assert_eq!(row("f.desktop")["detail"], json!({"override": "off"}));
        assert_eq!(row("f.desktop")["source"], "uq");
    }

    #[test]
    fn compat_rejected_gives_section_row() {
        let mut st = State::default();
        st.compat.apps = vec![AppFix::new("a.desktop")];
        let v = explain_compat_extra(&st, None, Some("bad"), &FileReport::default(), PATH);
        for k in [
            "compat",
            "electron_wayland",
            "chromium_wayland",
            "apps.a.desktop",
        ] {
            assert_eq!(v[k]["source"], "error", "{k}");
            assert_eq!(v[k]["detail"]["message"], "bad", "{k}");
        }
    }

    #[test]
    fn sharp_x11_off_is_default() {
        let row = |source: &str| {
            json!({"sharp_x11": {"source": source, "live": null,
            "stored": null, "detail": {}}, "other": {"source": "uq"}})
        };
        let mut st = State::default();
        let mut v = row("uq");
        default_when_off(&mut v, &st);
        assert_eq!(v["sharp_x11"]["source"], "default");
        assert_eq!(v["other"]["source"], "uq");
        st.compat.sharp_x11 = true;
        let mut v = row("uq");
        default_when_off(&mut v, &st);
        assert_eq!(v["sharp_x11"]["source"], "uq");
        st.compat.sharp_x11 = false;
        for s in ["user_lua", "error"] {
            let mut v = row(s);
            default_when_off(&mut v, &st);
            assert_eq!(v["sharp_x11"]["source"], s);
        }
    }

    #[test]
    fn autostart_error_row() {
        let mut st = State::default();
        st.autostart.files = vec![crate::autostart::FileChoice {
            id: "nm-applet.desktop".into(),
            enabled: Some(false),
            delay: None,
        }];
        let v = explain_autostart(&st, &FileReport::default());
        assert_eq!(v["nm-applet.desktop"]["source"], "uq");
        assert!(v.get("autostart").is_none());
        let report = FileReport {
            autostart: Some("Permission denied".into()),
            ..FileReport::default()
        };
        let v = explain_autostart(&st, &report);
        assert_eq!(
            v["autostart"],
            json!({"source": "error", "live": null, "stored": null,
                "detail": {"message": "Permission denied"}})
        );
    }
}
