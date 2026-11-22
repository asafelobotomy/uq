//! `uqctl log`, `uqctl explain`, `uqctl crashes` and `uqctl report`: uQ's problem codes, its
//! journal and its crash reports from a terminal (docs/diagnostics.md). None of them need uqd.

use std::process::ExitCode;

use clap::Subcommand;
use uq_core::diag::crash::{self, Crash, Dirs};
use uq_core::diag::journal::{self, Entry, Query};
use uq_core::diag::redact::Redactor;
use uq_core::diag::report;
use uq_core::diag::{Registry, Severity, is_code};
use uq_core::paths::Paths;

#[derive(Debug, Clone, clap::Args)]
pub struct LogArgs {
    /// How far back: `30m`, `2h`, `today`, `2026-11-22 10:00` (default: since this boot).
    #[arg(long)]
    pub since: Option<String>,
    /// The least serious kind shown: error, warn, info or debug.
    #[arg(long, default_value = "warn")]
    pub min: String,
    /// Only one area (`HYPR`) or one code (`HYPR-003`).
    #[arg(long)]
    pub only: Option<String>,
    /// Only lines with a code (leave out Qt's and other libraries' messages).
    #[arg(long)]
    pub coded: bool,
    /// One problem per line with how often and when it was last seen, newest first.
    #[arg(long)]
    pub summary: bool,
    /// Print JSON (one array) instead of text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CrashesCmd {
    /// List them, newest first (the default).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Hide one from Settings › Problems (its files stay).
    Dismiss { id: String },
}

/// `30m` → `-30min` for journalctl; anything else as it is.
pub fn since_arg(s: &str) -> String {
    let t = s.trim();
    let (num, unit) = t.split_at(t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len()));
    if num.is_empty() {
        return t.to_string();
    }
    let unit = match unit {
        "" | "m" | "min" => "min",
        "s" => "s",
        "h" => "h",
        "d" => "d",
        _ => return t.to_string(),
    };
    format!("-{num}{unit}")
}

fn time(us: i64) -> String {
    report::iso(us / 1_000_000)
}

pub fn log_lines(entries: &[Entry], reg: &Registry, summary: bool) -> Vec<String> {
    entries
        .iter()
        .map(|e| {
            let code = e.code.as_deref().unwrap_or("-");
            let mut l = format!(
                "{}  {:<5}  {:<12} {:<6} {}",
                time(e.time_us),
                e.severity.as_str().to_ascii_uppercase(),
                code,
                e.source,
                e.message
            );
            if e.count > 1 {
                l.push_str(&format!("  (×{}", e.count));
                if summary {
                    l.push_str(&format!(", first {}", time(e.first_us)));
                }
                l.push(')');
            }
            if summary && let Some(c) = e.code.as_deref().and_then(|c| reg.get(c)) {
                l.push_str(&format!("\n{:>40}{} — {}", "", c.title, c.fix));
            }
            l
        })
        .collect()
}

pub fn log(a: LogArgs) -> ExitCode {
    let Some(min) = Severity::parse(&a.min) else {
        eprintln!("uqctl log: --min is error, warn, info or debug");
        return ExitCode::from(2);
    };
    let reg = Registry::installed();
    let q = Query {
        since: a.since.as_deref().map(since_arg),
        min,
        coded_only: a.coded,
        only: a.only.clone(),
        ..Query::default()
    };
    let entries = match journal::read(&q, &reg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("uqctl log: can't run journalctl: {e}");
            return ExitCode::FAILURE;
        }
    };
    let entries = if a.summary {
        journal::group(entries)
    } else {
        journal::collapse_runs(entries)
    };
    if a.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).unwrap_or_default()
        );
    } else if entries.is_empty() {
        println!("Nothing logged at {} or above.", min.as_str());
    } else {
        for l in log_lines(&entries, &reg, a.summary) {
            println!("{l}");
        }
        if !a.summary {
            println!(
                "\n`uqctl explain CODE` says what a code means; `uqctl log --summary` groups them."
            );
        }
    }
    ExitCode::SUCCESS
}

pub fn explain_text(reg: &Registry, code: &str) -> Option<String> {
    let c = reg.get(code)?;
    Some(format!(
        "{} ({}, from {}){}\n  {}\n\nWhat it means: {}\nWhat to do: {}",
        c.id,
        c.severity.as_str(),
        c.owner,
        if c.retired { ", no longer used" } else { "" },
        c.title,
        c.means,
        c.fix
    ))
}

pub fn explain(code: Option<String>) -> ExitCode {
    let reg = Registry::installed();
    match code {
        Some(c) => {
            // `uq[WIN-001]` or `[WIN-001]` pasted from a log line works too.
            let c = uq_core::diag::code_in(&c).map(str::to_string).unwrap_or(c);
            match explain_text(&reg, &c) {
                Some(t) => {
                    println!("{t}");
                    ExitCode::SUCCESS
                }
                None => {
                    if is_code(&c.to_ascii_uppercase()) {
                        eprintln!("uqctl explain: no code {c} (`uqctl explain` lists them all)");
                    } else {
                        eprintln!("uqctl explain: codes look like HYPR-003");
                    }
                    ExitCode::FAILURE
                }
            }
        }
        None => {
            let mut area = "";
            for c in reg.all().iter().filter(|c| !c.retired) {
                if c.area() != area {
                    area = c.area();
                    println!();
                }
                println!("{:<12} {:<5}  {}", c.id, c.severity.as_str(), c.title);
            }
            ExitCode::SUCCESS
        }
    }
}

pub fn crash_lines(all: &[Crash]) -> Vec<String> {
    all.iter()
        .map(|c| {
            let mut l = format!(
                "{}  {:<9}  {:<22} {} — {}",
                report::iso(c.time),
                c.code,
                c.id,
                c.title(),
                c.reason
            );
            if let Some(p) = c.place.first() {
                l.push_str(&format!(" in {p}"));
            }
            if c.dismissed {
                l.push_str("  (dismissed)");
            }
            l
        })
        .collect()
}

pub fn crashes(cmd: Option<CrashesCmd>) -> ExitCode {
    let dirs = Dirs::from_env();
    match cmd.unwrap_or(CrashesCmd::List { json: false }) {
        CrashesCmd::List { json } => {
            let all = crash::scan(&dirs);
            if json {
                println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
            } else if all.is_empty() {
                println!("No crashes recorded.");
            } else {
                crash_lines(&all).iter().for_each(|l| println!("{l}"));
                println!(
                    "\n`uqctl report --crash ID` (or `--crash latest`) makes a report to share."
                );
            }
            ExitCode::SUCCESS
        }
        CrashesCmd::Dismiss { id } => match crash::find(&dirs, &id) {
            Some(c) => match crash::dismiss(&c) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("uqctl crashes dismiss: {e}");
                    ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("uqctl crashes dismiss: no crash {id}");
                ExitCode::FAILURE
            }
        },
    }
}

pub fn report(crash_id: Option<String>, stdout: bool, no_redact: bool) -> ExitCode {
    let reg = Registry::installed();
    let paths = Paths::from_env();
    let redact = if no_redact {
        Redactor::default()
    } else {
        Redactor::from_env()
    };
    let found = match crash_id.as_deref() {
        Some(id) => match crash::find(&Dirs::from_env(), id) {
            Some(c) => Some(c),
            None => {
                eprintln!("uqctl report: no crash {id} (`uqctl crashes` lists them)");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let input = report::gather(found.as_ref(), &reg, &paths, &redact);
    let text = report::render(&input, &reg);
    if stdout {
        print!("{text}");
        return ExitCode::SUCCESS;
    }
    // An unredacted report is never the crash's saved one, which Settings copies.
    let saved = match (&found, no_redact) {
        (Some(c), false) => report::save_crash(c, &text),
        _ => report::save_now(&paths, input.made, &text),
    };
    match saved {
        Ok(p) => {
            println!("{}", p.display());
            if no_redact {
                eprintln!("This report has window titles and names in it: read it before sharing.");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("uqctl report: can't save the report: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_spans_become_journalctl_ones() {
        assert_eq!(since_arg("30m"), "-30min");
        assert_eq!(since_arg("2h"), "-2h");
        assert_eq!(since_arg("15"), "-15min");
        assert_eq!(since_arg("today"), "today");
        assert_eq!(since_arg("2026-11-22 10:00"), "2026-11-22 10:00");
    }

    #[test]
    fn explain_says_what_and_what_to_do() {
        let t = explain_text(Registry::builtin(), "hypr-001").unwrap();
        assert!(t.starts_with("HYPR-001 (error, from uQ)"), "{t}");
        assert!(t.contains("What to do:"));
        assert!(explain_text(Registry::builtin(), "NOPE-999").is_none());
    }
}
