//! The About page (spec §2.6): the computer's name (editable through hostnamed, which asks
//! polkit), the system, the hardware, and the versions of uQ and what it runs on. Everything
//! but the name is read from files (`os-release`, `/proc`) and a few `--version` commands, with
//! a short timeout each; nothing here needs uqd.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::proxies::{HostnameProxy, fresh};
use super::runner::PageLogic;
use super::sysbus::{self, HOSTNAME, SystemBus, failure, settle};

/// How long a `--version` command may run.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
/// The most of a command's output read.
const MAX_OUTPUT: usize = 64 * 1024;

/// Runs a program with arguments and returns its standard output, or `None`.
pub type Runner = fn(&str, &[&str]) -> Option<String>;

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetHostname { name: String },
}

pub struct AboutState {
    system: SystemBus,
    /// `/`, or a test directory holding `etc/os-release` and `proc/…`.
    root: PathBuf,
    run: Runner,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

/// Run `program` (looked up in `PATH`) with a timeout; its standard output when it exits 0.
pub fn run_command(program: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = Vec::new();
                child
                    .stdout
                    .take()?
                    .take(MAX_OUTPUT as u64)
                    .read_to_end(&mut out)
                    .ok()?;
                return status
                    .success()
                    .then(|| String::from_utf8_lossy(&out).into_owned());
            }
            Ok(None) if start.elapsed() < COMMAND_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Text from files and other programs, for a label: no control characters, at most 200 chars.
fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

/// `KEY=value` pairs of an os-release file, with quotes removed.
pub fn os_release(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.trim().split_once('=')?;
            if k.starts_with('#') {
                return None;
            }
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(v);
            Some((k.to_string(), v.replace("\\\"", "\"")))
        })
        .collect()
}

/// The CPU model and the number of logical processors, from `/proc/cpuinfo`.
pub fn cpu(text: &str) -> (String, usize) {
    let mut model = String::new();
    let mut count = 0;
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        match k.trim() {
            "processor" => count += 1,
            "model name" | "Hardware" | "cpu model" if model.is_empty() => model = clean(v),
            _ => {}
        }
    }
    (model, count)
}

/// `MemTotal` from `/proc/meminfo`, as "15.5 GiB".
pub fn memory(text: &str) -> String {
    let kib: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or(0);
    if kib == 0 {
        return String::new();
    }
    let gib = kib as f64 / (1024.0 * 1024.0);
    format!("{gib:.1} GiB")
}

/// The graphics devices in `lspci -mm` output: "vendor device" for each display class.
pub fn gpus(lspci: &str) -> Vec<String> {
    lspci
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
            let (class, vendor, device) = (fields.first()?, fields.get(1)?, fields.get(2)?);
            matches!(
                *class,
                "VGA compatible controller" | "3D controller" | "Display controller"
            )
            .then(|| clean(&format!("{vendor} {device}")))
        })
        .collect()
}

/// The version in a `--version` line: the first word that starts with a digit (or `v` and a
/// digit), so "Hyprland 0.52.1 built from…" gives "0.52.1".
pub fn version_in(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(|w| w.strip_prefix('v').unwrap_or(w))
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()))
        .map(clean)
        .unwrap_or_default()
}

/// Why `name` can't be the computer's name, if it can't: DNS labels of letters, digits and
/// `-`, joined by dots, at most 64 characters.
pub fn hostname_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("Type a name.");
    }
    if name.len() > 64 {
        return Some("That name is too long (at most 64 characters).");
    }
    let ok = name.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });
    (!ok).then_some(
        "A computer name uses letters, digits and -, and a part can't start or end with -.",
    )
}

impl AboutState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(SystemBus::system(), PathBuf::from("/"), run_command)
    }

    pub fn with(system: SystemBus, root: PathBuf, run: Runner) -> Self {
        Self {
            system,
            root,
            run,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
        }
    }

    fn read(&self, rel: &str) -> String {
        read(&self.root, rel)
    }

    /// The name and what hostnamed knows about the machine; when it isn't there, the name
    /// from `/proc` and a "Not available" note.
    async fn hostname(&self) -> Value {
        let read = async {
            let conn = self.system.conn().await?;
            let h: HostnameProxy = fresh(&conn).await?;
            let name = match h.static_hostname().await {
                Ok(n) if !n.is_empty() => n,
                _ => h.hostname().await?,
            };
            let hardware = format!(
                "{} {}",
                h.hardware_vendor().await.unwrap_or_default(),
                h.hardware_model().await.unwrap_or_default()
            );
            zbus::Result::Ok(json!({
                "available": true,
                "name": clean(&name),
                "kernel": clean(&h.kernel_release().await.unwrap_or_default()),
                "hardware": clean(&hardware),
            }))
        };
        match sysbus::within(read).await {
            Ok(v) => v,
            Err(e) => {
                let mut v = sysbus::read_error(&e, &HOSTNAME);
                v["name"] = json!(clean(&self.read("proc/sys/kernel/hostname")));
                v
            }
        }
    }
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn os(root: &Path) -> Value {
    let mut text = read(root, "etc/os-release");
    if text.is_empty() {
        text = read(root, "usr/lib/os-release");
    }
    let pairs = os_release(&text);
    let get = |k: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| clean(v))
            .unwrap_or_default()
    };
    let name = [get("PRETTY_NAME"), get("NAME")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "Linux".to_string());
    let version = [get("VERSION_ID"), get("BUILD_ID")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    json!({"name": name, "version": version})
}

fn version(run: Runner, program: &str) -> String {
    run(program, &["--version"])
        .map(|out| version_in(&out))
        .unwrap_or_default()
}

/// Everything but the name: files under `root` and the commands `run` runs.
pub fn details(root: &Path, run: Runner) -> Value {
    let (cpu_model, cores) = cpu(&read(root, "proc/cpuinfo"));
    let gpus = run("lspci", &["-mm"])
        .map(|out| gpus(&out))
        .unwrap_or_default();
    json!({
        "os": os(root),
        "kernel": clean(&read(root, "proc/sys/kernel/osrelease")),
        "cpu": {"model": cpu_model, "cores": cores},
        "memory": memory(&read(root, "proc/meminfo")),
        "gpus": gpus,
        "versions": {
            "uq": env!("CARGO_PKG_VERSION"),
            "hyprland": version(run, "Hyprland"),
            "quickshell": version(run, "quickshell"),
        },
    })
}

impl PageLogic for AboutState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetHostname { name } => {
                let name = name.trim();
                if let Some(p) = hostname_problem(name) {
                    return Err(failure(p));
                }
                let conn = self.system.conn().await?;
                let h: HostnameProxy = fresh(&conn).await?;
                let r = h.set_static_hostname(name, true).await;
                let ok = r.is_ok();
                settle(r, &HOSTNAME)?;
                if ok {
                    let mut n = self.notice.lock().unwrap();
                    *n = (
                        n.0 + 1,
                        "The computer's name was changed. Some apps show the new name after you log in again.".into(),
                    );
                }
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let hostname = self.hostname().await;
        // Files and commands block; read them off the async threads.
        let root = self.root.clone();
        let run = self.run;
        let mut details = tokio::task::spawn_blocking(move || details(&root, run))
            .await
            .map_err(|e| failure(e.to_string()))?;
        if hostname["available"] == true && hostname["kernel"] != "" {
            details["kernel"] = hostname["kernel"].clone();
        }
        let (seq, text) = self.notice.lock().unwrap().clone();
        details["hostname"] = hostname;
        details["notice"] = json!({"seq": seq, "text": text});
        Ok(details)
    }
}

#[cfg(test)]
mod tests;
