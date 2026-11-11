//! App actions (Phase 4 spec §2.4.2): `/usr/share/uq/launcher/actions/<id>.appaction` files,
//! shown in the right-click menu of matching app rows in Spotlight and the app menu.
//!
//! The bridge reads the directory at start and on every change (inotify, 1 s debounce) and
//! sends the actions the shell may show in an `AppActions` event. Running one goes through
//! `Launch {app_action, desktop_id}`: the shell names the action and the row, and the bridge
//! builds the argv from the file (`%d` is the validated desktop id, one whole argument). The
//! shell never builds a command line.

use super::desktop_entry::{Dirs, normalise_id};
use super::launch::{self, Plan, scope_command, terminal_prefix};
use super::protocol::Outbound;
use super::uqd::Link;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uq_core::integration::appaction::{AppAction, scan_app_actions, valid_desktop_id};
use uq_core::integration::watch::{DEBOUNCE, watch_dirs};
use uqd::hypr::HyprSocket;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The actions that show: valid files whose program exists (§5), by file name.
pub fn usable(found: Vec<(PathBuf, Result<AppAction, String>)>) -> (Vec<AppAction>, Vec<String>) {
    let mut ok = Vec::new();
    let mut problems = Vec::new();
    for (path, r) in found {
        match r {
            Ok(a) if a.program_exists() => ok.push(a),
            Ok(a) => problems.push(format!(
                "{}: hidden, {} is missing",
                path.display(),
                a.program()
            )),
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    (ok, problems)
}

/// The `AppActions` event data: `[{id, name, icon, match, terminal}]`.
pub fn list_json(actions: &[AppAction]) -> Value {
    Value::Array(
        actions
            .iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "name": a.name,
                    "icon": a.icon,
                    "match": a.match_glob,
                    "terminal": a.terminal,
                })
            })
            .collect(),
    )
}

/// The plan for `Launch {app_action, desktop_id}`: the action must be one of `actions`, the
/// desktop id valid (`.desktop` added if missing), installed in `dirs` and matched by the
/// action. `Terminal=true` runs it in the chosen terminal; either way it runs in its own scope.
pub fn plan(
    arg: &Value,
    actions: &[AppAction],
    dirs: &Dirs,
    rand: u32,
    terminal: Option<&str>,
) -> Result<Plan, String> {
    let obj = arg.as_object().ok_or("Launch takes an object argument")?;
    let action_id = obj
        .get("app_action")
        .and_then(Value::as_str)
        .ok_or("app_action must be a string")?;
    let raw_id = obj
        .get("desktop_id")
        .and_then(Value::as_str)
        .ok_or("an app action needs a desktop_id")?;
    if obj.keys().any(|k| k != "app_action" && k != "desktop_id") {
        return Err("an app action takes only app_action and desktop_id".into());
    }
    let desktop_id = normalise_id(raw_id);
    if !valid_desktop_id(&desktop_id) {
        return Err(format!("invalid desktop id {raw_id:?}"));
    }
    let action = actions
        .iter()
        .find(|a| a.id == action_id)
        .ok_or_else(|| format!("no app action {action_id:?}"))?;
    if dirs.find(&desktop_id).is_none() {
        return Err(format!("no app called {desktop_id}"));
    }
    if !action.program_exists() {
        return Err(format!("{} is missing", action.program()));
    }
    let mut argv = action.argv(&desktop_id)?;
    if action.terminal {
        argv.splice(0..0, terminal_prefix(dirs, terminal));
    }
    Ok(Plan {
        command: scope_command(&format!("action-{}", action.id), &argv, rand),
        record: None,
    })
}

pub(crate) struct AppActions {
    tx: mpsc::Sender<Outbound>,
    stop: CancellationToken,
    dir: PathBuf,
    actions: Mutex<Vec<AppAction>>,
    sent: Mutex<Option<Value>>,
    logged: Mutex<HashSet<String>>,
}

impl AppActions {
    pub(crate) fn new(
        tx: mpsc::Sender<Outbound>,
        stop: CancellationToken,
        dir: PathBuf,
    ) -> Arc<AppActions> {
        Arc::new(AppActions {
            tx,
            stop,
            dir,
            actions: Mutex::default(),
            sent: Mutex::default(),
            logged: Mutex::default(),
        })
    }

    /// Scan, send the first `AppActions` event, then follow the directory until the bridge
    /// stops.
    pub(crate) async fn run(self: Arc<Self>) {
        self.rescan().await;
        let mut changes = match watch_dirs(vec![self.dir.clone()], DEBOUNCE) {
            Ok(rx) => Some(rx),
            Err(e) => {
                eprintln!("uqctl bridge: can't watch {}: {e}", self.dir.display());
                None
            }
        };
        loop {
            let next = async {
                match changes.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = self.stop.cancelled() => return,
                got = next => match got {
                    Some(()) => self.rescan().await,
                    None => changes = None,
                },
            }
        }
    }

    async fn rescan(&self) {
        let (ok, problems) = usable(scan_app_actions(&self.dir));
        for p in problems {
            if lock(&self.logged).insert(p.clone()) {
                eprintln!("uqctl bridge: app action skipped: {p}");
            }
        }
        let data = list_json(&ok);
        *lock(&self.actions) = ok;
        let send = {
            let mut sent = lock(&self.sent);
            if sent.as_ref() == Some(&data) {
                false
            } else {
                *sent = Some(data.clone());
                true
            }
        };
        if send {
            let ev = Outbound::Event {
                event: "AppActions".into(),
                data,
            };
            let _ = self.tx.send(ev).await;
        }
    }

    /// `Launch [{app_action, desktop_id}]`.
    pub(crate) async fn launch(&self, link: &Link, args: &[Value]) -> Result<Value, String> {
        let [arg] = args else {
            return Err("Launch takes one object argument".into());
        };
        let terminal = link
            .settings_rx()
            .borrow()
            .as_ref()
            .and_then(|s| s.shell.terminal.clone());
        let plan = {
            let actions = lock(&self.actions);
            plan(
                arg,
                &actions,
                &Dirs::from_env(),
                launch::random_suffix(),
                terminal.as_deref(),
            )?
        };
        launch::dispatch(&HyprSocket::from_env(), &plan).await?;
        Ok(Value::Null)
    }

    #[cfg(test)]
    fn dir(&self) -> &std::path::Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uq_core::integration::appaction::parse_app_action;

    fn dirs() -> (tempfile::TempDir, Dirs) {
        let t = tempfile::tempdir().unwrap();
        let apps = t.path().join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("org.gnome.Maps.desktop"),
            "[Desktop Entry]\nName=Maps\nExec=maps\n",
        )
        .unwrap();
        std::fs::write(
            apps.join("term.desktop"),
            "[Desktop Entry]\nName=Term\nExec=/usr/bin/term-x\nX-TerminalArgExec=--\n",
        )
        .unwrap();
        let d = Dirs::new(None, vec![apps]);
        (t, d)
    }

    fn action(id: &str, text: &str) -> AppAction {
        parse_app_action(id, text).unwrap()
    }

    #[test]
    fn plans_detached_and_terminal_actions() {
        let (_t, d) = dirs();
        let acts = vec![
            action(
                "check",
                "[App Action]\nName=Check\nExec=/usr/bin/true check --id %d\n",
            ),
            action(
                "rm",
                "[App Action]\nName=Uninstall…\nMatch=org.*\nExec=/usr/bin/true rm %d\nTerminal=true\n",
            ),
        ];
        let p = plan(
            &json!({"app_action": "check", "desktop_id": "org.gnome.Maps"}),
            &acts,
            &d,
            1,
            None,
        )
        .unwrap();
        assert_eq!(
            p.command,
            "systemd-run --user --scope --collect --slice=app.slice \
             --unit=app-uq-action-check-00000001 -- \
             '/usr/bin/true' 'check' '--id' 'org.gnome.Maps.desktop'",
        );
        assert_eq!(p.record, None);
        let p = plan(
            &json!({"app_action": "rm", "desktop_id": "org.gnome.Maps.desktop"}),
            &acts,
            &d,
            1,
            Some("term.desktop"),
        )
        .unwrap();
        assert!(
            p.command.ends_with(
                "-- '/usr/bin/term-x' '--' '/usr/bin/true' 'rm' 'org.gnome.Maps.desktop'"
            ),
            "{}",
            p.command
        );
    }

    #[test]
    fn refuses_bad_rows_and_unknown_actions() {
        let (_t, d) = dirs();
        let acts = vec![action(
            "rm",
            "[App Action]\nName=Rm\nMatch=org.*\nExec=/usr/bin/true %d\n",
        )];
        let try_plan = |v: Value| plan(&v, &acts, &d, 1, None).unwrap_err();
        assert!(
            try_plan(json!({"app_action": "nope", "desktop_id": "org.gnome.Maps"}))
                .contains("no app action")
        );
        assert!(
            try_plan(json!({"app_action": "rm", "desktop_id": "term"})).contains("doesn't apply")
        );
        assert!(
            try_plan(json!({"app_action": "rm", "desktop_id": "org.missing"}))
                .contains("no app called")
        );
        for bad in ["../x", "a b", "-rf", "x;y", "", "a/b.desktop", "$(id)"] {
            let e = try_plan(json!({"app_action": "rm", "desktop_id": bad}));
            assert!(e.contains("invalid desktop id"), "{bad}: {e}");
        }
        assert!(try_plan(json!({"app_action": "rm"})).contains("desktop_id"));
        assert!(
            try_plan(json!({"app_action": "rm", "desktop_id": "org.gnome.Maps", "argv": ["x"]}))
                .contains("only")
        );
        assert!(
            try_plan(json!({"app_action": 3, "desktop_id": "org.gnome.Maps"})).contains("string")
        );
    }

    #[test]
    fn missing_programs_are_hidden() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(
            t.path().join("ok.appaction"),
            "[App Action]\nName=Ok\nExec=/usr/bin/true %d\n",
        )
        .unwrap();
        std::fs::write(
            t.path().join("gone.appaction"),
            "[App Action]\nName=Gone\nExec=/usr/bin/uq-no-such-program %d\n",
        )
        .unwrap();
        std::fs::write(t.path().join("bad.appaction"), "[App Action]\nName=Bad\n").unwrap();
        let (ok, problems) = usable(scan_app_actions(t.path()));
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].id, "ok");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!(
            list_json(&ok),
            json!([{"id":"ok","name":"Ok","icon":null,"match":"*","terminal":false}])
        );
        let (tx, _rx) = mpsc::channel(1);
        let a = AppActions::new(tx, CancellationToken::new(), t.path().into());
        assert_eq!(a.dir(), t.path());
    }
}
