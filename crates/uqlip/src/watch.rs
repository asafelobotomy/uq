//! The Wayland side: follows the clipboard and sets it.
//!
//! Uses the `ext-data-control-v1` protocol when the compositor has it (Hyprland does) and the
//! older `wlr-data-control-unstable-v1` otherwise (sway and other wlroots compositors). Both let a
//! client without focus see every copy; neither needs a window. The two differ only in names, so
//! one macro writes the code for both.
//!
//! A copy is handled in two steps so a secret is never read: first the offered types are shown
//! to a policy (which also reads the password-manager hint), and only if it agrees is the data
//! read, with a size cap and a timeout. Reading happens on its own thread, so a slow or hung
//! source can never stall the event loop.

use std::collections::HashMap;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wayland_client::Connection;

use crate::privacy::SECRET_HINT_MIME;

/// Decides, from the offered types and the hint's content, whether to read the copy at all.
pub type Policy = Arc<dyn Fn(&[String], Option<&str>) -> bool + Send + Sync>;

#[derive(Debug)]
pub struct Captured {
    pub offered: Vec<String>,
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum Event {
    /// A new selection whose data was read.
    Copied(Captured),
    /// The selection changed to something the policy refused or that had no usable type.
    Ignored,
    /// The compositor offers no data-control protocol, or the connection failed for another
    /// reason (already logged): there is nothing to watch.
    Unsupported,
    /// The compositor closed the connection: it is shutting down (logout, shutdown, a crash).
    Closed,
}

/// Whether a Wayland error means the compositor went away (the socket hung up), rather than a
/// protocol error or a bug on either side.
fn compositor_gone(e: &io::Error) -> bool {
    use wayland_client::backend::WaylandError;
    let kind = match e.get_ref().and_then(|i| i.downcast_ref::<WaylandError>()) {
        Some(WaylandError::Io(io)) => io.kind(),
        Some(WaylandError::Protocol(_)) => return false,
        None => e.kind(),
    };
    matches!(
        kind,
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::UnexpectedEof
    )
}

enum Request {
    Set {
        mimes: Vec<String>,
        data: Arc<Vec<u8>>,
    },
    Stop,
}

/// Held by the daemon to change the selection or stop the watcher.
pub struct Handle {
    tx: Sender<Request>,
    wake: OwnedFd,
    which: &'static str,
}

impl Handle {
    /// Which protocol is in use: "ext-data-control" or "wlr-data-control".
    pub fn protocol(&self) -> &'static str {
        self.which
    }

    fn nudge(&self) {
        let _ = unsafe { libc::write(self.wake.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
    }

    /// Make `data` the clipboard, offered under `mimes` (text gets its usual aliases).
    pub fn set_selection(&self, mimes: Vec<String>, data: Vec<u8>) {
        let _ = self.tx.send(Request::Set {
            mimes,
            data: Arc::new(data),
        });
        self.nudge();
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Request::Stop);
        self.nudge();
    }
}

/// Read up to `cap` bytes (more than that is an error) within `timeout`.
fn read_capped(fd: OwnedFd, cap: u64, timeout: Duration) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    let raw = fd.as_raw_fd();
    let mut file = std::fs::File::from(fd);
    let mut buf = [0u8; 65536];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let mut p = libc::pollfd {
            fd: raw,
            events: libc::POLLIN,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut p, 1, left.as_millis().min(i32::MAX as u128) as i32) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let got = file.read(&mut buf)?;
        if got == 0 {
            return Ok(out);
        }
        out.extend_from_slice(&buf[..got]);
        if out.len() as u64 > cap {
            return Err(io::Error::other("too big"));
        }
    }
}

fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn text_mimes(first: &str) -> Vec<String> {
    let mut v = vec![first.to_owned()];
    for m in [
        "text/plain;charset=utf-8",
        "text/plain",
        "UTF8_STRING",
        "STRING",
        "TEXT",
    ] {
        if !v.iter().any(|x| x == m) {
            v.push(m.to_owned());
        }
    }
    v
}

/// The types to offer for stored data.
pub fn offer_types(mime: &str) -> Vec<String> {
    if mime.starts_with("text/") || mime == "UTF8_STRING" || mime == "STRING" {
        text_mimes(mime)
    } else {
        vec![mime.to_owned()]
    }
}

#[macro_use]
mod backend;

backend!(
    ext,
    wayland_protocols::ext::data_control::v1::client,
    ext_data_control_manager_v1 ExtDataControlManagerV1,
    ext_data_control_device_v1 ExtDataControlDeviceV1,
    ext_data_control_source_v1 ExtDataControlSourceV1,
    ext_data_control_offer_v1 ExtDataControlOfferV1,
    "ext-data-control"
);
backend!(
    wlr,
    wayland_protocols_wlr::data_control::v1::client,
    zwlr_data_control_manager_v1 ZwlrDataControlManagerV1,
    zwlr_data_control_device_v1 ZwlrDataControlDeviceV1,
    zwlr_data_control_source_v1 ZwlrDataControlSourceV1,
    zwlr_data_control_offer_v1 ZwlrDataControlOfferV1,
    "wlr-data-control"
);

/// Start watching on a thread of its own. `max_bytes` caps one copy.
pub fn spawn(events: Sender<Event>, policy: Policy, max_bytes: u64) -> io::Result<Handle> {
    let conn = Connection::connect_to_env().map_err(io::Error::other)?;
    let use_ext = ext::available(&conn);
    if !use_ext && !wlr::available(&conn) {
        let _ = events.send(Event::Unsupported);
        return Err(io::Error::other(
            "the compositor has no data-control protocol",
        ));
    }
    let (tx, rx) = channel();
    let (wake_r, wake_w) = pipe()?;
    let which = if use_ext {
        "ext-data-control"
    } else {
        "wlr-data-control"
    };
    std::thread::Builder::new()
        .name("uqlip-wayland".into())
        .spawn(move || {
            let r = if use_ext {
                ext::run(conn, events.clone(), policy, max_bytes, rx, wake_r)
            } else {
                wlr::run(conn, events.clone(), policy, max_bytes, rx, wake_r)
            };
            match r {
                Ok(()) => {}
                // Normal at logout and shutdown: the compositor exits before the session's
                // services are stopped.
                Err(e) if compositor_gone(&e) => {
                    tracing::info!("the compositor closed the Wayland connection ({e})");
                    let _ = events.send(Event::Closed);
                }
                Err(e) => {
                    tracing::error!("[CLIP-002] the Wayland connection ended: {e}");
                    let _ = events.send(Event::Unsupported);
                }
            }
        })?;
    let _ = HashMap::<(), ()>::new;
    Ok(Handle {
        tx,
        wake: wake_w,
        which,
    })
}

#[cfg(test)]
mod tests {
    use super::compositor_gone;
    use std::io;
    use wayland_client::backend::{WaylandError, protocol::ProtocolError};

    #[test]
    fn a_hung_up_socket_means_the_compositor_has_gone() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::ConnectionReset] {
            let e = io::Error::other(WaylandError::Io(kind.into()));
            assert!(compositor_gone(&e), "{kind:?}");
        }
        assert!(compositor_gone(&io::ErrorKind::BrokenPipe.into()));
    }

    #[test]
    fn other_errors_are_still_errors() {
        let proto = WaylandError::Protocol(ProtocolError {
            code: 1,
            object_id: 3,
            object_interface: "ext_data_control_device_v1".into(),
            message: "bad".into(),
        });
        assert!(!compositor_gone(&io::Error::other(proto)));
        assert!(!compositor_gone(&io::Error::other(WaylandError::Io(
            io::ErrorKind::PermissionDenied.into()
        ))));
        assert!(!compositor_gone(&io::Error::other(
            "no data-control protocol"
        )));
    }
}
