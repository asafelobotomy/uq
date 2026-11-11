//! The `PluginHost` singleton: what a plugin page's `ctx.command` and `ctx.dbus` reach (Phase 4
//! spec §2.3, Phase 3 spec §2.5). QML side: `qml/components/PluginContext.qml`.
//!
//! Every request returns an id at once; its result arrives later as `finished(id, json)`, always
//! queued (never during the call), so the page has registered its callback by then.
//!
//! - **Commands** run only as declared in the page's `[[command]]` tables, through
//!   `uq_core::integration::command`. A command with `confirm` first emits
//!   `confirmRequested(id, page, text)` and runs only after `answer(id, true)`; the text is the
//!   manifest's, with `%s` filled from the first argument. One run per command at a time.
//! - **D-Bus** reaches only the page's own name. With a `[dbus]` table, only its path, interface
//!   and listed methods and properties (no target is passed). With only Phase 3's `dbus_name`,
//!   the page names the object path and interface, and any method or property on that name is
//!   allowed. Types come from introspection (`uq_core::integration::dbus_json`), with a 2 s
//!   deadline (or the method's `[dbus] timeout_ms`, for a write that may ask for a password),
//!   at most 8 requests in flight per page. A page that waits for its name
//!   (`requires.bus_name`) never starts the service.
//!
//! - **Opening an app** (`ctx.openApp`): a plain `name.desktop` id, started by `uqctl launch`,
//!   the shell's own launch path (the app's fix, its own scope, counted), detached from Settings.
//!
//! This limits mistakes, not attacks: a page is trusted package code.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use uq_core::integration::command::{self, BusySet, CommandError, Runner};
use uq_core::integration::dbus_json::{self, Target};
use uq_core::integration::manifest::{Bus, CommandSpec, PageManifest};

use crate::plugins;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type PluginHost = super::PluginHostRust;

        /// Run `command` of page `page` with `args_json` (a JSON array of strings). Returns the
        /// request id.
        #[qinvokable]
        fn run_command(
            self: Pin<&mut PluginHost>,
            page: &QString,
            command: &QString,
            args_json: &QString,
        ) -> i32;

        /// The answer to `confirmRequested(request, …)`: run it, or report it cancelled.
        #[qinvokable]
        fn answer(self: Pin<&mut PluginHost>, request: i32, confirmed: bool);

        /// Call `method` on the page's D-Bus name with `args_json` (a JSON array). `target_json`
        /// is `{path, interface}` for a page with only `dbus_name`, else "" or "{}".
        #[qinvokable]
        fn dbus_call(
            self: Pin<&mut PluginHost>,
            page: &QString,
            method: &QString,
            args_json: &QString,
            target_json: &QString,
        ) -> i32;

        /// Read `property` (as `dbus_call` for the target).
        #[qinvokable]
        fn dbus_get(
            self: Pin<&mut PluginHost>,
            page: &QString,
            property: &QString,
            target_json: &QString,
        ) -> i32;

        /// Open the app `desktop_id` (`name.desktop`) as the launcher does. Returns the request
        /// id; `finished` gets `{ok: true}` or `{error}`.
        #[qinvokable]
        fn open_app(self: Pin<&mut PluginHost>, page: &QString, desktop_id: &QString) -> i32;

        /// A request ended: `{code, output}` or `{error, …}` for commands, `{value}` or
        /// `{error}` for D-Bus, `{error, cancelled: true}` when a confirmation was declined.
        #[qsignal]
        fn finished(self: Pin<&mut PluginHost>, request: i32, result_json: QString);

        /// A command needs the person's confirmation: show `text` (plain text) with Cancel and
        /// Confirm, then call `answer`.
        #[qsignal]
        fn confirm_requested(
            self: Pin<&mut PluginHost>,
            request: i32,
            page: QString,
            text: QString,
        );
    }

    impl cxx_qt::Threading for PluginHost {}
}

use qobject::PluginHost;

/// How long `uqctl launch` may take to hand the app to Hyprland.
const OPEN_APP_WAIT: Duration = Duration::from_secs(10);

/// The command that opens `desktop_id` for a page: `uqctl launch <id>`, the launcher's path.
pub fn open_app_argv(desktop_id: &str) -> Result<Vec<String>, String> {
    if !uq_core::integration::appaction::valid_desktop_id(desktop_id) {
        return Err(format!(
            "{} is not an app id (expected a name like org.example.App.desktop)",
            uq_core::desktop_entry::clean_display(desktop_id, 80)
        ));
    }
    Ok(vec!["uqctl".into(), "launch".into(), desktop_id.into()])
}

/// Run `argv` detached from Settings (own process group, no stdio) and wait for it, so it is
/// reaped: `{ok: true}` on exit 0, else `{error}`.
pub async fn launch(argv: &[String], within: Duration) -> Value {
    let child = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return error(format!("the app couldn't be opened: {e}")),
    };
    match tokio::time::timeout(within, child.wait()).await {
        Ok(Ok(status)) if status.success() => json!({"ok": true}),
        Ok(Ok(_)) => error("the app couldn't be opened (is it installed?)"),
        Ok(Err(e)) => error(format!("the app couldn't be opened: {e}")),
        // Still going: leave it running and let tokio reap it later.
        Err(_) => json!({"ok": true}),
    }
}

/// At most this many D-Bus requests in flight per page (Phase 4 spec §2.2).
const MAX_IN_FLIGHT: usize = 8;

/// The runtime the host's work runs on; set by `main` before the window exists.
static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn set_runtime(handle: tokio::runtime::Handle) {
    let _ = RUNTIME.set(handle);
}

fn busy() -> &'static BusySet {
    static BUSY: OnceLock<BusySet> = OnceLock::new();
    BUSY.get_or_init(BusySet::new)
}

fn runner() -> &'static Runner {
    static RUNNER: OnceLock<Runner> = OnceLock::new();
    RUNNER.get_or_init(Runner::system)
}

fn in_flight() -> &'static Mutex<HashMap<String, usize>> {
    static IN_FLIGHT: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(Mutex::default)
}

/// A slot of a page's D-Bus budget, freed on drop.
struct Slot(String);

impl Slot {
    fn take(page: &str) -> Option<Slot> {
        let mut map = in_flight().lock().unwrap_or_else(|e| e.into_inner());
        let n = map.entry(page.to_string()).or_insert(0);
        if *n >= MAX_IN_FLIGHT {
            return None;
        }
        *n += 1;
        Some(Slot(page.to_string()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut map = in_flight().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = map.get_mut(&self.0) {
            *n = n.saturating_sub(1);
        }
    }
}

struct Pending {
    page: String,
    spec: CommandSpec,
    args: Vec<String>,
}

#[derive(Default)]
pub struct PluginHostRust {
    next: i32,
    pending: HashMap<i32, Pending>,
}

fn error(msg: impl std::fmt::Display) -> Value {
    json!({"error": msg.to_string()})
}

/// A declared command of `page` and the caller's checked arguments.
pub fn prepare(
    manifest: Option<PageManifest>,
    command: &str,
    args_json: &str,
) -> Result<(CommandSpec, Vec<String>), String> {
    let m = manifest.ok_or("this page is not installed any more")?;
    let spec = command::find(&m.commands, command)
        .map_err(|e| e.to_string())?
        .clone();
    let args: Vec<String> = if args_json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(args_json).map_err(|_| "arguments must be a list of text values")?
    };
    runner().argv(&spec, &args).map_err(|e| e.to_string())?;
    Ok((spec, args))
}

/// Where a D-Bus request of a page goes and how long it may take.
#[derive(Debug)]
pub struct Route {
    pub target: Target,
    pub bus: Bus,
    /// Whether the service may be started by the request.
    pub auto_start: bool,
    /// 2 s, or the method's `[dbus] timeout_ms`.
    pub deadline: Duration,
}

/// Where a D-Bus request of `page` may go: the target, the bus, whether the service may be
/// started, its deadline, and whether `member` is allowed (`property` picks the list).
pub fn dbus_target(
    manifest: Option<PageManifest>,
    member: &str,
    property: bool,
    target_json: &str,
) -> Result<Route, String> {
    let m = manifest.ok_or("this page is not installed any more")?;
    let name = m
        .dbus_name
        .clone()
        .ok_or("this page has no service to talk to (no dbus_name in page.toml)")?;
    let given: Value = if target_json.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(target_json).map_err(|_| "the target must be {path, interface}")?
    };
    let auto_start = m.requires.bus_name.is_none();
    let field = |k: &str| given.get(k).and_then(Value::as_str).map(str::to_string);
    match &m.dbus {
        Some(d) => {
            let other = |k: &str, want: &str| field(k).is_some_and(|v| v != want);
            if other("path", &d.path) || other("interface", &d.interface) {
                return Err(
                    "this page may only use the path and interface in its page.toml".into(),
                );
            }
            let listed = if property { &d.properties } else { &d.methods };
            if !listed.iter().any(|x| x == member) {
                return Err(format!(
                    "{member} is not listed in the page's [dbus] {}",
                    if property { "properties" } else { "methods" }
                ));
            }
            Ok(Route {
                target: Target {
                    name,
                    path: d.path.clone(),
                    interface: d.interface.clone(),
                },
                bus: d.bus,
                auto_start,
                deadline: if property {
                    dbus_json::CALL_DEADLINE
                } else {
                    d.deadline(member)
                },
            })
        }
        None => {
            let (Some(path), Some(interface)) = (field("path"), field("interface")) else {
                return Err("give the object path and interface: {path, interface}".into());
            };
            Ok(Route {
                target: Target {
                    name,
                    path,
                    interface,
                },
                bus: Bus::Session,
                auto_start,
                deadline: dbus_json::CALL_DEADLINE,
            })
        }
    }
}

async fn connection(bus: Bus) -> Result<zbus::Connection, String> {
    static SESSION: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    static SYSTEM: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    let cell = match bus {
        Bus::Session => &SESSION,
        Bus::System => &SYSTEM,
    };
    cell.get_or_try_init(|| async {
        match bus {
            Bus::Session => match crate::session::bus() {
                Some((_, c)) => Ok(c),
                None => zbus::Connection::session().await,
            },
            Bus::System => zbus::Connection::system().await,
        }
    })
    .await
    .cloned()
    .map_err(|e| format!("can't reach the message bus: {e}"))
}

impl PluginHost {
    fn next_id(self: Pin<&mut Self>) -> i32 {
        let mut rust = self.rust_mut();
        rust.next = rust.next.wrapping_add(1).max(1);
        rust.next
    }

    /// Emit `finished(request, value)` from the Qt event loop, later.
    fn deliver(&self, request: i32, value: Value) {
        let json = QString::from(&value.to_string());
        let _ = self
            .qt_thread()
            .queue(move |host| host.finished(request, json));
    }

    fn run_command(
        mut self: Pin<&mut Self>,
        page: &QString,
        command: &QString,
        args_json: &QString,
    ) -> i32 {
        let id = self.as_mut().next_id();
        let page = page.to_string();
        match prepare(
            plugins::manifest(&page),
            &command.to_string(),
            &args_json.to_string(),
        ) {
            Err(e) => self.deliver(id, error(e)),
            Ok((spec, args)) => match spec.confirm_text(&args) {
                Some(text) => {
                    let (p, t) = (QString::from(&page), QString::from(&text));
                    self.as_mut()
                        .rust_mut()
                        .pending
                        .insert(id, Pending { page, spec, args });
                    let _ = self
                        .qt_thread()
                        .queue(move |host| host.confirm_requested(id, p, t));
                }
                None => self.start(id, page, spec, args),
            },
        }
        id
    }

    fn answer(mut self: Pin<&mut Self>, request: i32, confirmed: bool) {
        let Some(p) = self.as_mut().rust_mut().pending.remove(&request) else {
            return;
        };
        if confirmed {
            self.start(request, p.page, p.spec, p.args);
        } else {
            self.deliver(request, json!({"error": "Cancelled", "cancelled": true}));
        }
    }

    fn start(&self, id: i32, page: String, spec: CommandSpec, args: Vec<String>) {
        let token = match busy().try_acquire(&page, &spec.id) {
            Ok(t) => t,
            Err(e) => return self.deliver(id, command::error_json(&e)),
        };
        let Some(rt) = RUNTIME.get() else {
            return self.deliver(id, error("commands can't run right now"));
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let result = runner().run(&spec, &args).await;
            drop(token);
            let value = match &result {
                Ok(r) => {
                    if r.code != 0 {
                        eprintln!(
                            "uq-settings: page {page}: command {} exited with {}: {}",
                            spec.id,
                            r.code,
                            r.stderr_head.trim()
                        );
                    }
                    r.to_json()
                }
                Err(e) => {
                    if !matches!(e, CommandError::Busy) {
                        eprintln!("uq-settings: page {page}: command {}: {e}", spec.id);
                    }
                    command::error_json(e)
                }
            };
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
    }

    fn open_app(mut self: Pin<&mut Self>, page: &QString, desktop_id: &QString) -> i32 {
        let id = self.as_mut().next_id();
        let page = page.to_string();
        if plugins::manifest(&page).is_none() {
            self.deliver(id, error("this page is not installed any more"));
            return id;
        }
        let argv = match open_app_argv(&desktop_id.to_string()) {
            Ok(a) => a,
            Err(e) => {
                self.deliver(id, error(e));
                return id;
            }
        };
        let Some(rt) = RUNTIME.get() else {
            self.deliver(id, error("apps can't be opened right now"));
            return id;
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let value = launch(&argv, OPEN_APP_WAIT).await;
            if let Some(e) = value.get("error") {
                eprintln!("uq-settings: page {page}: opening {}: {e}", argv[2]);
            }
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
        id
    }

    fn dbus_call(
        self: Pin<&mut Self>,
        page: &QString,
        method: &QString,
        args_json: &QString,
        target_json: &QString,
    ) -> i32 {
        let args: Result<Value, String> = match args_json.to_string().trim() {
            "" => Ok(json!([])),
            text => serde_json::from_str(text).map_err(|_| "arguments must be a JSON list".into()),
        };
        self.dbus(page, method, target_json, false, args)
    }

    fn dbus_get(
        self: Pin<&mut Self>,
        page: &QString,
        property: &QString,
        target_json: &QString,
    ) -> i32 {
        self.dbus(page, property, target_json, true, Ok(Value::Null))
    }

    fn dbus(
        mut self: Pin<&mut Self>,
        page: &QString,
        member: &QString,
        target_json: &QString,
        property: bool,
        args: Result<Value, String>,
    ) -> i32 {
        let id = self.as_mut().next_id();
        let (page, member) = (page.to_string(), member.to_string());
        let target = dbus_target(
            plugins::manifest(&page),
            &member,
            property,
            &target_json.to_string(),
        );
        let (route, args) = match (target, args) {
            (Ok(route), Ok(args)) => (route, args),
            (Err(e), _) | (_, Err(e)) => {
                self.deliver(id, error(e));
                return id;
            }
        };
        let Some(slot) = Slot::take(&page) else {
            self.deliver(id, error("busy: too many requests at once"));
            return id;
        };
        let Some(rt) = RUNTIME.get() else {
            self.deliver(id, error("the service can't be reached right now"));
            return id;
        };
        let qt = self.qt_thread();
        rt.spawn(async move {
            let Route {
                target,
                bus,
                auto_start,
                deadline,
            } = route;
            let result = match connection(bus).await {
                Err(e) => Err(e),
                Ok(conn) => if property {
                    dbus_json::get_property(&conn, &target, &member, auto_start).await
                } else {
                    dbus_json::call_within(&conn, &target, &member, &args, auto_start, deadline)
                        .await
                }
                .map_err(|e| e.to_string()),
            };
            drop(slot);
            let value = match result {
                Ok(v) => json!({"value": v}),
                Err(e) => error(e),
            };
            let json = QString::from(&value.to_string());
            let _ = qt.queue(move |host| host.finished(id, json));
        });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uq_core::integration::manifest::parse_page;

    fn page(extra: &str) -> Option<PageManifest> {
        Some(
            parse_page(&format!(
                "id = \"p\"\nname = \"P\"\nqml = \"Page.qml\"\n{extra}"
            ))
            .unwrap(),
        )
    }

    const CMD: &str = "[[command]]\nid = \"remove\"\nargv = [\"/usr/bin/crumbs\", \"rm\"]\n\
                       output = \"json\"\nargs = [{ name = \"app\", pattern = \"^[a-z.]+$\" }]\n\
                       confirm = \"Remove %s?\"\n";

    #[test]
    fn prepare_checks_the_declared_command_and_args() {
        let (spec, args) = prepare(page(CMD), "remove", "[\"org.maps\"]").unwrap();
        assert_eq!(
            spec.confirm_text(&args).as_deref(),
            Some("Remove org.maps?")
        );
        for (cmd, args) in [
            ("nope", "[\"a\"]"),
            ("remove", "[]"),
            ("remove", "[\"-rf\"]"),
            ("remove", "[\"A B\"]"),
            ("remove", "[\"a\", \"b\"]"),
            ("remove", "[1]"),
            ("remove", "not json"),
        ] {
            assert!(prepare(page(CMD), cmd, args).is_err(), "{cmd} {args}");
        }
        assert!(prepare(None, "remove", "[\"a\"]").is_err(), "page gone");
    }

    #[test]
    fn dbus_targets_follow_the_manifest() {
        // Phase 3 shape: dbus_name only, the page names path and interface.
        let p3 = || page("dbus_name = \"org.x.Y\"\n");
        let r = dbus_target(
            p3(),
            "Get",
            false,
            r#"{"path": "/org/x/Y", "interface": "org.x.Y"}"#,
        )
        .unwrap();
        let t = &r.target;
        assert_eq!((t.name.as_str(), t.path.as_str()), ("org.x.Y", "/org/x/Y"));
        assert_eq!(r.bus, Bus::Session);
        assert!(r.auto_start);
        assert_eq!(r.deadline, dbus_json::CALL_DEADLINE);
        assert!(
            dbus_target(p3(), "Get", false, "").is_err(),
            "needs a target"
        );
        assert!(
            dbus_target(page(""), "Get", false, "{}").is_err(),
            "no dbus_name"
        );

        // With a [dbus] table: only its members; a different target is refused.
        let table = || {
            page(
                "[requires]\nbus_name = \"org.x.Y\"\n[dbus]\nname = \"org.x.Y\"\npath = \"/o\"\n\
                 interface = \"org.x.I\"\nmethods = [\"Pause\", \"State\"]\n\
                 properties = [\"Count\"]\nbus = \"system\"\ntimeout_ms = { Pause = 120000 }\n",
            )
        };
        let r = dbus_target(table(), "Pause", false, "").unwrap();
        let t = &r.target;
        assert_eq!((t.path.as_str(), t.interface.as_str()), ("/o", "org.x.I"));
        assert_eq!(r.bus, Bus::System);
        assert!(!r.auto_start, "a page waiting for its name never starts it");
        assert_eq!(r.deadline, Duration::from_secs(120), "its timeout_ms");
        assert_eq!(
            dbus_target(table(), "State", false, "").unwrap().deadline,
            dbus_json::CALL_DEADLINE
        );
        assert!(dbus_target(table(), "Count", true, "{}").is_ok());
        assert!(
            dbus_target(table(), "Count", false, "").is_err(),
            "not a method"
        );
        assert!(
            dbus_target(table(), "Pause", true, "").is_err(),
            "not a property"
        );
        assert!(dbus_target(table(), "Resume", false, "").is_err());
        assert!(
            dbus_target(table(), "Pause", false, r#"{"path": "/other"}"#).is_err(),
            "other path"
        );
        assert!(dbus_target(table(), "Pause", false, r#"{"path": "/o"}"#).is_ok());
    }

    #[test]
    fn open_app_takes_plain_desktop_ids_only() {
        assert_eq!(
            open_app_argv("org.noti_fi.App.desktop").unwrap(),
            ["uqctl", "launch", "org.noti_fi.App.desktop"]
        );
        for bad in [
            "",
            "firefox",
            "../x.desktop",
            "/usr/share/applications/x.desktop",
            "a/b.desktop",
            "-x.desktop",
            ".x.desktop",
            "x y.desktop",
            "x.desktop\n",
        ] {
            assert!(open_app_argv(bad).is_err(), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn launch_reports_the_exit() {
        let wait = Duration::from_secs(5);
        assert_eq!(launch(&["true".into()], wait).await, json!({"ok": true}));
        assert!(launch(&["false".into()], wait).await["error"].is_string());
        assert!(launch(&["/nonexistent/uq-launch".into()], wait).await["error"].is_string());
        // Slower than the wait: left running, reported as opened.
        let slow = ["sleep".to_string(), "2".to_string()];
        assert_eq!(
            launch(&slow, Duration::from_millis(100)).await,
            json!({"ok": true})
        );
    }

    #[test]
    fn in_flight_budget_per_page() {
        let slots: Vec<Slot> = (0..MAX_IN_FLIGHT)
            .map(|_| Slot::take("budget").unwrap())
            .collect();
        assert!(Slot::take("budget").is_none());
        assert!(Slot::take("other-page").is_some());
        drop(slots);
        assert!(Slot::take("budget").is_some());
    }
}
