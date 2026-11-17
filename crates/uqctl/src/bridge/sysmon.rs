//! The system monitor island: CPU (overall and per core) from `/proc/stat` deltas, memory from
//! `/proc/meminfo`, and a CPU temperature from hwmon, every 2 s and only while `sysmon` is
//! placed. Nothing leaves the machine.

use super::feeds::placed;
use super::protocol::Outbound;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::Settings;

pub const EVERY: Duration = Duration::from_secs(2);
/// Wait between the first two samples, so the island shows numbers quickly.
const FIRST: Duration = Duration::from_millis(250);
/// hwmon drivers for the CPU package temperature, best first.
const SENSORS: [&str; 5] = ["k10temp", "coretemp", "zenpower", "cpu_thermal", "acpitz"];

/// The graphics card's reading: use, video memory and temperature.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gpu {
    pub name: String,
    /// Percent, 0 to 100.
    pub busy: f64,
    pub vram_used: u64,
    pub vram_total: u64,
    pub temp_c: Option<f64>,
}

impl Gpu {
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "busy": self.busy,
            "vram_used": self.vram_used,
            "vram_total": self.vram_total,
            "temp_c": self.temp_c,
        })
    }
}

/// Where the NVIDIA tool's answer must arrive by; a wedged driver never holds the island.
const NVIDIA_TIMEOUT: Duration = Duration::from_millis(1500);

/// One line of `nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,
/// temperature.gpu --format=csv,noheader,nounits` (MiB for memory; `[N/A]` where a card says
/// nothing). The busiest of several cards is the one shown.
pub fn parse_nvidia(text: &str) -> Option<Gpu> {
    let mut best: Option<Gpu> = None;
    for line in text.lines() {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < 5 || f[0].is_empty() {
            continue;
        }
        let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0);
        let Some(busy) = num(f[1]) else { continue };
        let mib = |s: &str| num(s).map_or(0, |v| (v * 1024.0 * 1024.0) as u64);
        let g = Gpu {
            name: f[0].chars().take(48).collect(),
            busy: round1(busy.min(100.0)),
            vram_used: mib(f[2]),
            vram_total: mib(f[3]),
            temp_c: num(f[4]).map(round1),
        };
        if best.as_ref().is_none_or(|b| g.busy > b.busy) {
            best = Some(g);
        }
    }
    best
}

/// An AMD card's reading from `<sys>/class/drm/card*/device` (`gpu_busy_percent`,
/// `mem_info_vram_*`, hwmon `temp1_input`). Other makers expose no such files.
pub fn read_amd(sys: &Path) -> Option<Gpu> {
    let mut cards: Vec<PathBuf> = std::fs::read_dir(sys.join("class/drm"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.strip_prefix("card")
                    .is_some_and(|d| d.parse::<u32>().is_ok())
            })
        })
        .collect();
    cards.sort();
    let read = |p: PathBuf| std::fs::read_to_string(p).ok();
    for card in cards {
        let dev = card.join("device");
        let Some(busy) =
            read(dev.join("gpu_busy_percent")).and_then(|t| t.trim().parse::<f64>().ok())
        else {
            continue;
        };
        let bytes = |n: &str| read(dev.join(n)).and_then(|t| t.trim().parse::<u64>().ok());
        let temp = std::fs::read_dir(dev.join("hwmon"))
            .into_iter()
            .flatten()
            .flatten()
            .find_map(|h| read_temp(&h.path().join("temp1_input")));
        let name = read(dev.join("product_name"))
            .map(|t| t.trim().chars().take(48).collect::<String>())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "AMD graphics".into());
        return Some(Gpu {
            name,
            busy: round1(busy.clamp(0.0, 100.0)),
            vram_used: bytes("mem_info_vram_used").unwrap_or(0),
            vram_total: bytes("mem_info_vram_total").unwrap_or(0),
            temp_c: temp,
        });
    }
    None
}

/// Whether to ask `nvidia-smi` at all: the NVIDIA driver is loaded.
fn has_nvidia(proc_root: &Path) -> bool {
    proc_root.join("driver/nvidia").exists()
}

async fn read_nvidia() -> Option<Gpu> {
    let run = tokio::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(NVIDIA_TIMEOUT, run).await.ok()?.ok()?;
    out.status
        .success()
        .then(|| parse_nvidia(&String::from_utf8_lossy(&out.stdout)))
        .flatten()
}

/// Busy and total jiffies of one `cpu` line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Times {
    pub busy: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stat {
    pub all: Times,
    pub cores: Vec<Times>,
}

pub fn parse_stat(text: &str) -> Option<Stat> {
    let mut stat = Stat::default();
    let mut seen_all = false;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else { continue };
        if !label.starts_with("cpu") {
            continue;
        }
        // user nice system idle iowait irq softirq steal (guest time is inside user/nice).
        let v: Vec<u64> = parts.take(8).filter_map(|p| p.parse().ok()).collect();
        if v.len() < 4 {
            continue;
        }
        let total: u64 = v.iter().sum();
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        let t = Times {
            busy: total.saturating_sub(idle),
            total,
        };
        if label == "cpu" {
            stat.all = t;
            seen_all = true;
        } else {
            stat.cores.push(t);
        }
    }
    seen_all.then_some(stat)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn busy_pct(a: Times, b: Times) -> f64 {
    let dt = b.total.saturating_sub(a.total);
    if dt == 0 {
        return 0.0;
    }
    let db = b.busy.saturating_sub(a.busy).min(dt);
    round1(db as f64 / dt as f64 * 100.0)
}

/// (MemTotal, MemAvailable) in bytes.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        text.lines().find_map(|l| {
            let rest = l.strip_prefix(name)?.strip_prefix(':')?;
            let kib: u64 = rest.split_whitespace().next()?.parse().ok()?;
            Some(kib * 1024)
        })
    };
    Some((field("MemTotal")?, field("MemAvailable")?))
}

/// The `temp*_input` of the best known CPU sensor under `<sys>/class/hwmon`.
pub fn find_sensor(sys: &Path) -> Option<PathBuf> {
    let mut found: Vec<(usize, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(sys.join("class/hwmon")).ok()?.flatten() {
        let dir = entry.path();
        let Ok(name) = std::fs::read_to_string(dir.join("name")) else {
            continue;
        };
        let Some(rank) = SENSORS.iter().position(|s| *s == name.trim()) else {
            continue;
        };
        let mut inputs: Vec<(u32, PathBuf)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                let num = n
                    .strip_prefix("temp")?
                    .strip_suffix("_input")?
                    .parse()
                    .ok()?;
                Some((num, e.path()))
            })
            .collect();
        inputs.sort();
        if let Some((_, p)) = inputs.into_iter().next() {
            found.push((rank, p));
        }
    }
    found.sort();
    found.into_iter().next().map(|(_, p)| p)
}

fn read_temp(p: &Path) -> Option<f64> {
    let milli: i64 = std::fs::read_to_string(p).ok()?.trim().parse().ok()?;
    Some(round1(milli as f64 / 1000.0))
}

/// Reads one sample set and turns two of them into the `SysMon` payload.
pub struct SysMon {
    proc_root: PathBuf,
    sys_root: PathBuf,
}

impl SysMon {
    pub fn new() -> SysMon {
        SysMon::with_roots("/proc", "/sys")
    }

    pub fn with_roots(proc_root: impl Into<PathBuf>, sys_root: impl Into<PathBuf>) -> SysMon {
        SysMon {
            proc_root: proc_root.into(),
            sys_root: sys_root.into(),
        }
    }

    pub fn stat(&self) -> Option<Stat> {
        parse_stat(&std::fs::read_to_string(self.proc_root.join("stat")).ok()?)
    }

    /// The payload from the previous and current CPU samples.
    pub fn payload(
        &self,
        prev: &Stat,
        cur: &Stat,
        sensor: Option<&Path>,
        gpu: Option<&Gpu>,
    ) -> Value {
        let cores: Vec<f64> = prev
            .cores
            .iter()
            .zip(&cur.cores)
            .map(|(a, b)| busy_pct(*a, *b))
            .collect();
        let mem = std::fs::read_to_string(self.proc_root.join("meminfo"))
            .ok()
            .and_then(|t| parse_meminfo(&t));
        let (mem_pct, used, total) = match mem {
            Some((total, avail)) if total > 0 => {
                let used = total.saturating_sub(avail);
                (
                    json!(round1(used as f64 / total as f64 * 100.0)),
                    json!(used),
                    json!(total),
                )
            }
            _ => (Value::Null, Value::Null, Value::Null),
        };
        json!({
            "cpu": busy_pct(prev.all, cur.all),
            "cores": cores,
            "mem": mem_pct,
            "mem_used": used,
            "mem_total": total,
            "temp_c": sensor.and_then(read_temp),
            "gpu": gpu.map_or(Value::Null, Gpu::to_json),
        })
    }

    /// The graphics card, when there is one this island can read.
    pub async fn gpu(&self) -> Option<Gpu> {
        if has_nvidia(&self.proc_root) {
            return read_nvidia().await;
        }
        read_amd(&self.sys_root)
    }

    /// Every `every` while `sysmon` is placed; removing it emits `null` once.
    pub async fn run(
        self,
        tx: mpsc::Sender<Outbound>,
        mut rx: watch::Receiver<Option<Settings>>,
        stop: CancellationToken,
        every: Duration,
    ) {
        let emit = |data: Value| {
            let tx = tx.clone();
            let stop = stop.clone();
            async move {
                let out = Outbound::Event {
                    event: "SysMon".into(),
                    data,
                };
                tokio::select! { _ = stop.cancelled() => false, r = tx.send(out) => r.is_ok() }
            }
        };
        let mut on = false;
        let mut prev: Option<Stat> = None;
        let mut sensor: Option<PathBuf> = None;
        let mut next = tokio::time::Instant::now();
        loop {
            let want = rx
                .borrow_and_update()
                .as_ref()
                .is_some_and(|s| placed(s, "sysmon"));
            let want_gpu = want
                && rx
                    .borrow()
                    .as_ref()
                    .and_then(|s| s.shell.bar.as_ref())
                    .is_none_or(|b| b.options.sysmon.gpu);
            if want != on {
                on = want;
                if on {
                    sensor = find_sensor(&self.sys_root);
                    prev = self.stat();
                    next = tokio::time::Instant::now() + FIRST.min(every);
                } else {
                    prev = None;
                    if !emit(Value::Null).await {
                        return;
                    }
                }
            }
            tokio::select! {
                _ = stop.cancelled() => return,
                r = rx.changed() => if r.is_err() { return },
                _ = tokio::time::sleep_until(next), if on => {
                    next = tokio::time::Instant::now() + every;
                    let cur = self.stat();
                    let gpu = if want_gpu { self.gpu().await } else { None };
                    if let (Some(a), Some(b)) = (&prev, &cur)
                        && !emit(self.payload(a, b, sensor.as_deref(), gpu.as_ref())).await
                    {
                        return;
                    }
                    prev = cur;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uq_core::settings::BarItem;

    const STAT1: &str = "cpu  100 0 100 800 0 0 0 0 0 0\n\
        cpu0 50 0 50 400 0 0 0 0 0 0\n\
        cpu1 50 0 50 400 0 0 0 0 0 0\n\
        intr 1 2 3\nctxt 5\n";
    // +200 total: all: busy +100 (50%); cpu0 busy +90 of 100; cpu1 busy +10 of 100 (iowait idle).
    const STAT2: &str = "cpu  180 10 110 880 20 0 0 0 0 0\n\
        cpu0 120 10 60 410 0 0 0 0 0 0\n\
        cpu1 60 0 50 470 20 0 0 0 0 0\n";

    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        std::fs::create_dir_all(p.join("proc")).unwrap();
        std::fs::write(p.join("proc/stat"), STAT1).unwrap();
        std::fs::write(
            p.join("proc/meminfo"),
            "MemTotal:       16000000 kB\nMemFree:  1000 kB\nMemAvailable:    4000000 kB\n",
        )
        .unwrap();
        for (dir, name, temps) in [
            ("hwmon0", "nvme", &[("temp1_input", "45000")][..]),
            ("hwmon1", "acpitz", &[("temp1_input", "30000")][..]),
            (
                "hwmon2",
                "k10temp",
                &[("temp3_input", "70000"), ("temp1_input", "52375")][..],
            ),
        ] {
            let d = p.join("sys/class/hwmon").join(dir);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("name"), format!("{name}\n")).unwrap();
            for (f, v) in temps {
                std::fs::write(d.join(f), v).unwrap();
            }
        }
        d
    }

    #[test]
    fn deltas_memory_and_sensor() {
        let d = fixture();
        let m = SysMon::with_roots(d.path().join("proc"), d.path().join("sys"));
        let a = m.stat().unwrap();
        let b = parse_stat(STAT2).unwrap();
        assert_eq!(a.cores.len(), 2);
        let sensor = find_sensor(&d.path().join("sys"));
        assert!(sensor.as_ref().unwrap().ends_with("hwmon2/temp1_input"));
        let v = m.payload(&a, &b, sensor.as_deref(), None);
        assert_eq!(
            v,
            json!({"cpu": 50.0, "cores": [90.0, 10.0], "mem": 75.0,
                   "mem_used": 12_000_000u64 * 1024, "mem_total": 16_000_000u64 * 1024,
                   "temp_c": 52.4, "gpu": null})
        );
        // Identical samples: 0 %, never a division by zero.
        assert_eq!(m.payload(&a, &a, None, None)["cpu"], 0.0);
        assert!(m.payload(&a, &a, None, None)["temp_c"].is_null());
    }

    #[test]
    fn nvidia_line_and_amd_files() {
        let g = parse_nvidia(
            "NVIDIA GeForce RTX 4070, 37, 1024, 12282, 51\nNVIDIA T400, 90, [N/A], 4096, [N/A]\n",
        )
        .unwrap();
        assert_eq!(g.name, "NVIDIA T400", "the busiest card");
        assert_eq!(
            (g.busy, g.vram_used, g.vram_total, g.temp_c),
            (90.0, 0, 4096 << 20, None)
        );
        let g = parse_nvidia("RTX, 5, 2048, 8192, 40\n").unwrap();
        assert_eq!((g.vram_used, g.temp_c), (2048 << 20, Some(40.0)));
        assert_eq!(parse_nvidia(""), None);
        assert_eq!(parse_nvidia("No devices were found\n"), None);
        assert_eq!(parse_nvidia("x, nan, 1, 1, 1\n"), None);

        let d = tempfile::tempdir().unwrap();
        let dev = d.path().join("class/drm/card0/device");
        std::fs::create_dir_all(dev.join("hwmon/hwmon3")).unwrap();
        std::fs::write(dev.join("gpu_busy_percent"), "42\n").unwrap();
        std::fs::write(dev.join("mem_info_vram_used"), "1000\n").unwrap();
        std::fs::write(dev.join("mem_info_vram_total"), "4000\n").unwrap();
        std::fs::write(dev.join("hwmon/hwmon3/temp1_input"), "61000\n").unwrap();
        let g = read_amd(d.path()).unwrap();
        assert_eq!(
            (g.name.as_str(), g.busy, g.vram_used, g.vram_total, g.temp_c),
            ("AMD graphics", 42.0, 1000, 4000, Some(61.0))
        );
        assert_eq!(read_amd(&d.path().join("none")), None);
    }

    #[test]
    fn no_known_sensor_is_none() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path().join("class/hwmon/hwmon0");
        std::fs::create_dir_all(&h).unwrap();
        std::fs::write(h.join("name"), "amdgpu").unwrap();
        std::fs::write(h.join("temp1_input"), "60000").unwrap();
        assert_eq!(find_sensor(d.path()), None);
        assert_eq!(find_sensor(&d.path().join("missing")), None);
        assert_eq!(parse_stat("intr 1\n"), None);
        assert_eq!(parse_meminfo("MemTotal: 5 kB\n"), None);
    }

    #[tokio::test]
    async fn runs_only_while_placed() {
        let d = fixture();
        let m = SysMon::with_roots(d.path().join("proc"), d.path().join("sys"));
        let off = Some(Settings::default());
        let mut on = Settings::default();
        on.shell
            .bar
            .as_mut()
            .unwrap()
            .start
            .push(BarItem::new("sysmon"));
        let (stx, srx) = watch::channel(off.clone());
        let (tx, mut rx) = mpsc::channel(16);
        let stop = CancellationToken::new();
        let task = tokio::spawn(m.run(tx, srx, stop.clone(), Duration::from_millis(50)));
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(rx.try_recv().is_err());
        stx.send_replace(Some(on));
        let got = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let Outbound::Event { event, data } = got else {
            panic!()
        };
        assert_eq!(event, "SysMon");
        assert_eq!(data["mem"], 75.0);
        stx.send_replace(off);
        // Drain samples until the null.
        loop {
            let Outbound::Event { data, .. } =
                tokio::time::timeout(Duration::from_secs(2), rx.recv())
                    .await
                    .unwrap()
                    .unwrap()
            else {
                panic!()
            };
            if data.is_null() {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(rx.try_recv().is_err(), "stopped after removal");
        stop.cancel();
        task.await.unwrap();
    }
}
