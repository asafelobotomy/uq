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
mod tests;
