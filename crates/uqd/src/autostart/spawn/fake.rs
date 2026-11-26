use super::*;
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// What the fake was asked to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnCall {
    pub unit: String,
    pub tag: String,
    /// The app's own argv.
    pub argv: Vec<String>,
    /// The whole command the real spawner would run ([`scope_argv`]).
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

/// Records calls. Exits are scripted by journal tag; an unscripted run never ends.
pub struct FakeSpawner {
    pub systemd_run: PathBuf,
    pub systemd_cat: PathBuf,
    pub calls: Mutex<Vec<SpawnCall>>,
    pub exits: Mutex<HashMap<String, ExitInfo>>,
    /// The `(pid, unit)` pairs `alive` answers true for.
    pub live: Mutex<BTreeSet<(u32, String)>>,
    /// `(pid, unit)` of every `alive` call.
    pub alive_calls: Mutex<Vec<(u32, String)>>,
    pub journal: Mutex<Vec<String>>,
    /// Journal reads fail (`None`) while this is set.
    pub journal_fails: AtomicBool,
    pub journal_delay: Mutex<Option<Duration>>,
    /// `(unit, tag, since)` of every journal read.
    pub tails: Mutex<Vec<(String, String, i64)>>,
    pub tails_now: AtomicUsize,
    pub tails_max: AtomicUsize,
    /// Exits that are used once each, in order, before `exits`.
    pub exit_queue: Mutex<HashMap<String, std::collections::VecDeque<ExitInfo>>>,
    /// Every scope `stop` was asked about.
    pub stops: Mutex<Vec<String>>,
    stoppers: Mutex<HashMap<String, std::sync::Arc<tokio::sync::Notify>>>,
    next_pid: AtomicU32,
}

impl Default for FakeSpawner {
    fn default() -> Self {
        FakeSpawner {
            systemd_run: PathBuf::from("/fake/bin/systemd-run"),
            systemd_cat: PathBuf::from("/fake/bin/systemd-cat"),
            calls: Mutex::default(),
            exits: Mutex::default(),
            live: Mutex::default(),
            alive_calls: Mutex::default(),
            journal: Mutex::default(),
            journal_fails: AtomicBool::new(false),
            journal_delay: Mutex::default(),
            tails: Mutex::default(),
            tails_now: AtomicUsize::new(0),
            tails_max: AtomicUsize::new(0),
            exit_queue: Mutex::default(),
            stops: Mutex::default(),
            stoppers: Mutex::default(),
            next_pid: AtomicU32::new(1000),
        }
    }
}

impl FakeSpawner {
    pub fn calls(&self) -> Vec<SpawnCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn exit(&self, tag: &str, e: ExitInfo) {
        self.exits.lock().unwrap().insert(tag.to_string(), e);
    }

    /// The next run with `tag` ends this way (then the ones after follow `exit`).
    pub fn exit_once(&self, tag: &str, e: ExitInfo) {
        self.exit_queue
            .lock()
            .unwrap()
            .entry(tag.to_string())
            .or_default()
            .push_back(e);
    }

    /// Make `pid` alive in `unit`'s scope.
    pub fn set_alive(&self, pid: u32, unit: &str) {
        self.live.lock().unwrap().insert((pid, unit.to_string()));
    }
}

#[async_trait]
impl Spawner for FakeSpawner {
    async fn spawn(
        &self,
        unit: &str,
        tag: &str,
        argv: &[String],
        env: &[(String, String)],
        cwd: &Path,
    ) -> std::io::Result<Child> {
        self.calls.lock().unwrap().push(SpawnCall {
            unit: unit.into(),
            tag: tag.into(),
            argv: argv.to_vec(),
            command: scope_argv(&self.systemd_run, &self.systemd_cat, unit, tag, argv),
            env: env.to_vec(),
            cwd: cwd.into(),
        });
        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);
        let queued = self
            .exit_queue
            .lock()
            .unwrap()
            .get_mut(tag)
            .and_then(|q| q.pop_front());
        let exit = queued.or_else(|| self.exits.lock().unwrap().get(tag).copied());
        let stopper = std::sync::Arc::new(tokio::sync::Notify::new());
        self.stoppers
            .lock()
            .unwrap()
            .insert(unit.to_string(), stopper.clone());
        Ok(Child {
            pid,
            wait: Box::pin(async move {
                match exit {
                    Some(e) => e,
                    None => {
                        stopper.notified().await;
                        ExitInfo::Signal(15)
                    }
                }
            }),
        })
    }

    async fn stop(&self, unit: &str) -> bool {
        self.stops.lock().unwrap().push(unit.to_string());
        match self.stoppers.lock().unwrap().get(unit) {
            Some(n) => {
                n.notify_one();
                true
            }
            None => false,
        }
    }

    async fn alive(&self, pid: u32, unit: &str) -> bool {
        let key = (pid, unit.to_string());
        self.alive_calls.lock().unwrap().push(key.clone());
        self.live.lock().unwrap().contains(&key)
    }

    async fn journal_tail(
        &self,
        unit: &str,
        tag: &str,
        since: i64,
        lines: usize,
    ) -> Option<Vec<String>> {
        self.tails
            .lock()
            .unwrap()
            .push((unit.into(), tag.into(), since));
        let now = self.tails_now.fetch_add(1, Ordering::SeqCst) + 1;
        self.tails_max.fetch_max(now, Ordering::SeqCst);
        let delay = *self.journal_delay.lock().unwrap();
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        self.tails_now.fetch_sub(1, Ordering::SeqCst);
        if self.journal_fails.load(Ordering::SeqCst) {
            return None;
        }
        let all = self.journal.lock().unwrap().clone();
        Some(all[all.len().saturating_sub(lines)..].to_vec())
    }
}
