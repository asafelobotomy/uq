//! Launch counts: how often each app is started, saved now and then.

use super::*;

/// Launch counts stop here per app.
pub(crate) const MAX_LAUNCH_COUNT: u32 = 10_000;

/// Distinct apps tracked; further new ids are ignored.
pub(crate) const MAX_LAUNCH_IDS: usize = 2048;

/// Launch counts hit the disk at most this often.
pub(crate) const LAUNCH_SAVE_INTERVAL: Duration = Duration::from_secs(30);

/// How long a failed launch-count write waits before it is tried again.
pub(crate) const LAUNCH_RETRY: Duration = Duration::from_secs(60);

/// Whether `<id>` (or `<id>.desktop`) exists under an `applications/` folder of `data_dirs`.
pub fn desktop_file_installed(id: &str, data_dirs: &[PathBuf]) -> bool {
    if !valid_pinned_id(id) {
        return false;
    }
    let file = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };
    data_dirs
        .iter()
        .any(|d| d.join("applications").join(&file).is_file())
}

/// `$XDG_DATA_HOME` (default `~/.local/share`) followed by `$XDG_DATA_DIRS`
/// (default `/usr/local/share:/usr/share`).
pub fn xdg_data_dirs() -> Vec<PathBuf> {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let mut dirs = Vec::new();
    if let Some(h) = var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(h));
    } else if let Some(h) = var("HOME") {
        dirs.push(PathBuf::from(h).join(".local/share"));
    }
    let rest = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(rest.split(':').filter(|p| !p.is_empty()).map(PathBuf::from));
    dirs
}

impl Desktop {
    pub fn get_launch_counts(&self) -> BTreeMap<String, u32> {
        self.state.launch_counts.clone()
    }

    /// How many times launch counts have been written (for throttle tests).
    pub fn launch_saves(&self) -> u32 {
        self.launch_saves
    }

    pub fn record_launch(&mut self, id: &str) {
        self.record_launch_at(id, Instant::now());
    }

    /// Count a launch. Memory and subscribers update at once; the disk write is throttled.
    pub fn record_launch_at(&mut self, id: &str, now: Instant) {
        if !self.state.settings.privacy.record_launches || !valid_pinned_id(id) {
            return;
        }
        let counts = &mut self.state.launch_counts;
        if !counts.contains_key(id) && counts.len() >= MAX_LAUNCH_IDS {
            return;
        }
        let n = counts.entry(id.to_string()).or_insert(0);
        *n = (*n + 1).min(MAX_LAUNCH_COUNT);
        self.launch_dirty = true;
        self.flush_launch_counts_at(now);
        let _ = self
            .events
            .send(Event::LaunchCountsChanged(self.state.launch_counts.clone()));
    }

    /// Write dirty launch counts if the last write is at least 30 s old (and a failed write at
    /// least 60 s). Called when `launch_flush_due` comes.
    pub fn flush_launch_counts_at(&mut self, now: Instant) {
        if self.launch_flush_due(now).is_none_or(|due| now < due) {
            return;
        }
        self.save_launch_counts(now);
    }

    /// When dirty launch counts may next be written (`now` if they may be now); `None` while
    /// there is nothing to write.
    pub fn launch_flush_due(&self, now: Instant) -> Option<Instant> {
        if !self.launch_dirty {
            return None;
        }
        let after_save = self.last_launch_save.map(|t| t + LAUNCH_SAVE_INTERVAL);
        Some(after_save.max(self.launch_retry_at).unwrap_or(now))
    }

    pub fn flush_launch_counts(&mut self) {
        self.flush_launch_counts_at(Instant::now());
    }

    /// Writes `state.toml` outside `persist` on purpose: launch counts are never journalled.
    pub(crate) fn save_launch_counts(&mut self, now: Instant) {
        match self.state.save(&self.paths.state_file) {
            Ok(()) => {
                self.launch_dirty = false;
                self.last_launch_save = Some(now);
                self.launch_retry_at = None;
                self.launch_saves += 1;
            }
            Err(e) => {
                // Stay dirty; tried again after LAUNCH_RETRY.
                tracing::warn!("[STATE-007] saving launch counts: {e}");
                self.launch_retry_at = Some(now + LAUNCH_RETRY);
            }
        }
    }

    pub fn clear_launch_counts(&mut self) {
        self.state.launch_counts.clear();
        self.save_launch_counts(Instant::now());
        let _ = self
            .events
            .send(Event::LaunchCountsChanged(BTreeMap::new()));
    }

    /// Drop counts for apps that are no longer installed. Run at startup.
    pub fn prune_launch_counts(&mut self, installed: &dyn Fn(&str) -> bool) {
        let before = self.state.launch_counts.len();
        self.state.launch_counts.retain(|id, _| installed(id));
        if self.state.launch_counts.len() != before {
            self.save_launch_counts(Instant::now());
            let _ = self
                .events
                .send(Event::LaunchCountsChanged(self.state.launch_counts.clone()));
        }
    }
}
