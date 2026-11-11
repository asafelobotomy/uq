//! Environment variables, Apps and scaling, and startup apps in uqd: the explain hooks, the
//! `compat` pre-check, the main screen's scale, the children's environment and the file sync.
//! Writes go through `SetSection`, which `commit_section` renders (`env`, `compat`) or just
//! saves (`autostart`); `persist` then calls [`Desktop::sync_files`].

use crate::files::{self, MarkerKind};
use crate::service::{Desktop, DesktopError};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uq_core::autostart::Added;
use uq_core::autostart::scan::{Plan, Resolved, read_capped, read_folders, resolve};
use uq_core::compat::launch::main_scale;
use uq_core::desktop_entry::{normalise_id, valid_desktop_id};
use uq_core::env::{environment_d_lines, session_env};
use uq_core::settings::{LiveOutput, parse_live};
use uq_core::startup_explain::{
    BrowserFile, OverrideState, default_when_off, explain_autostart, explain_compat_extra,
    explain_env,
};
use uq_core::state::State;

const OWN_FILE: &str = "You already have your own copy of this app's file, so uQ won't replace it.";
const NO_SOURCE: &str = "uQ can't find this app's file.";
const DEV_SESSION: &str = "Not available in a test session";

impl Desktop {
    /// Mark this uqd as running in a `uq-dev session` (`UQ_DEV_SESSION`, R20): `hl.env` doesn't
    /// export, and launcher-file overrides are refused and never written or deleted.
    pub fn with_dev_session(mut self, on: bool) -> Self {
        self.dev_session = on;
        self
    }

    /// The variables an autostarted app gets on top of uqd's own (F19): the theme's cursor
    /// lines, as `generated.lua` renders them, then the session variables.
    pub(crate) fn child_env(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(t) = self.themes.get(&self.state.theme, self.state.variant) {
            let t = t.with_appearance(&self.state.settings.appearance);
            let size = t.icons.cursor_size.to_string();
            out.push(("XCURSOR_THEME".to_string(), t.icons.cursor_theme.clone()));
            out.push(("XCURSOR_SIZE".to_string(), size.clone()));
            out.push(("HYPRCURSOR_SIZE".to_string(), size));
        }
        out.extend(session_env(
            &self.state.env,
            &self.state.compat,
            self.main_scale,
        ));
        out
    }

    /// The run plan and the page's rows for startup apps, with the variables an autostarted
    /// app gets on top of uqd's own (F19). Reads the folders, so the runner calls it once, under
    /// the desktop lock, and drops the lock before running anything. A run's working directory
    /// is its absolute, existing `Path=`, else the home folder.
    pub(crate) fn autostart_plan(&self) -> (Vec<Resolved>, Vec<(String, String)>) {
        let paths = &self.paths.startup;
        let found = read_folders(&paths.user_autostart(), &paths.system_autostart());
        let app_dirs = paths.app_dirs();
        let apps = |id: &str| -> Option<(PathBuf, String)> {
            if !valid_desktop_id(id) {
                return None;
            }
            let file = normalise_id(id);
            app_dirs.iter().find_map(|d| {
                let p = d.join(&file);
                read_capped(&p).map(|t| (p, t))
            })
        };
        let mut plan = resolve(
            &found,
            &self.state.autostart,
            &self.state.compat,
            &apps,
            &program_exists,
        );
        for e in &mut plan {
            if let Plan::Run { cwd, .. } = &mut e.plan {
                let dir = cwd.take().filter(|p| p.is_dir());
                *cwd = Some(dir.unwrap_or_else(|| paths.home.clone()));
            }
        }
        (plan, self.child_env())
    }

    /// The argv of each added command, by id, for `ListAutostart`.
    pub(crate) fn autostart_commands(&self) -> BTreeMap<String, Vec<String>> {
        self.state
            .autostart
            .added
            .iter()
            .filter_map(|a| match a {
                Added::Command { id, argv, .. } => Some((id.clone(), argv.clone())),
                Added::App { .. } => None,
            })
            .collect()
    }

    /// The main screen's scale for `next` (R13, F4): Hyprland's outputs from one
    /// `monitors all` read, with `next`'s stored display config (position, scale, on/off) laid
    /// over them, so a stored scale wins before it is live. `None` without Hyprland.
    pub(crate) async fn main_scale_for(&self, next: &State) -> Option<f64> {
        let live = parse_live(&self.comp.monitors().await.ok()?);
        let stored = &next.settings.monitors.outputs;
        let outputs: Vec<LiveOutput> = live
            .into_iter()
            .map(|mut o| {
                if let Some(c) = stored.get(&o.name) {
                    if let Some(p) = &c.position {
                        (o.x, o.y) = (p.x, p.y);
                    }
                    if let Some(s) = c.scale {
                        o.scale = s;
                    }
                    if let Some(on) = c.enabled {
                        o.disabled = !on;
                    }
                }
                o
            })
            .collect();
        main_scale(&outputs)
    }

    /// Refuse turning a fix's "Also when started elsewhere" on when its override can't be
    /// written (F3): a file of the user's own is there, the app has no launcher file, or this
    /// is a dev session. Only fixes whose `elsewhere` turns on in this write are checked.
    pub(crate) fn check_compat_files(&self, next: &State) -> Result<(), DesktopError> {
        let paths = &self.paths.startup;
        let refuse = |m: &str| Err(DesktopError::InvalidSetting(m.to_string()));
        for fix in next.compat.apps.iter().filter(|f| f.elsewhere) {
            if self
                .state
                .compat
                .fix_for(&fix.id)
                .is_some_and(|o| o.elsewhere)
            {
                continue;
            }
            if self.dev_session {
                return refuse(DEV_SESSION);
            }
            let Some(target) = files::override_target(paths, &fix.id) else {
                return refuse(NO_SOURCE);
            };
            if files::present(&target) && !files::is_ours(&target, MarkerKind::Override) {
                return refuse(OWN_FILE);
            }
            if files::override_source(paths, &fix.id).is_none() {
                return refuse(NO_SOURCE);
            }
        }
        Ok(())
    }

    /// Bring the files outside uqd's state in line with the state just adopted, running each
    /// part whose inputs changed since `old`. Failures are recorded for Explain and logged;
    /// they never fail the write. (The main scale feeds no file: F18 keeps the scale lines out
    /// of `environment.d`, and an override's scale is the fix's own.)
    pub(crate) fn sync_files(&mut self, old: &State) {
        let (env, compat, autostart) = (
            old.env != self.state.env,
            old.compat != self.state.compat,
            old.autostart != self.state.autostart,
        );
        if env || compat {
            self.sync_env_file();
        }
        if compat {
            self.sync_flag_files();
            let gone: Vec<String> = old
                .compat
                .apps
                .iter()
                .filter(|f| self.state.compat.fix_for(&f.id).is_none())
                .map(|f| f.id.clone())
                .collect();
            self.sync_override_files(&gone);
        }
        if autostart {
            self.sync_autostart_copies();
        }
    }

    /// Every part of the sync, at startup. Overrides of uQ's whose fix went away while uqd
    /// wasn't running are removed too.
    pub(crate) fn sync_all_files(&mut self) {
        self.sync_env_file();
        self.sync_flag_files();
        let gone: Vec<String> = if self.dev_session {
            Vec::new()
        } else {
            files::marked_overrides(&self.paths.startup)
                .into_iter()
                .filter(|id| self.state.compat.fix_for(id).is_none())
                .collect()
        };
        self.sync_override_files(&gone);
        self.sync_autostart_copies();
    }

    /// The 60 s check (R18, F37): overrides follow their source files, and a browser
    /// installed since gets its flags file.
    pub fn refresh_overrides(&mut self) {
        self.sync_override_files(&[]);
        self.sync_flag_files();
    }

    fn sync_env_file(&mut self) {
        let lines = environment_d_lines(&self.state.env, &self.state.compat);
        self.files.env_file = files::write_environment_d(&self.paths.startup, &lines)
            .err()
            .map(|e| {
                tracing::warn!("writing the session variables file: {e}");
                format!("uQ couldn't write the session variables file: {e}")
            });
    }

    fn sync_flag_files(&mut self) {
        let app_dirs = self.paths.startup.app_dirs();
        let installed = |id: &str| app_dirs.iter().any(|d| d.join(id).is_file());
        let report = files::sync_flags(&self.paths.startup, &self.state.compat, &installed);
        for (name, f) in &report {
            if let BrowserFile::Error(m) = f {
                tracing::warn!("{name} flags file: {m}");
            }
        }
        self.files.browsers = report;
    }

    fn sync_override_files(&mut self, gone: &[String]) {
        let paths = &self.paths.startup;
        for (id, e) in
            files::remove_overrides(paths, gone.iter().map(String::as_str), self.dev_session)
        {
            tracing::warn!("removing the launcher file for {id}: {e}");
        }
        let report = files::sync_overrides(paths, &self.state.compat, self.dev_session);
        for (id, s) in &report {
            if let OverrideState::Error(m) = s {
                tracing::warn!("launcher file for {id}: {m}");
            }
        }
        self.files.overrides = report;
    }

    fn sync_autostart_copies(&mut self) {
        let paths = &self.paths.startup;
        let found = read_folders(&paths.user_autostart(), &paths.system_autostart());
        self.files.autostart = files::sync_hidden_copies(paths, &found, &self.state.autostart);
        if let Some(m) = &self.files.autostart {
            tracing::warn!("{m}");
        }
    }

    /// `Explain` for `env`, `compat` and `autostart`. For `compat`, `generic` holds the
    /// generic option rows (`sharp_x11`); the other rows come from the core functions.
    pub(crate) fn explain_startup(&self, domain: &str, generic: Value) -> Value {
        let text = self.read_user_lua();
        let path = self.paths.user_lua.to_string_lossy();
        let rejected = self
            .last_rejected
            .as_ref()
            .filter(|(d, _)| d == domain)
            .map(|(_, m)| m.as_str());
        let st = &self.state;
        match domain {
            "env" => explain_env(st, text.as_deref(), rejected, &self.files, &path),
            "compat" => {
                let mut rows = generic;
                default_when_off(&mut rows, st);
                let extra = explain_compat_extra(st, text.as_deref(), rejected, &self.files, &path);
                if let (Value::Object(rows), Value::Object(extra)) = (&mut rows, extra) {
                    rows.extend(extra);
                }
                rows
            }
            _ => explain_autostart(st, &self.files),
        }
    }
}

/// `TryExec`: an absolute path to a file, or a file of that name in an absolute entry of uqd's
/// own `PATH`.
fn program_exists(name: &str) -> bool {
    if name.contains('/') {
        return Path::new(name).is_absolute() && Path::new(name).is_file();
    }
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p)
            .filter(|d| d.is_absolute())
            .any(|d| d.join(name).is_file())
    })
}

#[cfg(test)]
mod tests;
