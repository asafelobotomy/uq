//! The on-disk journal: one JSON line per change in `history-YYYY-MM-DD.jsonl` (local date).

use chrono::{Duration, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

pub const MAX_AGE_DAYS: i64 = 30;
pub const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// `"{time}-{seq:04}"`, unique within the process.
    pub id: String,
    /// Unix seconds.
    pub time: i64,
    pub origin: String,
    pub domain: String,
    pub summary: String,
    pub before: Value,
    pub after: Value,
}

static SEQ: AtomicU32 = AtomicU32::new(0);

impl Entry {
    pub fn new(
        time: i64,
        origin: impl Into<String>,
        domain: impl Into<String>,
        summary: impl Into<String>,
        before: Value,
        after: Value,
    ) -> Entry {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed) % 10_000;
        Entry {
            id: format!("{time}-{seq:04}"),
            time,
            origin: origin.into(),
            domain: domain.into(),
            summary: summary.into(),
            before,
            after,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Journal {
    dir: PathBuf,
}

fn local_date(secs: i64) -> NaiveDate {
    Local
        .timestamp_opt(secs, 0)
        .earliest()
        .unwrap_or_else(Local::now)
        .date_naive()
}

fn file_name(date: NaiveDate) -> String {
    format!("history-{}.jsonl", date.format("%Y-%m-%d"))
}

fn file_date(path: &Path) -> Option<NaiveDate> {
    let name = path.file_name()?.to_str()?;
    let d = name.strip_prefix("history-")?.strip_suffix(".jsonl")?;
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

fn parse_lines(path: &Path) -> Vec<Entry> {
    fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

impl Journal {
    pub fn open(dir: impl Into<PathBuf>) -> Journal {
        Journal { dir: dir.into() }
    }

    /// Journal files, oldest first.
    fn files(&self) -> Vec<(NaiveDate, PathBuf)> {
        let mut v: Vec<_> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| file_date(&e.path()).map(|d| (d, e.path())))
            .collect();
        v.sort();
        v
    }

    pub fn append(&self, entries: &[Entry]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        for e in entries {
            let path = self.dir.join(file_name(local_date(e.time)));
            let mut line = serde_json::to_string(e).map_err(io::Error::other)?;
            line.push('\n');
            // A crash can leave a last line without its newline; start on a fresh line.
            if fs::read(&path).is_ok_and(|b| !b.is_empty() && !b.ends_with(b"\n")) {
                line.insert(0, '\n');
            }
            let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
            f.write_all(line.as_bytes())?;
            f.sync_all()?;
        }
        Ok(())
    }

    /// Newest first, at most `limit`; unparsable lines are skipped.
    pub fn list(&self, limit: usize) -> Vec<Entry> {
        let mut out = Vec::new();
        for (_, path) in self.files().into_iter().rev() {
            for e in parse_lines(&path).into_iter().rev() {
                if out.len() >= limit {
                    return out;
                }
                out.push(e);
            }
        }
        out
    }

    pub fn get(&self, id: &str) -> Option<Entry> {
        self.files()
            .into_iter()
            .rev()
            .find_map(|(_, p)| parse_lines(&p).into_iter().find(|e| e.id == id))
    }

    /// Delete whole files older than [`MAX_AGE_DAYS`], then drop the oldest entries until at
    /// most [`MAX_ENTRIES`] remain. Best effort: failures are logged.
    pub fn prune(&self, now: i64) {
        let cutoff = local_date(now) - Duration::days(MAX_AGE_DAYS);
        let mut kept = Vec::new();
        for (d, p) in self.files() {
            if d < cutoff {
                if let Err(e) = fs::remove_file(&p) {
                    tracing::warn!("history: removing {}: {e}", p.display());
                }
            } else {
                kept.push(p);
            }
        }
        let mut counts: Vec<usize> = kept.iter().map(|p| parse_lines(p).len()).collect();
        let mut excess = counts.iter().sum::<usize>().saturating_sub(MAX_ENTRIES);
        for (path, n) in kept.iter().zip(counts.iter_mut()) {
            if excess == 0 {
                break;
            }
            if *n <= excess {
                excess -= *n;
                if let Err(e) = fs::remove_file(path) {
                    tracing::warn!("history: removing {}: {e}", path.display());
                }
            } else {
                let rest: String = parse_lines(path)
                    .into_iter()
                    .skip(excess)
                    .filter_map(|e| serde_json::to_string(&e).ok())
                    .map(|l| l + "\n")
                    .collect();
                if let Err(e) = crate::atomic::atomic_write(path, &rest) {
                    tracing::warn!("history: rewriting {}: {e}", path.display());
                }
                excess = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DAY: i64 = 86_400;
    const NOW: i64 = 1_800_000_000;

    fn entry(time: i64, summary: &str) -> Entry {
        Entry::new(
            time,
            "settings",
            "look",
            summary,
            json!({}),
            json!({"x": 1}),
        )
    }

    #[test]
    fn journal_round_trip_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(dir.path());
        let es = [
            entry(NOW - 3 * DAY, "a"),
            entry(NOW - DAY, "b"),
            entry(NOW, "c"),
        ];
        j.append(&es[..2]).unwrap();
        j.append(&es[2..]).unwrap();
        let got = j.list(10);
        assert_eq!(
            got.iter().map(|e| e.summary.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
        assert_eq!(j.list(2).len(), 2);
        assert_eq!(j.get(&es[1].id), Some(es[1].clone()));
        assert_eq!(j.get("nope"), None);
        let ids: std::collections::HashSet<_> = es.iter().map(|e| &e.id).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn journal_skips_truncated_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(dir.path());
        let first = entry(NOW, "ok");
        j.append(std::slice::from_ref(&first)).unwrap();
        let path = dir.path().join(file_name(local_date(NOW)));
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"id\":\"1-0000\",\"ti").unwrap();
        drop(f);
        assert_eq!(j.list(10), std::slice::from_ref(&first));
        // The next append starts on its own line and is readable.
        let second = entry(NOW + 1, "next");
        j.append(std::slice::from_ref(&second)).unwrap();
        assert_eq!(j.list(10), [second, first]);
    }

    #[test]
    fn prune_drops_files_older_than_30_days() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(dir.path());
        j.append(&[
            entry(NOW - 45 * DAY, "old"),
            entry(NOW - 29 * DAY, "recent"),
        ])
        .unwrap();
        j.prune(NOW);
        let got = j.list(10);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].summary, "recent");
        assert_eq!(j.files().len(), 1);
    }

    #[test]
    fn prune_keeps_at_most_500() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(dir.path());
        let es: Vec<_> = (0..620)
            .map(|i| entry(NOW - 10 * DAY + i / 100 * DAY + i % 100, &i.to_string()))
            .collect();
        j.append(&es).unwrap();
        j.prune(NOW);
        let got = j.list(1000);
        assert_eq!(got.len(), MAX_ENTRIES);
        assert_eq!(got[0].summary, "619");
        assert_eq!(got.last().unwrap().summary, "120");
    }
}
