//! Processor and memory use of the windows' processes, for the Open windows island's "By system
//! usage" order: `ProcessUsage([pid, …])` → `{"<pid>": {"cpu": %, "rss": bytes}}`. Processor use
//! is over the time since the previous call (0 on the first); memory is the resident set. Only
//! /proc is read; at most 64 pids per call; a pid that is gone is left out.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

pub const MAX_PIDS: usize = 64;
/// Clock ticks per second of /proc/<pid>/stat times (USER_HZ, 100 on Linux).
const HZ: f64 = 100.0;

pub struct ProcUsage {
    root: PathBuf,
    last: Mutex<HashMap<u32, (u64, Instant)>>,
}

/// utime + stime (ticks) from /proc/<pid>/stat; the command name may hold spaces and ")".
pub fn parse_ticks(stat: &str) -> Option<u64> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    // After the name: state(0) ppid(1) … utime(11) stime(12).
    Some(f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?)
}

/// VmRSS from /proc/<pid>/status, in bytes.
pub fn parse_rss(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

impl ProcUsage {
    pub fn new() -> ProcUsage {
        ProcUsage::with_root("/proc")
    }

    pub fn with_root(root: impl Into<PathBuf>) -> ProcUsage {
        ProcUsage {
            root: root.into(),
            last: Mutex::new(HashMap::new()),
        }
    }

    fn read(&self, pid: u32, file: &str) -> Option<String> {
        std::fs::read_to_string(Path::new(&self.root).join(pid.to_string()).join(file)).ok()
    }

    pub fn usage(&self, pids: &[u32], now: Instant) -> Value {
        let mut out = Map::new();
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let mut seen = HashMap::new();
        for &pid in pids.iter().take(MAX_PIDS) {
            let Some(ticks) = self.read(pid, "stat").as_deref().and_then(parse_ticks) else {
                continue;
            };
            let rss = self
                .read(pid, "status")
                .as_deref()
                .and_then(parse_rss)
                .unwrap_or(0);
            let cpu = match last.get(&pid) {
                Some(&(t0, at)) if now > at && ticks >= t0 => {
                    let secs = now.duration_since(at).as_secs_f64();
                    ((ticks - t0) as f64 / HZ / secs * 1000.0).round() / 10.0
                }
                _ => 0.0,
            };
            seen.insert(pid, (ticks, now));
            out.insert(pid.to_string(), json!({"cpu": cpu, "rss": rss}));
        }
        *last = seen;
        Value::Object(out)
    }
}

/// The `ProcessUsage` call's argument: an array of positive pids (others are dropped).
pub fn pids_arg(args: &[Value]) -> Result<Vec<u32>, String> {
    match args {
        [Value::Array(a)] => Ok(a
            .iter()
            .filter_map(|v| v.as_u64())
            .filter(|&p| p > 0 && p <= u64::from(u32::MAX))
            .map(|p| p as u32)
            .take(MAX_PIDS)
            .collect()),
        _ => Err("ProcessUsage takes one array of process ids".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const STAT: &str =
        "1234 (my (odd) app) S 1 1234 1234 0 -1 4194304 100 0 0 0 250 50 0 0 20 0 4 0 100 1000 200";

    #[test]
    fn parses_times_and_memory() {
        assert_eq!(parse_ticks(STAT), Some(300));
        assert_eq!(parse_ticks("garbage"), None);
        assert_eq!(
            parse_rss("Name:\tx\nVmRSS:\t  2048 kB\n"),
            Some(2048 * 1024)
        );
        assert_eq!(parse_rss("Name:\tx\n"), None);
    }

    #[test]
    fn cpu_from_the_change_since_the_last_call() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("1234");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("stat"), STAT).unwrap();
        std::fs::write(p.join("status"), "VmRSS:\t1000 kB\n").unwrap();
        let u = ProcUsage::with_root(d.path());
        let t0 = Instant::now();
        let first = u.usage(&[1234, 99], t0);
        assert_eq!(
            first,
            json!({"1234": {"cpu": 0.0, "rss": 1024000}}),
            "gone pids are left out"
        );
        // 50 more ticks (0.5 s of processor) over 2 s: 25%.
        std::fs::write(p.join("stat"), STAT.replace(" 250 50 ", " 300 50 ")).unwrap();
        let second = u.usage(&[1234], t0 + Duration::from_secs(2));
        assert_eq!(second["1234"]["cpu"], 25.0);
    }

    #[test]
    fn pids_arg_keeps_real_pids() {
        assert_eq!(
            pids_arg(&[json!([1, 0, -3, "x", 42])]).unwrap(),
            vec![1, 42]
        );
        assert!(pids_arg(&[]).is_err());
        assert!(pids_arg(&[json!(1)]).is_err());
        let many: Vec<u32> = (1..200).collect();
        assert_eq!(pids_arg(&[json!(many)]).unwrap().len(), MAX_PIDS);
    }
}
