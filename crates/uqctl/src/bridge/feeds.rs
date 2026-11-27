//! What the bar's data islands share: the HTTP client config, size-capped body reads, the
//! on-disk cache of the last good payload, and the fetch loop that runs only while an island is
//! placed (stocks and news use it; see `stocks.rs` and `news.rs`).
//!
//! Rules as in `net.rs`: HTTPS only (plain-HTTP bases exist only under `cfg(test)`), 5 s
//! timeout, fixed User-Agent, no cookies, nothing logged but the kind of a failure.

use super::protocol::Outbound;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::future::Future;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::Settings;

pub const TIMEOUT: Duration = Duration::from_secs(5);
pub const USER_AGENT: &str = concat!("uq/", env!("CARGO_PKG_VERSION"));
/// Redirects a feed may take, each to https.
pub const MAX_REDIRECTS: usize = 3;
/// Retry steps after a failed fetch, capped at the period; a success starts over.
pub const RETRY: [Duration; 4] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
    Duration::from_secs(300),
];
/// Least time between two manual refreshes.
pub const MANUAL_GAP: Duration = Duration::from_secs(30);

/// The one client config every bridge fetch uses: fixed UA, 5 s timeout, no cookie store.
pub fn client(redirect: reqwest::redirect::Policy) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .redirect(redirect)
        .build()
        .expect("static reqwest client config")
}

/// Follow a redirect only from https to https, at most [`MAX_REDIRECTS`] times.
pub fn https_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|a| {
        if may_follow(a.previous(), a.url()) {
            a.follow()
        } else {
            a.error("redirect refused")
        }
    })
}

/// `previous`: every URL requested so far in the chain (the first is the original).
pub fn may_follow(previous: &[reqwest::Url], next: &reqwest::Url) -> bool {
    previous.len() <= MAX_REDIRECTS
        && next.scheme() == "https"
        && previous.iter().all(|u| u.scheme() == "https")
}

/// The kind of a request failure; never the URL or a body.
pub fn kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_redirect() {
        "redirect"
    } else if e.is_decode() || e.is_body() {
        "decode"
    } else {
        "other"
    }
}

/// GET `url` and read at most `cap` bytes of the body. Over the cap: an error, or with
/// `truncate` the first `cap` bytes (the rest is never read).
pub async fn get_capped(
    http: &reqwest::Client,
    url: &str,
    cap: usize,
    truncate: bool,
) -> Result<Vec<u8>, &'static str> {
    let mut resp = http.get(url).send().await.map_err(|e| kind(&e))?;
    if !resp.status().is_success() {
        return Err("status");
    }
    if !truncate && resp.content_length().is_some_and(|n| n > cap as u64) {
        return Err("too large");
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| kind(&e))? {
        let room = cap - body.len();
        if chunk.len() > room {
            if !truncate {
                return Err("too large");
            }
            body.extend_from_slice(&chunk[..room]);
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// `$XDG_CACHE_HOME/uq/feeds` (absolute values only), else `~/.cache/uq/feeds`.
pub fn cache_dir() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let base = xdg.or_else(|| {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|h| h.join(".cache"))
    })?;
    Some(base.join("uq").join("feeds"))
}

/// Write `contents` atomically as a 0600 file (directory 0700): temp file, sync, rename.
pub fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp-{}", std::process::id()));
    let tmp = path.with_file_name(name);
    let _ = std::fs::remove_file(&tmp);
    let res = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .and_then(|mut f| {
            f.write_all(contents.as_bytes())?;
            f.sync_all()
        })
        .and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// One island's data source for [`FeedLoop`].
pub trait Source: Send + Sync + 'static {
    /// What a fetch depends on (the symbols, the feed URLs); a change refetches at once and the
    /// cache only counts when it was written for the same key.
    type Key: Clone + PartialEq + Serialize + DeserializeOwned + Send + Sync;
    /// Event name.
    const EVENT: &'static str;
    /// Cache file name inside the cache directory.
    const CACHE: &'static str;
    /// The key while the island is placed, `None` while it isn't.
    fn key(s: &Settings) -> Option<Self::Key>;
    /// One fetch round. Returns the payload; its `status` is `ok`, `partial` or `error`. `last`
    /// is the previous payload (fresh or from the cache), to carry its values over failures.
    fn fetch(&self, key: &Self::Key, last: Option<&Value>) -> impl Future<Output = Value> + Send;
    /// Called after each event this source's loop sent (`Null` when the island went away).
    fn emitted(&self, _payload: &Value) {}
}

/// Runs a [`Source`] only while its island is placed: at once when placed or when the key
/// changes, then every `period`; failures back off; a network change after a failure retries
/// at once. Removing the island emits `null` once and stops fetching.
pub struct FeedLoop<S: Source> {
    pub src: S,
    tx: mpsc::Sender<Outbound>,
    settings: watch::Receiver<Option<Settings>>,
    stop: CancellationToken,
    period: Duration,
    retry: Vec<Duration>,
    cache_dir: Option<PathBuf>,
    failed: AtomicBool,
    placed: AtomicBool,
    net_kick: Notify,
    now_kick: Notify,
    last_manual: std::sync::Mutex<Option<Instant>>,
    manual_gap: Duration,
}

impl<S: Source> FeedLoop<S> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        src: S,
        tx: mpsc::Sender<Outbound>,
        settings: watch::Receiver<Option<Settings>>,
        stop: CancellationToken,
        period: Duration,
        retry: Vec<Duration>,
        cache_dir: Option<PathBuf>,
        manual_gap: Duration,
    ) -> Arc<Self> {
        Arc::new(FeedLoop {
            src,
            tx,
            settings,
            stop,
            period,
            retry,
            cache_dir,
            failed: AtomicBool::new(false),
            placed: AtomicBool::new(false),
            net_kick: Notify::new(),
            now_kick: Notify::new(),
            last_manual: std::sync::Mutex::new(None),
            manual_gap,
        })
    }

    async fn emit(&self, data: Value) -> bool {
        let out = Outbound::Event {
            event: S::EVENT.into(),
            data: data.clone(),
        };
        let sent = tokio::select! { _ = self.stop.cancelled() => false, r = self.tx.send(out) => r.is_ok() };
        if sent {
            self.src.emitted(&data);
        }
        sent
    }

    /// The shell reported a network change: retry now if the last fetch failed.
    pub fn network_changed(&self) {
        if self.failed.load(Ordering::SeqCst) {
            self.net_kick.notify_one();
        }
    }

    /// A manual refresh: true when one was started, false while the island isn't placed or
    /// within the rate limit of the last one.
    pub fn refresh_now(&self) -> bool {
        if !self.placed.load(Ordering::SeqCst) {
            return false;
        }
        let mut last = self.last_manual.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if last.is_some_and(|t| now.saturating_duration_since(t) < self.manual_gap) {
            return false;
        }
        *last = Some(now);
        self.now_kick.notify_one();
        true
    }

    fn cache_path(&self) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|d| d.join(S::CACHE))
    }

    /// The cached payload, if it was written for `key`.
    fn read_cache(&self, key: &S::Key) -> Option<Value> {
        let text = std::fs::read_to_string(self.cache_path()?).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        let k: S::Key = serde_json::from_value(v.get("key")?.clone()).ok()?;
        (k == *key)
            .then(|| v.get("payload").cloned())
            .flatten()
            .filter(Value::is_object)
    }

    fn write_cache(&self, key: &S::Key, payload: &Value) {
        let Some(path) = self.cache_path() else {
            return;
        };
        let text = json!({"key": key, "payload": payload}).to_string();
        if let Err(e) = write_private(&path, &text) {
            eprintln!(
                "uqctl bridge: could not write the {} cache: {}",
                S::EVENT,
                e.kind()
            );
        }
    }

    pub async fn run(self: Arc<Self>) {
        let mut rx = self.settings.clone();
        let mut current: Option<S::Key> = None;
        let mut next: Option<tokio::time::Instant> = None;
        let mut failures = 0usize;
        let mut last: Option<Value> = None;
        loop {
            let want = rx.borrow_and_update().as_ref().and_then(S::key);
            if want != current {
                let had = current.is_some();
                current = want;
                match &current {
                    Some(k) => {
                        self.placed.store(true, Ordering::SeqCst);
                        if !had && let Some(mut cached) = self.read_cache(k) {
                            cached["status"] = json!("stale");
                            if !self.emit(cached.clone()).await {
                                return;
                            }
                            last = Some(cached);
                        }
                        if !self.refresh(k, &mut last, &mut failures, &mut next).await {
                            return;
                        }
                    }
                    None => {
                        self.placed.store(false, Ordering::SeqCst);
                        next = None;
                        failures = 0;
                        self.failed.store(false, Ordering::SeqCst);
                        if had && !self.emit(Value::Null).await {
                            return;
                        }
                    }
                }
            }
            let run_now = tokio::select! {
                _ = self.stop.cancelled() => return,
                r = rx.changed() => { if r.is_err() { return } false }
                _ = self.net_kick.notified() => self.failed.load(Ordering::SeqCst),
                _ = self.now_kick.notified() => true,
                _ = tokio::time::sleep_until(next.unwrap_or_else(tokio::time::Instant::now)), if next.is_some() => true,
            };
            if run_now
                && let Some(k) = &current
                && !self.refresh(k, &mut last, &mut failures, &mut next).await
            {
                return;
            }
        }
    }

    /// Fetch, emit, cache a good result, schedule the next round; false only when stopping.
    async fn refresh(
        &self,
        key: &S::Key,
        last: &mut Option<Value>,
        failures: &mut usize,
        next: &mut Option<tokio::time::Instant>,
    ) -> bool {
        let payload = tokio::select! {
            _ = self.stop.cancelled() => return false,
            p = self.src.fetch(key, last.as_ref()) => p,
        };
        let good = matches!(payload["status"].as_str(), Some("ok" | "partial"));
        if !self.emit(payload.clone()).await {
            return false;
        }
        let wait = if good {
            *failures = 0;
            self.failed.store(false, Ordering::SeqCst);
            self.write_cache(key, &payload);
            self.period
        } else {
            let step = self.retry.get(*failures).copied().unwrap_or(self.period);
            *failures += 1;
            self.failed.store(true, Ordering::SeqCst);
            step.min(self.period)
        };
        *last = Some(payload);
        *next = Some(tokio::time::Instant::now() + wait);
        true
    }
}

/// Overall status of a round of `n` requests of which `ok` succeeded (no requests is `ok`).
pub fn status(ok: usize, n: usize) -> &'static str {
    if ok == n {
        "ok"
    } else if ok == 0 {
        "error"
    } else {
        "partial"
    }
}

/// True when `id` is placed in the bar.
pub fn placed(s: &Settings, id: &str) -> bool {
    s.shell.bar.as_ref().is_some_and(|b| b.has(id))
}

#[cfg(test)]
pub(crate) mod test_http;

#[cfg(test)]
mod tests;
