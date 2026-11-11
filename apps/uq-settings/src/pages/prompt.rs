//! A question a system service asks the person through an open page: NetworkManager asking
//! for a Wi-Fi password, BlueZ asking to confirm a pairing code. The service's call waits in
//! [`Prompts::ask`] until the page answers it, the service cancels it, a newer question
//! replaces it, or the page closes ([`Prompts::close`]). Nothing is ever answered without the
//! person: there is no default answer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::{Notify, oneshot};

/// The person's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Yes, with what they typed (field name → text), if anything.
    Yes(HashMap<String, String>),
    No,
}

struct Pending {
    id: u64,
    /// What the service names the question by, to cancel it.
    key: String,
    shown: Value,
    reply: oneshot::Sender<Answer>,
}

#[derive(Default)]
pub struct Prompts {
    pending: Mutex<Option<Pending>>,
    next: AtomicU64,
    closed: std::sync::atomic::AtomicBool,
    changed: Mutex<Option<Arc<Notify>>>,
}

impl Prompts {
    /// Ping `n` whenever the question on show changes.
    pub fn follow(&self, n: Arc<Notify>) {
        *self.changed.lock().unwrap() = Some(n);
    }

    fn ping(&self) {
        if let Some(n) = &*self.changed.lock().unwrap() {
            n.notify_one();
        }
    }

    /// Show `shown` and wait for the answer. `None` when it was cancelled, replaced, or the
    /// page closed.
    pub async fn ask(&self, key: &str, shown: Value) -> Option<Answer> {
        if self.closed.load(Ordering::SeqCst) {
            return None;
        }
        let (tx, rx) = oneshot::channel();
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        *self.pending.lock().unwrap() = Some(Pending {
            id,
            key: key.to_string(),
            shown,
            reply: tx,
        });
        self.ping();
        let answer = rx.await.ok();
        let mut p = self.pending.lock().unwrap();
        if p.as_ref().is_some_and(|p| p.id == id) {
            *p = None;
        }
        drop(p);
        self.ping();
        answer
    }

    /// Answer question `id`; false when it is no longer on show.
    pub fn answer(&self, id: u64, a: Answer) -> bool {
        let mut p = self.pending.lock().unwrap();
        if p.as_ref().is_some_and(|p| p.id == id) {
            let taken = p.take().expect("checked");
            drop(p);
            return taken.reply.send(a).is_ok();
        }
        false
    }

    /// The service gave up on the question named `key` (`None`: whatever is on show).
    pub fn cancel(&self, key: Option<&str>) {
        let mut p = self.pending.lock().unwrap();
        if p.as_ref().is_some_and(|p| key.is_none_or(|k| p.key == k)) {
            *p = None;
        }
        drop(p);
        self.ping();
    }

    /// The page closed: drop the question on show and refuse new ones.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        *self.pending.lock().unwrap() = None;
    }

    /// The question on show as `{id, …}`, or null.
    pub fn current(&self) -> Value {
        match &*self.pending.lock().unwrap() {
            Some(p) => {
                let mut v = p.shown.clone();
                if let Some(o) = v.as_object_mut() {
                    o.insert("id".into(), json!(p.id));
                }
                v
            }
            None => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn answered_cancelled_replaced_and_closed() {
        let p = Arc::new(Prompts::default());
        let n = Arc::new(Notify::new());
        p.follow(n.clone());

        let asker = {
            let p = p.clone();
            tokio::spawn(async move { p.ask("a", json!({"kind": "confirm"})).await })
        };
        n.notified().await;
        let shown = p.current();
        assert_eq!(shown["kind"], "confirm");
        let id = shown["id"].as_u64().unwrap();
        assert!(!p.answer(id + 1, Answer::No), "not the one on show");
        assert!(p.answer(id, Answer::No));
        assert_eq!(asker.await.unwrap(), Some(Answer::No));
        assert!(p.current().is_null());

        let first = {
            let p = p.clone();
            tokio::spawn(async move { p.ask("a", json!({})).await })
        };
        while p.current().is_null() {
            tokio::task::yield_now().await;
        }
        let second = {
            let p = p.clone();
            tokio::spawn(async move { p.ask("b", json!({})).await })
        };
        assert_eq!(first.await.unwrap(), None, "replaced by a newer question");
        while p.current().is_null() {
            tokio::task::yield_now().await;
        }
        p.cancel(Some("a"));
        assert!(!p.current().is_null(), "cancelling another key leaves it");
        p.cancel(Some("b"));
        assert_eq!(second.await.unwrap(), None);

        p.close();
        assert_eq!(p.ask("c", json!({})).await, None);
    }
}
