//! The state of `uqlip` and the handling of each request. Kept apart from the socket and the
//! Wayland code so tests can drive it directly.

use crate::config::Config;
use crate::ipc::{Request, Status};
use crate::store::{Added, Kind, Store};
use crate::watch::{self, Captured};
use serde_json::{Value, json};
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

type Setter = Box<dyn Fn(Vec<String>, Vec<u8>) + Send>;

pub struct Daemon {
    pub store: Store,
    pub config: Config,
    pub paused: bool,
    pub protocol: &'static str,
    setter: Option<Setter>,
    watchers: Vec<Sender<String>>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn err(msg: impl ToString) -> Value {
    json!({"ok": false, "error": msg.to_string()})
}

impl Daemon {
    pub fn new(store: Store, config: Config) -> Daemon {
        Daemon {
            store,
            config,
            paused: false,
            protocol: "none",
            setter: None,
            watchers: Vec::new(),
        }
    }

    pub fn set_setter(&mut self, f: impl Fn(Vec<String>, Vec<u8>) + Send + 'static) {
        self.setter = Some(Box::new(f));
    }

    pub fn status(&self) -> Status {
        let all = self.store.list("", None, usize::MAX);
        Status {
            running: true,
            paused: self.paused,
            enabled: self.config.enabled,
            count: all.len(),
            pinned: all.iter().filter(|e| e.pinned).count(),
            protocol: self.protocol.to_owned(),
            latest: all
                .iter()
                .filter(|e| !e.pinned)
                .map(|e| e.id)
                .max()
                .unwrap_or(0),
        }
    }

    fn state_line(&self) -> String {
        let mut v = serde_json::to_value(self.status()).unwrap_or_default();
        v["event"] = json!("state");
        v.to_string()
    }

    /// Tell every watcher; drop the ones that hung up.
    fn broadcast(&mut self) {
        let line = self.state_line();
        self.watchers.retain(|w| w.send(line.clone()).is_ok());
    }

    pub fn add_watcher(&mut self, tx: Sender<String>) {
        let _ = tx.send(self.state_line());
        self.watchers.push(tx);
    }

    /// A copy the watcher read and the privacy rules allowed.
    pub fn captured(&mut self, c: Captured, app: Option<&str>) {
        if !self.config.enabled || self.paused {
            return;
        }
        let Some((_, kind)) = crate::privacy::choose(&c.offered) else {
            return;
        };
        if kind == Kind::Text && c.data.len() as u64 > self.config.privacy.max_text_bytes {
            return;
        }
        match self.store.add(kind, &c.mime, &c.data, app, now()) {
            Ok(Added::New(_) | Added::Bumped(_)) => self.broadcast(),
            Ok(_) => {}
            Err(e) => tracing::warn!("couldn't record a copy: {e}"),
        }
    }

    pub fn handle(&mut self, req: Request) -> Value {
        match self.run(req) {
            Ok(v) => v,
            Err(e) => err(e),
        }
    }

    fn run(&mut self, req: Request) -> Result<Value, String> {
        let s = |e: crate::store::StoreError| e.to_string();
        Ok(match req {
            Request::List { query, kind, limit } => {
                // An image's entry says where its file is, so the shell can show a thumbnail.
                let entries: Vec<Value> = self
                    .store
                    .list(&query, kind, limit.min(1000))
                    .into_iter()
                    .map(|e| {
                        let mut v = serde_json::to_value(&e).unwrap_or_default();
                        if e.kind == Kind::Image
                            && let Ok(p) = self.store.blob_path(e.id)
                        {
                            v["path"] = json!(p);
                        }
                        v
                    })
                    .collect();
                json!({"ok": true, "entries": entries})
            }
            Request::Get { id } => {
                let e = self.store.entry(id).map_err(s)?.clone();
                match e.kind {
                    Kind::Text => {
                        let data = self.store.data(id).map_err(s)?;
                        let text = String::from_utf8_lossy(&data[..data.len().min(1 << 20)]);
                        json!({"ok": true, "entry": e, "text": text})
                    }
                    Kind::Image => {
                        let path = self.store.blob_path(id).map_err(s)?;
                        json!({"ok": true, "entry": e, "path": path})
                    }
                }
            }
            Request::Copy { id } => {
                let e = self.store.entry(id).map_err(s)?.clone();
                let data = self.store.data(id).map_err(s)?;
                let set = self
                    .setter
                    .as_ref()
                    .ok_or("the clipboard can't be reached")?;
                set(watch::offer_types(&e.mime), data);
                // Using an entry again brings it to the top (the watcher will not see our own
                // selection, so do it here).
                if let Ok(data) = self.store.data(id) {
                    let _ = self
                        .store
                        .add(e.kind, &e.mime, &data, e.app.as_deref(), now());
                }
                self.broadcast();
                json!({"ok": true})
            }
            Request::Pin { id, pinned } => {
                self.store.set_pinned(id, pinned).map_err(s)?;
                self.broadcast();
                json!({"ok": true})
            }
            Request::Delete { id } => {
                self.store.delete(id).map_err(s)?;
                self.broadcast();
                json!({"ok": true})
            }
            Request::Clear { keep_pinned } => {
                let n = self.store.clear(keep_pinned).map_err(s)?;
                self.broadcast();
                json!({"ok": true, "removed": n})
            }
            Request::Pause { paused } => {
                self.paused = paused;
                self.broadcast();
                json!({"ok": true, "paused": paused})
            }
            Request::Status => json!({"ok": true, "status": self.status()}),
            Request::Reload => {
                self.config = Config::load(&crate::config::config_path());
                self.store.set_limits(self.config.limits());
                self.broadcast();
                json!({"ok": true})
            }
            Request::Watch => json!({"ok": true}),
        })
    }
}
