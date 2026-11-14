//! Processor and memory use of the windows' processes, for the Open windows island's "By system
//! usage" order: `ProcessUsage([pid, …])` → `{"<pid>": {"cpu": %, "rss": bytes}}`. Processor use
//! is over the time since the previous call (0 on the first); memory is the resident set. Only
//! /proc is read (`stat` and `statm`); at most 64 pids per call; a pid that is gone is left out.

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

/// The resident set from /proc/<pid>/statm (`size resident shared …`, in pages), in bytes.
/// The same counters as `VmRSS` in /proc/<pid>/status (anon + file + shmem), from a file the
/// kernel formats in a fraction of the time.
pub fn parse_rss(statm: &str, page: u64) -> Option<u64> {
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * page)
}

/// The system's page size in bytes.
fn page_size() -> u64 {
    static PAGE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *PAGE.get_or_init(|| {
        // SAFETY: sysconf only reads a system constant.
        let n = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        u64::try_from(n).ok().filter(|&n| n > 0).unwrap_or(4096)
    })
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
            // Windows of one app share a process: read it once.
            if seen.contains_key(&pid) {
                continue;
            }
            let Some(ticks) = self.read(pid, "stat").as_deref().and_then(parse_ticks) else {
                continue;
            };
            let rss = self
                .read(pid, "statm")
                .as_deref()
                .and_then(|m| parse_rss(m, page_size()))
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
            parse_rss("12000 512 300 10 0 900 0\n", 4096),
            Some(512 * 4096)
        );
        assert_eq!(parse_rss("12000\n", 4096), None);
        assert_eq!(parse_rss("", 4096), None);
    }

    /// statm's resident pages and status's VmRSS are the same number (a quiet child process,
    /// so nothing allocates between the two reads).
    #[test]
    fn statm_matches_vmrss() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let pid = child.id();
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).unwrap();
        let _ = child.kill();
        let _ = child.wait();
        let vmrss_kb: u64 = status
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|n| n.parse().ok())
            .unwrap();
        assert_eq!(parse_rss(&statm, page_size()), Some(vmrss_kb * 1024));
    }

    #[test]
    fn cpu_from_the_change_since_the_last_call() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("1234");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("stat"), STAT).unwrap();
        std::fs::write(p.join("statm"), "5000 250 100 10 0 400 0\n").unwrap();
        let u = ProcUsage::with_root(d.path());
        let t0 = Instant::now();
        let first = u.usage(&[1234, 99, 1234], t0);
        assert_eq!(
            first,
            json!({"1234": {"cpu": 0.0, "rss": 250 * page_size()}}),
            "gone pids are left out; a repeated pid is one entry"
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
