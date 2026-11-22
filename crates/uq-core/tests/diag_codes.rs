//! Every warning and error uQ logs carries a code from `codes.toml`, and every code there is
//! still used (docs/diagnostics.md). Reads the source: Rust under `crates/` and `apps/`, QML
//! and JS under `shell/`, `lock/`, `greeter/` and Settings' `qml/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use uq_core::diag::{Registry, code_in};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn walk(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !["target", "tests", "testdata", "node_modules", ".git"].contains(&name.as_str()) {
                walk(&p, exts, out);
            }
        } else if p.extension().is_some_and(|x| exts.iter().any(|e| x == *e)) && name != "tests.rs"
        {
            out.push(p);
        }
    }
}

fn rust_files() -> Vec<PathBuf> {
    let r = root();
    let mut v = Vec::new();
    for d in ["crates", "apps/uq-settings/src", "native/uq-diag"] {
        walk(&r.join(d), &["rs"], &mut v);
    }
    v
}

fn qml_files() -> Vec<PathBuf> {
    let r = root();
    let mut v = Vec::new();
    for d in ["shell", "lock", "greeter", "apps/uq-settings/qml"] {
        walk(&r.join(d), &["qml", "js"], &mut v);
    }
    v
}

/// The text of each `tracing::warn!`/`tracing::error!` call up to its first string literal's
/// end, with its line number.
fn rust_calls(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for mac in ["tracing::warn!(", "tracing::error!("] {
        let mut from = 0;
        while let Some(i) = text[from..].find(mac) {
            let at = from + i;
            let rest = &text[at..];
            let lit = rest.find('"').map(|q| {
                let body = &rest[q + 1..];
                let end = body.find('"').unwrap_or(body.len());
                body[..end].to_string()
            });
            let line = text[..at].lines().count();
            out.push((line, lit.unwrap_or_default()));
            from = at + mac.len();
        }
    }
    out
}

#[test]
fn every_warning_and_error_has_a_known_code() {
    let reg = Registry::builtin();
    let mut bad = Vec::new();
    for f in rust_files() {
        let text = std::fs::read_to_string(&f).unwrap();
        // Unit tests inside a file: what follows `#[cfg(test)]\nmod tests {` isn't shipped.
        let shipped = text.split("#[cfg(test)]\nmod tests {").next().unwrap();
        for (line, lit) in rust_calls(shipped) {
            match code_in(&lit) {
                Some(c)
                    if lit.starts_with(&format!("[{c}] "))
                        && reg.get(c).is_some_and(|x| !x.retired) => {}
                Some(c) if reg.get(c).is_none() => {
                    bad.push(format!("{}:{line}: unknown code {c}", f.display()))
                }
                _ => bad.push(format!(
                    "{}:{line}: no [CODE] at the start of {lit:?}",
                    f.display()
                )),
            }
        }
    }
    for f in qml_files() {
        let text = std::fs::read_to_string(&f).unwrap();
        for (n, l) in text.lines().enumerate() {
            if !(l.contains("console.warn(") || l.contains("console.error(")) {
                continue;
            }
            match code_in(l) {
                Some(c) if reg.get(c).is_some_and(|x| !x.retired) => {}
                Some(c) => bad.push(format!("{}:{}: unknown code {c}", f.display(), n + 1)),
                None => bad.push(format!(
                    "{}:{}: console.warn/error without uq[CODE]",
                    f.display(),
                    n + 1
                )),
            }
        }
    }
    assert!(
        bad.is_empty(),
        "every warning and error needs a code from crates/uq-core/src/diag/codes.toml \
         (Rust: `tracing::warn!(\"[AREA-NNN] …\")`, QML: `console.warn(\"uq[AREA-NNN] …\")`):\n{}",
        bad.join("\n")
    );
}

#[test]
fn every_code_is_still_used() {
    let reg = Registry::builtin();
    let mut used = BTreeSet::new();
    for f in rust_files().into_iter().chain(qml_files()) {
        let text = std::fs::read_to_string(&f).unwrap();
        for c in reg.all() {
            if text.contains(&c.id) {
                used.insert(c.id.clone());
            }
        }
    }
    let unused: Vec<&str> = reg
        .all()
        .iter()
        .filter(|c| !c.retired && !used.contains(&c.id))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        unused.is_empty(),
        "codes nothing logs any more: retire them (retired = true) instead of deleting: {unused:?}"
    );
}
