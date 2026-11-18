//! `uqctl clip`: talk to `uq-clipd`, the clipboard history (see `uq-clip`).

use clap::Subcommand;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use uq_clip::config::socket_path;
use uq_clip::ipc::{Request, call, entries_from};
use uq_clip::store::{Entry, Kind};

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
    /// Stop recording until `resume` (or until uq-clipd restarts).
    Pause,
    Resume,
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Read clipboard.toml again.
    Reload,
    /// Print one JSON line now and after every change, until interrupted.
    Watch,
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
                let now = uq_clip::daemon::now();
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
                UnixStream::connect(&sock).map_err(|e| format!("uq-clipd isn't running ({e})"))?;
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
