//! Idle tracking: the helper exits once no call has been in flight for 30 s.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the helper stays up with no calls.
pub const IDLE_EXIT: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct State {
    in_flight: usize,
    last: Instant,
}

/// Calls in flight and when the last one ended.
#[derive(Debug)]
pub struct Activity {
    state: Mutex<State>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                in_flight: 0,
                last: Instant::now(),
            }),
        }
    }
}

/// Held for the length of one call.
#[derive(Debug)]
pub struct Busy(Arc<Activity>);

impl Drop for Busy {
    fn drop(&mut self) {
        let mut s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.in_flight -= 1;
        s.last = Instant::now();
    }
}

impl Activity {
    pub fn begin(self: &Arc<Self>) -> Busy {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.in_flight += 1;
        s.last = Instant::now();
        Busy(self.clone())
    }

    /// How long nothing has been in flight; `None` while a call runs (a polkit prompt can
    /// take minutes).
    pub fn idle_for(&self) -> Option<Duration> {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        (s.in_flight == 0).then(|| s.last.elapsed())
    }

    /// Resolve once the helper has been idle for `limit`, checking every `poll`.
    pub async fn wait_idle(&self, limit: Duration, poll: Duration) {
        loop {
            match self.idle_for() {
                Some(d) if d >= limit => return,
                Some(d) => tokio::time::sleep((limit - d).min(poll)).await,
                None => tokio::time::sleep(poll).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exits_after_the_limit_with_no_calls() {
        let a = Arc::new(Activity::default());
        let t = Instant::now();
        a.wait_idle(Duration::from_millis(80), Duration::from_millis(10))
            .await;
        assert!(t.elapsed() >= Duration::from_millis(80));
    }

    #[tokio::test]
    async fn never_idle_while_a_call_is_in_flight() {
        let a = Arc::new(Activity::default());
        let busy = a.begin();
        assert_eq!(a.idle_for(), None);
        let w = a.wait_idle(Duration::from_millis(30), Duration::from_millis(5));
        let r = tokio::time::timeout(Duration::from_millis(120), w).await;
        assert!(r.is_err(), "went idle during a call");
        drop(busy);
        let d = a.idle_for().unwrap();
        assert!(d < Duration::from_millis(50));
        // A finished call restarts the clock.
        a.wait_idle(Duration::from_millis(30), Duration::from_millis(5))
            .await;
    }
}
