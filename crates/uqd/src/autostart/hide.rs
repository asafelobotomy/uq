//! "Start minimised": once a startup app's window opens, hide it. An app that has an icon in
//! the system tray goes to the tray (its window is closed, which is what a tray app does
//! with its window); one without a tray icon is minimised, which is the move the bar's tabs
//! make (`special:minimized`, tagged with the workspace it came from, so it can be restored).
//!
//! The watch is bounded: it ends [`WINDOW_WAIT`] after the app started, or [`GRACE`] after
//! the app's first window opened, whichever is later, so no timer outlives a login. Windows are
//! matched to the app by its systemd scope (`/proc/<pid>/cgroup`), so a window the user
//! opened later of the same program is never touched, and everything read from Hyprland or the
//! bus is untrusted text that is checked before it is used.

mod place;

use super::spawn::cgroup_has_unit;
use crate::compositor::CompositorError;
use crate::hypr::HyprSocket;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

/// How long after an app starts its first window is waited for.
pub const WINDOW_WAIT: Duration = Duration::from_secs(60);
/// How long after the first window other windows of the app (a splash screen, then the main
/// window) are still hidden.
pub const GRACE: Duration = Duration::from_secs(15);
/// How long a window waits for its app's tray icon before it is minimised instead.
pub const TRAY_WAIT: Duration = Duration::from_secs(4);
const POLL: Duration = Duration::from_millis(300);

/// What happened to a startup app that was to start minimised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidden {
    /// Its window was closed to the tray icon it has.
    Tray,
    /// Its window was minimised (the app has no tray icon).
    Minimised,
    /// No window opened in time.
    NoWindow,
}

impl Hidden {
    pub fn key(self) -> &'static str {
        match self {
            Hidden::Tray => "tray",
            Hidden::Minimised => "minimised",
            Hidden::NoWindow => "no_window",
        }
    }
}

/// Hides the windows of one started scope.
#[async_trait]
pub trait Hider: Send + Sync {
    /// Wait for the windows of the scope `unit` and hide them. `None` when the compositor
    /// couldn't be reached.
    async fn hide(&self, unit: &str) -> Option<Hidden>;

    /// Wait for the windows of the scope `unit` and move them to `workspace` without following
    /// them (restore last session). Whether a window was moved.
    async fn place(&self, _unit: &str, _workspace: i64) -> bool {
        false
    }
}

/// One window Hyprland lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub address: String,
    pub pid: u32,
    pub workspace: i64,
    pub mapped: bool,
}

/// `0x` and hex digits, as Hyprland writes a window's address.
fn valid_address(a: &str) -> bool {
    a.len() <= 18
        && a.strip_prefix("0x")
            .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The windows in a `j/clients` reply; an entry that doesn't look right is skipped.
pub fn parse_clients(reply: &str) -> Vec<Client> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(reply) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|c| {
            let address = c["address"].as_str().filter(|a| valid_address(a))?;
            Some(Client {
                address: address.to_string(),
                pid: u32::try_from(c["pid"].as_u64()?).ok()?,
                workspace: c["workspace"]["id"].as_i64()?,
                mapped: c["mapped"].as_bool().unwrap_or(true),
            })
        })
        .collect()
}

fn lua_string(s: &str) -> String {
    format!("\"{s}\"")
}

/// The Lua that closes a window (an address that passed `valid_address`).
pub fn close_lua(address: &str) -> String {
    format!(
        "return hl.dispatch(hl.dsp.window.close({{ window = {} }}))",
        lua_string(&format!("address:{address}"))
    )
}

/// The two steps of a minimise, as the shell's tabs make it: remember the workspace, then move
/// the window to `special:minimized`.
pub fn minimise_lua(address: &str, from_ws: i64) -> [String; 2] {
    let win = lua_string(&format!("address:{address}"));
    [
        format!(
            "return hl.dispatch(hl.dsp.window.tag({{ tag = \"+uq_ws_{from_ws}\", window = {win} }}))"
        ),
        format!(
            "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:minimized\", follow = false, window = {win} }}))"
        ),
    ]
}

/// The tray items' bus names out of `RegisteredStatusNotifierItems`: an item is
/// `<bus name>/<object path>` (some apps register only a path or only a name).
pub fn item_bus_name(item: &str) -> Option<&str> {
    let name = item.split('/').next().filter(|n| !n.is_empty())?;
    (name.len() <= 255 && !name.contains(char::is_whitespace)).then_some(name)
}

/// Which processes own an icon in the system tray right now.
#[async_trait]
pub trait Tray: Send + Sync {
    async fn pids(&self) -> Vec<u32>;
}

/// The real tray: the pids behind the session bus's registered status notifier items.
#[derive(Default)]
pub struct BusTray {
    bus: tokio::sync::OnceCell<Option<zbus::Connection>>,
}

impl BusTray {
    async fn bus(&self) -> Option<&zbus::Connection> {
        self.bus
            .get_or_init(|| async { zbus::Connection::session().await.ok() })
            .await
            .as_ref()
    }
}

#[async_trait]
impl Tray for BusTray {
    async fn pids(&self) -> Vec<u32> {
        let Some(bus) = self.bus().await else {
            return Vec::new();
        };
        let items = bus
            .call_method(
                Some("org.kde.StatusNotifierWatcher"),
                "/StatusNotifierWatcher",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(
                    "org.kde.StatusNotifierWatcher",
                    "RegisteredStatusNotifierItems",
                ),
            )
            .await
            .ok()
            .and_then(|m| m.body().deserialize::<zbus::OwnedVariant>().ok())
            .and_then(|v| Vec::<String>::try_from(v).ok())
            .unwrap_or_default();
        let mut pids = Vec::new();
        for item in items.iter().take(64) {
            let Some(name) = item_bus_name(item) else {
                continue;
            };
            let pid = bus
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "GetConnectionUnixProcessID",
                    &(name,),
                )
                .await
                .ok()
                .and_then(|m| m.body().deserialize::<u32>().ok());
            pids.extend(pid);
        }
        pids
    }
}

/// The real hider: Hyprland's request socket, the tray, and `/proc` for the scopes.
pub struct SystemHider {
    hypr: HyprSocket,
    tray: Box<dyn Tray>,
    proc: PathBuf,
    tray_wait: Duration,
    grace: Duration,
}

impl SystemHider {
    pub fn new(hypr: HyprSocket) -> SystemHider {
        SystemHider::with(hypr, Box::new(BusTray::default()), PathBuf::from("/proc"))
    }

    /// Any tray and any `/proc` (tests give a folder of `<pid>/cgroup` files).
    pub fn with(hypr: HyprSocket, tray: Box<dyn Tray>, proc: PathBuf) -> SystemHider {
        SystemHider {
            hypr,
            tray,
            proc,
            tray_wait: TRAY_WAIT,
            grace: GRACE,
        }
    }

    /// How long a window waits for its app's tray icon, and for more windows after the first
    /// (tests shorten both).
    pub fn with_waits(mut self, tray: Duration, grace: Duration) -> SystemHider {
        self.tray_wait = tray;
        self.grace = grace;
        self
    }

    async fn in_unit(&self, pid: u32, unit: &str) -> bool {
        tokio::fs::read_to_string(self.proc.join(pid.to_string()).join("cgroup"))
            .await
            .is_ok_and(|t| cgroup_has_unit(&t, unit))
    }

    async fn has_tray(&self, unit: &str) -> bool {
        for pid in self.tray.pids().await {
            if self.in_unit(pid, unit).await {
                return true;
            }
        }
        false
    }

    /// The windows of `unit` that are open and not handled yet.
    async fn fresh(
        &self,
        unit: &str,
        seen: &BTreeSet<String>,
    ) -> Result<Vec<Client>, CompositorError> {
        let clients = parse_clients(&self.hypr.request("j/clients").await?);
        let mut out = Vec::new();
        for c in clients {
            if c.mapped && !seen.contains(&c.address) && self.in_unit(c.pid, unit).await {
                out.push(c);
            }
        }
        Ok(out)
    }

    async fn hide_one(&self, unit: &str, c: &Client) -> Result<Hidden, CompositorError> {
        let waited = tokio::time::Instant::now();
        let mut tray = false;
        loop {
            tray = tray || self.has_tray(unit).await;
            if tray || waited.elapsed() >= self.tray_wait {
                break;
            }
            tokio::time::sleep(POLL).await;
        }
        if tray {
            self.hypr.eval(&close_lua(&c.address)).await?;
            return Ok(Hidden::Tray);
        }
        for step in minimise_lua(&c.address, c.workspace) {
            self.hypr.eval(&step).await?;
        }
        Ok(Hidden::Minimised)
    }
}

#[async_trait]
impl Hider for SystemHider {
    async fn place(&self, unit: &str, workspace: i64) -> bool {
        self.place_windows(unit, workspace).await
    }

    async fn hide(&self, unit: &str) -> Option<Hidden> {
        let start = tokio::time::Instant::now();
        let mut seen = BTreeSet::new();
        let mut first: Option<tokio::time::Instant> = None;
        let mut result = Hidden::NoWindow;
        loop {
            let now = tokio::time::Instant::now();
            let over = match first {
                Some(f) => now.duration_since(f) >= self.grace,
                None => now.duration_since(start) >= WINDOW_WAIT,
            };
            if over {
                return Some(result);
            }
            let fresh = match self.fresh(unit, &seen).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("[START-003] start minimised: reading the windows: {e}");
                    return None;
                }
            };
            for c in fresh {
                seen.insert(c.address.clone());
                first.get_or_insert(tokio::time::Instant::now());
                match self.hide_one(unit, &c).await {
                    // A window sent to the tray stays the answer; a later one doesn't undo it.
                    Ok(h) if result != Hidden::Tray => result = h,
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("[START-003] start minimised: hiding a window: {e}");
                        return None;
                    }
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_parse_and_skip_odd_entries() {
        let reply = r#"[
          {"address":"0x55d0c0ffee","mapped":true,"pid":4242,"workspace":{"id":3,"name":"3"}},
          {"address":"0x1","mapped":false,"pid":7,"workspace":{"id":-98,"name":"special:minimized"}},
          {"address":"nope","pid":1,"workspace":{"id":1}},
          {"address":"0x2","pid":-1,"workspace":{"id":1}},
          {"address":"0x3","pid":5}
        ]"#;
        assert_eq!(
            parse_clients(reply),
            vec![
                Client {
                    address: "0x55d0c0ffee".into(),
                    pid: 4242,
                    workspace: 3,
                    mapped: true
                },
                Client {
                    address: "0x1".into(),
                    pid: 7,
                    workspace: -98,
                    mapped: false
                },
            ]
        );
        assert!(parse_clients("not json").is_empty());
        assert!(parse_clients("{}").is_empty());
    }

    #[test]
    fn lua_matches_the_shells_own_minimise_and_close() {
        let w = r#""address:0x55d0c0ffee""#;
        assert_eq!(
            minimise_lua("0x55d0c0ffee", 4),
            [
                format!(
                    "return hl.dispatch(hl.dsp.window.tag({{ tag = \"+uq_ws_4\", window = {w} }}))"
                ),
                format!(
                    "return hl.dispatch(hl.dsp.window.move({{ workspace = \"special:minimized\", follow = false, window = {w} }}))"
                ),
            ]
        );
        assert_eq!(
            close_lua("0x55d0c0ffee"),
            format!("return hl.dispatch(hl.dsp.window.close({{ window = {w} }}))")
        );
    }

    #[test]
    fn addresses_and_bus_names_are_checked() {
        assert!(valid_address("0xdeadbeef"));
        for bad in [
            "",
            "0x",
            "0xzz",
            "x1",
            "0x1\") os.exit()--",
            "0x11111111111111111",
        ] {
            assert!(!valid_address(bad), "{bad}");
        }
        assert_eq!(
            item_bus_name("org.kde.StatusNotifierItem-1234-1/StatusNotifierItem"),
            Some("org.kde.StatusNotifierItem-1234-1")
        );
        assert_eq!(item_bus_name(":1.45/org/ayatana"), Some(":1.45"));
        assert_eq!(item_bus_name("/only/path"), None);
        assert_eq!(item_bus_name(""), None);
    }

    #[test]
    fn hidden_keys() {
        assert_eq!(Hidden::Tray.key(), "tray");
        assert_eq!(Hidden::Minimised.key(), "minimised");
        assert_eq!(Hidden::NoWindow.key(), "no_window");
    }

    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    const UNIT: &str = "app-uq-autostart-nm-applet-0000abcd";

    /// A Hyprland that lists `clients` and records every `eval`.
    fn fake_hyprland(
        dir: &std::path::Path,
        clients: String,
    ) -> (HyprSocket, Arc<Mutex<Vec<String>>>) {
        let sock = dir.join("hypr/sig");
        std::fs::create_dir_all(&sock).unwrap();
        let listener = UnixListener::bind(sock.join(".socket.sock")).unwrap();
        let evals = Arc::new(Mutex::new(Vec::new()));
        let seen = evals.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let reply = if req == "j/clients" {
                    clients.clone()
                } else {
                    seen.lock().unwrap().push(req);
                    "ok".to_string()
                };
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });
        (
            HyprSocket::new(Some(dir.to_path_buf()), Some("sig".into())),
            evals,
        )
    }

    struct FakeTray(Vec<u32>);

    #[async_trait]
    impl Tray for FakeTray {
        async fn pids(&self) -> Vec<u32> {
            self.0.clone()
        }
    }

    /// `/proc` with pid 100 in the app's scope and pid 200 in someone else's.
    fn fake_proc(dir: &std::path::Path) -> PathBuf {
        for (pid, cg) in [
            (
                100,
                format!(
                    "0::/user.slice/user-1000.slice/user@1000.service/app.slice/{UNIT}.scope\n"
                ),
            ),
            (
                200,
                "0::/user.slice/user-1000.slice/session-2.scope\n".to_string(),
            ),
        ] {
            let d = dir.join("proc").join(pid.to_string());
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("cgroup"), cg).unwrap();
        }
        dir.join("proc")
    }

    const CLIENTS: &str = r#"[
      {"address":"0xaaa1","mapped":true,"pid":100,"workspace":{"id":2}},
      {"address":"0xbbb2","mapped":true,"pid":200,"workspace":{"id":2}}
    ]"#;

    #[tokio::test]
    async fn an_app_with_a_tray_icon_is_closed_to_the_tray() {
        let dir = tempfile::tempdir().unwrap();
        let (hypr, evals) = fake_hyprland(dir.path(), CLIENTS.into());
        let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![100])), fake_proc(dir.path()))
            .with_waits(Duration::from_millis(50), Duration::from_millis(100));
        assert_eq!(hider.hide(UNIT).await, Some(Hidden::Tray));
        // Only the app's own window, and only a close.
        assert_eq!(
            evals.lock().unwrap().clone(),
            vec![format!("eval {}", close_lua("0xaaa1"))]
        );
    }

    #[tokio::test]
    async fn an_app_without_a_tray_icon_is_minimised_where_it_opened() {
        let dir = tempfile::tempdir().unwrap();
        let (hypr, evals) = fake_hyprland(dir.path(), CLIENTS.into());
        // Another app's tray icon doesn't count.
        let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![200])), fake_proc(dir.path()))
            .with_waits(Duration::from_millis(50), Duration::from_millis(100));
        assert_eq!(hider.hide(UNIT).await, Some(Hidden::Minimised));
        let want: Vec<String> = minimise_lua("0xaaa1", 2)
            .into_iter()
            .map(|l| format!("eval {l}"))
            .collect();
        assert_eq!(evals.lock().unwrap().clone(), want);
    }

    #[tokio::test]
    async fn an_unreachable_compositor_gives_no_answer() {
        let dir = tempfile::tempdir().unwrap();
        let hypr = HyprSocket::new(Some(dir.path().to_path_buf()), Some("none".into()));
        let hider = SystemHider::with(hypr, Box::new(FakeTray(vec![])), fake_proc(dir.path()));
        assert_eq!(hider.hide(UNIT).await, None);
    }
}
