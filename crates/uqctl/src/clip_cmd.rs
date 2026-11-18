//! `uqctl clip`: talk to `uqlip`, the clipboard history (see `uqlip`).

use clap::Subcommand;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use uqlip::config::{Config, config_path, socket_path};
use uqlip::ipc::{Request, call, entries_from};
use uqlip::store::{Entry, Kind};

#[derive(Subcommand)]
pub enum ClipCmd {
    /// List entries, pinned first, then newest first.
    List {
        /// Only entries containing this text.
        query: Option<String>,
        /// Only "text" or "image".
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Print an entry's text, or the file an image is kept in.
    Get {
        id: u64,
    },
    /// Put an entry back on the clipboard.
    Copy {
        id: u64,
    },
    /// Keep an entry through pruning and "clear".
    Pin {
        id: u64,
    },
    Unpin {
        id: u64,
    },
    Delete {
        id: u64,
    },
    /// Forget the history. Pinned entries stay unless --all.
    Clear {
        #[arg(long)]
        all: bool,
    },
    /// Stop recording until `resume` (or until uqlip restarts).
    Pause,
    Resume,
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Read uqlip.toml again.
    Reload,
    /// Show the settings (JSON), or change one. A running uqlip picks the change up at once.
    Config {
        #[command(subcommand)]
        cmd: Option<ConfigCmd>,
    },
    /// Print one JSON line now and after every change, until interrupted.
    Watch,
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// One of: enabled (true or false), max_items, max_entry_mb, max_text_kb.
    Set { key: String, value: String },
    /// Never keep copies from this app (its window class, as `hyprctl clients` shows it).
    IgnoreAdd { app: String },
    /// Keep copies from this app again.
    IgnoreRemove { app: String },
}

/// Apply one change to the settings. Pure, so it is tested without a file.
fn apply(cfg: &mut Config, cmd: &ConfigCmd) -> Result<(), String> {
    let num = |v: &str, lo: u64, hi: u64, what: &str| {
        v.parse::<u64>()
            .ok()
            .filter(|n| (lo..=hi).contains(n))
            .ok_or(format!("{what} must be a number from {lo} to {hi}"))
    };
    match cmd {
        ConfigCmd::Set { key, value } => match key.as_str() {
            "enabled" => {
                cfg.enabled = match value.as_str() {
                    "true" | "on" => true,
                    "false" | "off" => false,
                    _ => return Err("enabled must be true or false".into()),
                }
            }
            "max_items" => cfg.max_items = num(value, 1, 5000, "max_items")? as usize,
            "max_entry_mb" => cfg.max_entry_mb = num(value, 1, 256, "max_entry_mb")?,
            "max_text_kb" => cfg.privacy.max_text_bytes = num(value, 1, 65536, "max_text_kb")? * 1024,
            other => return Err(format!("unknown setting {other}")),
        },
        ConfigCmd::IgnoreAdd { app } => {
            let app = app.trim();
            if app.is_empty() || app.len() > 64 {
                return Err("the app name must be 1 to 64 characters".into());
            }
            if !cfg.privacy.ignore_apps.iter().any(|a| a.eq_ignore_ascii_case(app)) {
                cfg.privacy.ignore_apps.push(app.to_string());
            }
        }
        ConfigCmd::IgnoreRemove { app } => {
            cfg.privacy.ignore_apps.retain(|a| !a.eq_ignore_ascii_case(app.trim()));
        }
    }
    Ok(())
}

fn save_config(cfg: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn kind_of(s: &str) -> Result<Kind, String> {
    match s {
        "text" => Ok(Kind::Text),
        "image" => Ok(Kind::Image),
        other => Err(format!("kind must be text or image, not {other}")),
    }
}

fn age(now: u64, then: u64) -> String {
    let d = now.saturating_sub(then);
    match d {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", d / 60),
        3600..=86399 => format!("{}h", d / 3600),
        _ => format!("{}d", d / 86400),
    }
}

fn row(e: &Entry, now: u64) -> String {
    let what = match e.kind {
        Kind::Text => e.preview.clone(),
        Kind::Image => format!("[image, {} KiB]", e.bytes.div_ceil(1024)),
    };
    let pin = if e.pinned { "*" } else { " " };
    format!("{:>5} {pin} {:>4}  {what}", e.id, age(now, e.created))
}

fn run(cmd: ClipCmd) -> Result<(), String> {
    let sock = socket_path();
    let ok = |req: Request| call(&sock, &req).map(|_| ());
    match cmd {
        ClipCmd::List {
            query,
            kind,
            limit,
            json,
        } => {
            let kind = kind.as_deref().map(kind_of).transpose()?;
            let v = call(
                &sock,
                &Request::List {
                    query: query.unwrap_or_default(),
                    kind,
                    limit,
                },
            )?;
            if json {
                println!("{}", v["entries"]);
            } else {
                let now = uqlip::daemon::now();
                for e in entries_from(&v) {
                    println!("{}", row(&e, now));
                }
            }
            Ok(())
        }
        ClipCmd::Get { id } => {
            let v = call(&sock, &Request::Get { id })?;
            match (v["text"].as_str(), v["path"].as_str()) {
                (Some(t), _) => print!("{t}"),
                (_, Some(p)) => println!("{p}"),
                _ => {}
            }
            Ok(())
        }
        ClipCmd::Copy { id } => ok(Request::Copy { id }),
        ClipCmd::Pin { id } => ok(Request::Pin { id, pinned: true }),
        ClipCmd::Unpin { id } => ok(Request::Pin { id, pinned: false }),
        ClipCmd::Delete { id } => ok(Request::Delete { id }),
        ClipCmd::Clear { all } => {
            let v = call(&sock, &Request::Clear { keep_pinned: !all })?;
            println!("removed {}", v["removed"]);
            Ok(())
        }
        ClipCmd::Pause => ok(Request::Pause { paused: true }),
        ClipCmd::Resume => ok(Request::Pause { paused: false }),
        ClipCmd::Reload => ok(Request::Reload),
        ClipCmd::Config { cmd: None } => {
            let cfg = Config::load(&config_path());
            println!("{}", serde_json::to_string(&cfg).map_err(|e| e.to_string())?);
            Ok(())
        }
        ClipCmd::Config { cmd: Some(cmd) } => {
            let mut cfg = Config::load(&config_path());
            apply(&mut cfg, &cmd)?;
            save_config(&cfg)?;
            // A daemon that isn't running reads the file when it starts.
            let _ = call(&sock, &Request::Reload);
            Ok(())
        }
        ClipCmd::Status { json } => {
            let v = call(&sock, &Request::Status)?;
            if json {
                println!("{}", v["status"]);
            } else {
                let s = &v["status"];
                println!(
                    "{} entries ({} pinned), {}, watching through {}",
                    s["count"],
                    s["pinned"],
                    if s["paused"] == true {
                        "paused"
                    } else {
                        "recording"
                    },
                    s["protocol"].as_str().unwrap_or("?")
                );
            }
            Ok(())
        }
        ClipCmd::Watch => {
            let mut s =
                UnixStream::connect(&sock).map_err(|e| format!("uqlip isn't running ({e})"))?;
            s.write_all(b"{\"cmd\":\"watch\"}\n")
                .map_err(|e| e.to_string())?;
            let mut out = std::io::stdout();
            for line in BufReader::new(s).lines() {
                let line = line.map_err(|e| e.to_string())?;
                if line.contains("\"event\"") {
                    writeln!(out, "{line}")
                        .and_then(|_| out.flush())
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }
    }
}

pub fn main(cmd: ClipCmd) -> ExitCode {
    match run(cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("uqctl clip: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(k: &str, v: &str) -> ConfigCmd {
        ConfigCmd::Set { key: k.into(), value: v.into() }
    }

    #[test]
    fn set_checks_ranges_and_names() {
        let mut c = Config::default();
        apply(&mut c, &set("max_items", "50")).unwrap();
        apply(&mut c, &set("enabled", "off")).unwrap();
        apply(&mut c, &set("max_text_kb", "2")).unwrap();
        assert_eq!((c.max_items, c.enabled, c.privacy.max_text_bytes), (50, false, 2048));
        assert!(apply(&mut c, &set("max_items", "0")).is_err());
        assert!(apply(&mut c, &set("max_entry_mb", "999")).is_err());
        assert!(apply(&mut c, &set("enabled", "maybe")).is_err());
        assert!(apply(&mut c, &set("nope", "1")).is_err());
    }

    #[test]
    fn ignore_list_adds_once_and_removes_any_case() {
        let mut c = Config::default();
        let n = c.privacy.ignore_apps.len();
        apply(&mut c, &ConfigCmd::IgnoreAdd { app: "MyVault".into() }).unwrap();
        apply(&mut c, &ConfigCmd::IgnoreAdd { app: "myvault".into() }).unwrap();
        assert_eq!(c.privacy.ignore_apps.len(), n + 1);
        apply(&mut c, &ConfigCmd::IgnoreRemove { app: "MYVAULT".into() }).unwrap();
        assert_eq!(c.privacy.ignore_apps.len(), n);
        assert!(apply(&mut c, &ConfigCmd::IgnoreAdd { app: " ".into() }).is_err());
    }
}
