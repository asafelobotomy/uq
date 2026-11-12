//! Hyprland's event socket: which screens came and went, and config reloads, debounced into one
//! nudge per burst.
//!
//! Only `monitoradded>>NAME`, `monitorremoved>>NAME` and `configreloaded>>` are read; the `…v2`
//! events come with each of them and are ignored. Everything on the socket is untrusted text.

use crate::dbus::SharedDesktop;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// A screen appeared or went away, or Hyprland reloaded its config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputEvent {
    Added(String),
    Removed(String),
    ConfigReloaded,
}

/// How long the socket must be quiet before a burst of events counts as one change.
pub const DEBOUNCE: Duration = Duration::from_secs(1);

/// One quiet second after a run of events: `outputs` is whether any screen came or went,
/// `added` whether any of them was an add, `config` whether Hyprland reloaded its config.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Burst {
    pub outputs: bool,
    pub added: bool,
    pub config: bool,
}

/// The listener's clocks; tests shorten them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timing {
    pub debounce: Duration,
    pub backoff_start: Duration,
    pub backoff_max: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            debounce: DEBOUNCE,
            backoff_start: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
        }
    }
}

/// Longest line worth parsing; Hyprland cuts its own at 1024 bytes.
const MAX_LINE: usize = 4096;

/// `monitoradded>>NAME`, `monitorremoved>>NAME` and `configreloaded>>`; anything else, an odd
/// name or a line over 4 KiB is `None`.
pub fn parse_event(line: &str) -> Option<OutputEvent> {
    if line.len() > MAX_LINE {
        return None;
    }
    let line = line.trim_end_matches(['\n', '\r']);
    let name = |rest: &str| uq_core::settings::valid_output_name(rest).then(|| rest.to_string());
    if line == "configreloaded>>" {
        return Some(OutputEvent::ConfigReloaded);
    }
    if let Some(rest) = line.strip_prefix("monitoradded>>") {
        return name(rest).map(OutputEvent::Added);
    }
    line.strip_prefix("monitorremoved>>")
        .and_then(name)
        .map(OutputEvent::Removed)
}

/// Splits a byte stream into lines, dropping any line longer than [`MAX_LINE`].
#[derive(Default)]
struct Lines {
    buf: Vec<u8>,
    skipping: bool,
}

impl Lines {
    fn push(&mut self, chunk: &[u8], mut each: impl FnMut(&str)) {
        for &b in chunk {
            if b == b'\n' {
                if !self.skipping {
                    each(&String::from_utf8_lossy(&self.buf));
                }
                self.buf.clear();
                self.skipping = false;
            } else if !self.skipping {
                if self.buf.len() >= MAX_LINE {
                    self.buf.clear();
                    self.skipping = true;
                } else {
                    self.buf.push(b);
                }
            }
        }
    }
}

pub async fn listen(path: PathBuf, tx: mpsc::Sender<Burst>) {
    listen_with(path, tx, Timing::default(), &AtomicUsize::new(0)).await
}

/// Connect, read, debounce; on EOF or a missing socket wait (doubling up to the cap) and try
/// again. Returns only when the receiver is gone.
pub(crate) async fn listen_with(
    path: PathBuf,
    tx: mpsc::Sender<Burst>,
    timing: Timing,
    attempts: &AtomicUsize,
) {
    let mut wait = timing.backoff_start;
    loop {
        attempts.fetch_add(1, Ordering::SeqCst);
        match UnixStream::connect(&path).await {
            Ok(stream) => {
                wait = timing.backoff_start;
                if read_events(stream, &tx, timing.debounce).await.is_err() {
                    return;
                }
            }
            Err(e) => tracing::debug!("Hyprland's event socket isn't there yet: {e}"),
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(timing.backoff_max);
    }
}

/// Read one connection until it closes. `Err` means the receiver is gone.
async fn read_events(
    mut stream: UnixStream,
    tx: &mpsc::Sender<Burst>,
    debounce: Duration,
) -> Result<(), ()> {
    let mut lines = Lines::default();
    let mut chunk = [0u8; 2048];
    // The pending burst: when it fires, and what it holds.
    let mut burst: Option<(tokio::time::Instant, Burst)> = None;
    loop {
        let fire = async {
            match burst {
                Some((at, _)) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            read = stream.read(&mut chunk) => {
                let n = match read {
                    Ok(0) | Err(_) => return Ok(()),
                    Ok(n) => n,
                };
                let mut seen = false;
                let mut b = burst.map(|(_, b)| b).unwrap_or_default();
                lines.push(&chunk[..n], |l| {
                    if let Some(ev) = parse_event(l) {
                        seen = true;
                        match ev {
                            OutputEvent::Added(_) => (b.outputs, b.added) = (true, true),
                            OutputEvent::Removed(_) => b.outputs = true,
                            OutputEvent::ConfigReloaded => b.config = true,
                        }
                    }
                });
                if seen {
                    burst = Some((tokio::time::Instant::now() + debounce, b));
                }
            }
            () = fire => {
                if let Some((_, b)) = burst.take() {
                    tx.send(b).await.map_err(|_| ())?;
                }
            }
        }
    }
}

/// Follow Hyprland's events for the life of uqd: each burst takes the desktop lock once.
pub async fn run(shared: SharedDesktop, path: PathBuf) {
    let (tx, mut rx) = mpsc::channel(8);
    tokio::spawn(listen(path, tx));
    while let Some(burst) = rx.recv().await {
        let mut d = shared.lock().await;
        if burst.config {
            d.config_reloaded().await;
        }
        if burst.outputs {
            d.outputs_changed(burst.added).await;
        }
    }
}

#[cfg(test)]
mod tests;
