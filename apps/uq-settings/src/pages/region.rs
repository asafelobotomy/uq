//! The Region page (spec §2.6): the system language, the formats (dates, numbers, money,
//! measurements, paper), and the keyboard layout of the text console, all through localed and
//! its own polkit prompt. The lists come from `localectl`, or from the files it reads.

use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::about::{Runner, run_command};
use super::proxies::{LocaleProxy, fresh};
use super::runner::PageLogic;
use super::sysbus::{self, LOCALE, SystemBus, failure, settle};

/// The locale variables "Formats" sets together.
pub const FORMAT_KEYS: [&str; 5] = [
    "LC_TIME",
    "LC_NUMERIC",
    "LC_MONETARY",
    "LC_MEASUREMENT",
    "LC_PAPER",
];

pub const LOCALE_NOTE: &str = "Saved. It applies after you log out and back in.";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    SetLanguage { locale: String },
    SetFormats { locale: String },
    SetKeymap { keymap: String },
}

pub struct RegionState {
    system: SystemBus,
    root: PathBuf,
    run: Runner,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

/// The value of `key` in localed's `Locale` list.
pub fn get<'a>(vars: &'a [String], key: &str) -> Option<&'a str> {
    vars.iter()
        .find_map(|v| v.strip_prefix(key)?.strip_prefix('='))
}

/// `vars` with `key` set to `value`, or removed when `value` is `None`.
pub fn with_var(vars: &[String], key: &str, value: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = vars
        .iter()
        .filter(|v| v.split_once('=').map(|(k, _)| k) != Some(key))
        .cloned()
        .collect();
    if let Some(value) = value {
        out.push(format!("{key}={value}"));
    }
    out
}

/// The locales `/etc/locale.gen` turns on (`en_GB.UTF-8 UTF-8` gives `en_GB.UTF-8`).
pub fn locale_gen(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect()
}

/// Console keymap names under `dir` (`…/qwerty/uk.map.gz` gives `uk`), sorted.
pub fn scan_keymaps(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() && depth < 4 {
                walk(&path, depth + 1, out);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let stem = name
                    .strip_suffix(".map.gz")
                    .or_else(|| name.strip_suffix(".map"));
                if let Some(stem) = stem {
                    out.push(stem.to_string());
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out.dedup();
    out
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

impl RegionState {
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

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    async fn lists(&self) -> (Vec<String>, Vec<String>) {
        let root = self.root.clone();
        let run = self.run;
        tokio::task::spawn_blocking(move || {
            let locales = run("localectl", &["list-locales", "--no-pager"])
                .map(|t| lines(&t))
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| {
                    locale_gen(
                        &std::fs::read_to_string(root.join("etc/locale.gen")).unwrap_or_default(),
                    )
                });
            let keymaps = run("localectl", &["list-keymaps", "--no-pager"])
                .map(|t| lines(&t))
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| scan_keymaps(&root.join("usr/share/kbd/keymaps")));
            (locales, keymaps)
        })
        .await
        .unwrap_or_default()
    }

    async fn proxy(&self) -> zbus::Result<LocaleProxy<'static>> {
        let conn = self.system.conn().await?;
        fresh(&conn).await
    }

    async fn set_locale(&self, edit: impl FnOnce(&[String]) -> Vec<String>) -> zbus::Result<()> {
        let p = self.proxy().await?;
        let current = match p.locale().await {
            Ok(v) => v,
            Err(e) => return settle(Err::<(), _>(e), &LOCALE),
        };
        let next = edit(&current);
        let refs: Vec<&str> = next.iter().map(String::as_str).collect();
        let r = p.set_locale(&refs, true).await;
        let ok = r.is_ok();
        settle(r, &LOCALE)?;
        if ok && next != current {
            self.notice(LOCALE_NOTE);
        }
        Ok(())
    }

    async fn check_locale(&self, locale: &str) -> zbus::Result<()> {
        let (locales, _) = self.lists().await;
        if locale.is_empty() || (!locales.is_empty() && !locales.iter().any(|l| l == locale)) {
            return Err(failure(
                "That language isn't set up on this computer. Its locale has to be generated first.",
            ));
        }
        Ok(())
    }
}

impl PageLogic for RegionState {
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
            Op::SetLanguage { locale } => {
                self.check_locale(&locale).await?;
                // Formats set on their own stay; unset ones follow the language. A format equal
                // to the new language is dropped, so it follows from now on.
                self.set_locale(|vars| {
                    let out = with_var(vars, "LANG", Some(&locale));
                    FORMAT_KEYS.iter().fold(out, |acc, key| {
                        if get(&acc, key) == Some(locale.as_str()) {
                            with_var(&acc, key, None)
                        } else {
                            acc
                        }
                    })
                })
                .await
            }
            Op::SetFormats { locale } => {
                self.check_locale(&locale).await?;
                self.set_locale(|vars| {
                    let same = get(vars, "LANG") == Some(locale.as_str());
                    FORMAT_KEYS.iter().fold(vars.to_vec(), |acc, key| {
                        with_var(&acc, key, (!same).then_some(locale.as_str()))
                    })
                })
                .await
            }
            Op::SetKeymap { keymap } => {
                let (_, keymaps) = self.lists().await;
                if keymap.is_empty() || (!keymaps.is_empty() && !keymaps.contains(&keymap)) {
                    return Err(failure("That keyboard layout isn't installed."));
                }
                let p = self.proxy().await?;
                settle(
                    p.set_vconsole_keyboard(&keymap, "", false, true).await,
                    &LOCALE,
                )
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let read = async {
            let p = self.proxy().await?;
            let vars = p.locale().await?;
            let keymap = p.vconsole_keymap().await.unwrap_or_default();
            zbus::Result::Ok((vars, keymap))
        };
        let localed = match sysbus::within(read).await {
            Ok((vars, keymap)) => {
                let lang = get(&vars, "LANG").unwrap_or("C").to_string();
                let formats = get(&vars, "LC_TIME").unwrap_or(&lang).to_string();
                json!({"available": true, "language": lang, "formats": formats,
                    "keymap": keymap, "vars": vars})
            }
            Err(e) => sysbus::read_error(&e, &LOCALE),
        };
        let (locales, keymaps) = self.lists().await;
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "localed": localed,
            "locales": locales,
            "keymaps": keymaps,
            "notice": {"seq": seq, "text": text},
        }))
    }
}

#[cfg(test)]
mod tests;
