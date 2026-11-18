//! Test helper: become the clipboard, offering `DATA` under every type given, for a few seconds.
//! `offer 5 secret text/plain x-kde-passwordManagerHint` stands in for a password manager.

use std::sync::Arc;
use std::sync::mpsc::channel;
use uq_clip::watch;

fn main() {
    let mut a = std::env::args().skip(1);
    let secs: u64 = a.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    let data = a.next().unwrap_or_default();
    let mimes: Vec<String> = a.collect();
    let (tx, _rx) = channel();
    let h = watch::spawn(tx, Arc::new(|_, _| false), 1 << 20).expect("no data-control");
    h.set_selection(mimes, data.into_bytes());
    std::thread::sleep(std::time::Duration::from_secs(secs));
    h.stop();
}
