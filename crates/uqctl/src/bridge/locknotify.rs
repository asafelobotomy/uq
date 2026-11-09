//! Lock state from Hyprland's `hyprland-lock-notify-v1` (spec §2.7).
//!
//! A blocking Wayland event loop runs on its own `std::thread` and feeds each reported state into
//! a tokio channel. The thread holds only the sender: once the bridge drops the receiver, its
//! next send fails and the thread ends. The bridge never joins it, so a thread blocked waiting on
//! the compositor cannot keep the process alive at stdin EOF.
//!
//! The protocol XML is vendored from hyprwm/hyprland-protocols (BSD-3-Clause).

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use tokio::sync::mpsc;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};

#[allow(dead_code, non_upper_case_globals, unused_imports, clippy::all)]
mod proto {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-lock-notify-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-lock-notify-v1.xml");
}

use proto::hyprland_lock_notification_v1::{Event, HyprlandLockNotificationV1};
use proto::hyprland_lock_notifier_v1::HyprlandLockNotifierV1;

/// The lock state a protocol event reports, if any (a later protocol version may add events
/// that report none).
pub fn lock_event(ev: &Event) -> Option<bool> {
    match ev {
        Event::Locked => Some(true),
        Event::Unlocked => Some(false),
    }
}

/// Start the watcher thread for `display` (a `WAYLAND_DISPLAY` value) and return the receiver of
/// lock states. The thread is detached: see the module doc.
pub fn spawn(display: Option<OsString>) -> mpsc::Receiver<bool> {
    let (tx, rx) = mpsc::channel(16);
    let started = std::thread::Builder::new()
        .name("uq-lock-notify".into())
        .spawn(move || watch(display, tx));
    if let Err(e) = started {
        eprintln!("uqctl bridge: lock-notify thread: {e}; lock state is off");
    }
    rx
}

/// The socket a `WAYLAND_DISPLAY` value names: absolute as given, else under `XDG_RUNTIME_DIR`.
fn socket_path(display: &OsString) -> Option<PathBuf> {
    let p = PathBuf::from(display);
    if p.is_absolute() {
        return Some(p);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())?;
    Some(PathBuf::from(dir).join(p))
}

struct State {
    tx: mpsc::Sender<bool>,
    /// The receiver is gone: stop.
    done: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore HyprlandLockNotifierV1);

impl Dispatch<HyprlandLockNotificationV1, ()> for State {
    fn event(
        st: &mut Self,
        _: &HyprlandLockNotificationV1,
        ev: Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(locked) = lock_event(&ev)
            && st.tx.blocking_send(locked).is_err()
        {
            st.done = true;
        }
    }
}

/// Connect to `display`, subscribe to lock notifications and send each reported state on `tx`
/// until the connection or the receiver goes away. Blocking; problems are logged once and end it.
pub fn watch(display: Option<OsString>, tx: mpsc::Sender<bool>) {
    let off = |what: &dyn std::fmt::Display| {
        eprintln!("uqctl bridge: lock-notify off ({what}); no LockState events");
    };
    let Some(display) = display.filter(|d| !d.is_empty()) else {
        return off(&"no Wayland display");
    };
    let Some(path) = socket_path(&display) else {
        return off(&"no XDG_RUNTIME_DIR for the Wayland socket");
    };
    let conn = match UnixStream::connect(&path).map(Connection::from_socket) {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return off(&format!("Wayland connection: {e}")),
        Err(e) => return off(&format!("Wayland socket {}: {e}", path.display())),
    };
    let (globals, mut queue) = match registry_queue_init::<State>(&conn) {
        Ok(g) => g,
        Err(e) => return off(&format!("Wayland registry: {e}")),
    };
    let qh = queue.handle();
    let notifier: HyprlandLockNotifierV1 = match globals.bind(&qh, 1..=1, ()) {
        Ok(n) => n,
        Err(e) => return off(&format!("no hyprland_lock_notifier_v1 ({e})")),
    };
    let _notification = notifier.get_lock_notification(&qh, ());
    let mut st = State { tx, done: false };
    while !st.done {
        if let Err(e) = queue.blocking_dispatch(&mut st) {
            return off(&format!("Wayland connection lost: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lock_event_maps() {
        assert_eq!(lock_event(&Event::Locked), Some(true));
        assert_eq!(lock_event(&Event::Unlocked), Some(false));
    }

    #[tokio::test]
    async fn no_display_is_quiet() {
        let (tx, mut rx) = mpsc::channel(4);
        // Called directly with no display name: it must not fall back to the environment.
        tokio::task::spawn_blocking(move || watch(None, tx))
            .await
            .unwrap();
        assert!(
            rx.recv().await.is_none(),
            "no event, and the sender is gone"
        );

        // A display that does not exist is just as quiet.
        let mut rx = spawn(Some("uq-no-such-wayland-display".into()));
        let got = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("the watcher gives up promptly");
        assert_eq!(got, None);
    }
}
