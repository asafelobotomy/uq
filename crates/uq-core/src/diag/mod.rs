//! uQ's problem codes, logs and crash reports (docs/diagnostics.md).
//!
//! Every warning or error uQ logs starts with a code in brackets, `[AREA-NNN]`, so one line in
//! the journal can be looked up (`uqctl explain`), filtered (`uqctl log --area HYPR`) and shown in
//! plain words on the Settings Problems page. The codes are listed once, in `codes.toml`; other
//! apps add their own in `/usr/share/uq/diag/<app>.toml` with their own prefix (`BINDLE-001`).
//!
//! - [`log`]: the tracing setup every uQ program uses, and the panic hook that saves a crash
//!   (the same file apps vendor from `native/uq-diag`).
//! - [`journal`]: reading uQ's lines back from the journal.
//! - [`crash`]: finding crashes (Quickshell's own reports and uQ's panics).
//! - [`report`]: the Markdown report a person can paste into an issue.
//! - [`redact`]: what is taken out of a report before it leaves the computer.

pub mod crash;
pub mod journal;
/// The shared file other apps vendor (`native/uq-diag/uq_diag.rs`), used here as it is.
#[path = "../../../../native/uq-diag/uq_diag.rs"]
pub mod log;
pub mod redact;
pub mod report;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

/// uQ's own codes, compiled in so a broken install can still explain itself.
pub const UQ_CODES: &str = include_str!("codes.toml");

/// Where apps install their own code lists (`<app>.toml`, same layout as `codes.toml`).
pub const EXTRA_DIR: &str = crate::uq_consts::install::DIAG_DIR;

/// How serious a problem is. Ordered: `Debug < Info < Warn < Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Debug,
    Info,
    Warn,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
        }
    }

    /// `error`, `warn`/`warning`, `info`, `debug`; any case.
    pub fn parse(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "error" | "err" | "fatal" | "critical" | "crit" => Some(Severity::Error),
            "warn" | "warning" => Some(Severity::Warn),
            "info" | "notice" => Some(Severity::Info),
            "debug" | "trace" => Some(Severity::Debug),
            _ => None,
        }
    }

    /// The syslog priority journald stores for it.
    pub fn priority(self) -> u8 {
        match self {
            Severity::Error => 3,
            Severity::Warn => 4,
            Severity::Info => 6,
            Severity::Debug => 7,
        }
    }

    /// From a syslog priority (0–7).
    pub fn from_priority(p: u8) -> Severity {
        match p {
            0..=3 => Severity::Error,
            4 => Severity::Warn,
            5 | 6 => Severity::Info,
            _ => Severity::Debug,
        }
    }
}

/// One code: what it means and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Code {
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub means: String,
    pub fix: String,
    /// A code no longer logged. Kept so old logs still explain, never reused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
    /// Who defines it: "uQ", or the app named in its file.
    #[serde(default)]
    pub owner: String,
}

impl Code {
    /// The part before the dash: `HYPR` for `HYPR-003`.
    pub fn area(&self) -> &str {
        area(&self.id)
    }
}

/// The part of a code before the dash.
pub fn area(id: &str) -> &str {
    id.split_once('-').map_or(id, |(a, _)| a)
}

#[derive(Deserialize)]
struct File {
    /// The app's name as people know it (e.g. "Bindle"); uQ's own file has none.
    #[serde(default)]
    app: Option<String>,
    /// The systemd user units whose journal `uqctl log` reads for this app.
    #[serde(default)]
    units: Vec<String>,
    /// Syslog identifiers whose lines `uqctl log` reads too, wherever they ran: an app run from a
    /// terminal or a window logs outside its units (`systemd-cat -t bindle`, `logger -t bindle`).
    #[serde(default)]
    identifiers: Vec<String>,
    #[serde(default)]
    code: Vec<Code>,
}

/// Every known code, uQ's first.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    codes: Vec<Code>,
    /// The user units to read logs from (uQ's own and each app's).
    units: Vec<String>,
    /// Apps' syslog identifiers, read wherever their lines came from.
    identifiers: Vec<String>,
}

/// uQ's own user units: where its lines are in the journal.
pub const UQ_UNITS: &[&str] = &[
    "uqd.service",
    "uq-shell.service",
    "uq-lock.service",
    "uqlip.service",
];

pub use log::is_code;

pub use log::code_in;

impl Registry {
    /// uQ's own codes only.
    pub fn builtin() -> &'static Registry {
        static R: LazyLock<Registry> = LazyLock::new(|| {
            let mut r = Registry::default();
            r.add(UQ_CODES, "uQ")
                .expect("codes.toml parses (tested in diag::tests)");
            r.units = UQ_UNITS.iter().map(|s| s.to_string()).collect();
            r
        });
        &R
    }

    /// uQ's codes and every app's from `dir` (normally [`EXTRA_DIR`]). A file that doesn't
    /// parse, or a code that clashes with one already known, is skipped with a warning.
    pub fn with_apps(dir: &Path) -> Registry {
        let mut r = Registry::builtin().clone();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return r;
        };
        let mut files: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .filter(|p| p.file_stem().is_some_and(|s| s != "uq"))
            .collect();
        files.sort();
        for p in files {
            let stem = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            match std::fs::read_to_string(&p) {
                Ok(text) => {
                    if let Err(e) = r.add(&text, &stem) {
                        tracing::debug!("{}: {e}", p.display());
                    }
                }
                Err(e) => tracing::debug!("{}: {e}", p.display()),
            }
        }
        r
    }

    /// The installed registry: [`Registry::with_apps`] on [`EXTRA_DIR`].
    pub fn installed() -> Registry {
        Registry::with_apps(Path::new(EXTRA_DIR))
    }

    /// Add one file's codes, all or none.
    fn add(&mut self, text: &str, fallback_owner: &str) -> Result<(), String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let owner = f.app.unwrap_or_else(|| fallback_owner.to_string());
        for (i, c) in f.code.iter().enumerate() {
            if !is_code(&c.id) {
                return Err(format!("{:?} is not AREA-NNN", c.id));
            }
            if self.get(&c.id).is_some() || f.code[..i].iter().any(|o| o.id == c.id) {
                return Err(format!("{} is defined twice", c.id));
            }
        }
        for mut c in f.code {
            c.owner = owner.clone();
            self.codes.push(c);
        }
        for u in f.units {
            if !self.units.contains(&u) {
                self.units.push(u);
            }
        }
        for i in f.identifiers {
            if !self.identifiers.contains(&i) {
                self.identifiers.push(i);
            }
        }
        Ok(())
    }

    /// Check an app's list (`/usr/share/uq/diag/<app>.toml`) the way `uqctl integrations check`
    /// does: it parses, names the app, its codes share one prefix that isn't one of uQ's areas,
    /// none is listed twice, each says what it means and what to do, and its units are units.
    /// Returns how many codes it has.
    pub fn check_app(text: &str) -> Result<usize, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        if f.app.as_deref().is_none_or(|a| a.trim().is_empty()) {
            return Err("no `app` (the name people know it by)".into());
        }
        let Some(first) = f.code.first() else {
            return Err("no [[code]] entries".into());
        };
        let prefix = first.id.split('-').next().unwrap_or_default().to_string();
        let builtin = Registry::builtin();
        if builtin.all().iter().any(|c| c.area() == prefix) {
            return Err(format!(
                "{prefix} is one of uQ's own areas: pick your app's own prefix"
            ));
        }
        for c in &f.code {
            if !is_code(&c.id) {
                return Err(format!("{:?} is not PREFIX-NNN", c.id));
            }
            if !c.id.starts_with(&format!("{prefix}-")) {
                return Err(format!(
                    "{} doesn't start with {prefix}-: one prefix per app",
                    c.id
                ));
            }
            for (k, v) in [("title", &c.title), ("means", &c.means), ("fix", &c.fix)] {
                if v.trim().is_empty() {
                    return Err(format!("{} has no {k}", c.id));
                }
            }
        }
        let mut r = Registry::default();
        r.add(text, "")?;
        if let Some(u) = f
            .units
            .iter()
            .find(|u| !(u.ends_with(".service") || u.ends_with(".timer")))
        {
            return Err(format!("units: {u:?} is not a .service or .timer"));
        }
        if let Some(i) = f.identifiers.iter().find(|i| !valid_identifier(i)) {
            return Err(format!(
                "identifiers: {i:?} is not a syslog identifier (lower-case letters, digits, - and _)"
            ));
        }
        Ok(f.code.len())
    }

    pub fn get(&self, id: &str) -> Option<&Code> {
        self.codes.iter().find(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub fn all(&self) -> &[Code] {
        &self.codes
    }

    pub fn units(&self) -> &[String] {
        &self.units
    }

    pub fn identifiers(&self) -> &[String] {
        &self.identifiers
    }
}

/// A program name as `-t` takes it: what an app logs under.
fn valid_identifier(i: &str) -> bool {
    !i.is_empty()
        && i.len() <= 64
        && i.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests;
