//! `uqctl integrations list | check` (Phase 4 spec §2.7): what uQ found under `/usr/share/uq/`,
//! and a check an app's CI runs on its `uq/` folder before packaging.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;
use uq_core::integration::appaction::load_app_action;
use uq_core::integration::manifest::{Bus, CommandSpec, load_page, load_widget, valid_bus_name};
use uq_core::integration::rules::{RULES_FILE, load_rules};
use uq_core::integration::scan::{Entry, Roots, scan_all};
use uq_core::launcher_providers::parse_def;
mod checker;
pub use checker::*;

/// The roots under a data prefix laid out like `/usr/share/uq` (the hidden `--root` option).
pub fn roots_under(prefix: &Path) -> Roots {
    Roots {
        widgets: prefix.join("shell/widgets"),
        pages: prefix.join("settings/pages"),
        providers: vec![prefix.join("launcher")],
        actions: prefix.join("launcher/actions"),
        integrations: prefix.join("integrations"),
        bin_dir: uq_core::integration::manifest::BIN_DIR.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// list

/// Mark entries waiting for a bus name that has no owner as hidden. Without a bus the entry
/// keeps its state and says the name wasn't checked.
async fn check_owners(entries: &mut [Entry]) {
    let mut session: Option<Option<zbus::Connection>> = None;
    let mut system: Option<Option<zbus::Connection>> = None;
    for e in entries.iter_mut().filter(|e| e.visible) {
        let Some(name) = e.waits_for.clone() else {
            continue;
        };
        let slot = match e.bus {
            Bus::Session => &mut session,
            Bus::System => &mut system,
        };
        if slot.is_none() {
            let conn = match e.bus {
                Bus::Session => zbus::Connection::session().await,
                Bus::System => zbus::Connection::system().await,
            };
            *slot = Some(conn.ok());
        }
        let owned = match slot.as_ref().and_then(Option::as_ref) {
            Some(conn) => name_has_owner(conn, &name).await,
            None => None,
        };
        match owned {
            Some(true) => {}
            Some(false) => {
                e.visible = false;
                e.error = Some(format!("hidden until {name} is running"));
            }
            None => e.error = Some(format!("shown only while {name} is running (not checked)")),
        }
    }
}

async fn name_has_owner(conn: &zbus::Connection, name: &str) -> Option<bool> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await.ok()?;
    let name = zbus::names::BusName::try_from(name).ok()?;
    tokio::time::timeout(std::time::Duration::from_secs(2), dbus.name_has_owner(name))
        .await
        .ok()?
        .ok()
}

fn state(e: &Entry) -> &'static str {
    match (e.valid, e.visible) {
        (false, _) => "invalid",
        (true, true) => "shown",
        (true, false) => "hidden",
    }
}

/// One line per entry: kind, id, state, source and the reason, tab-separated.
pub fn list_lines(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| {
            let mut line = format!(
                "{}\t{}\t{}\t{}",
                e.kind.as_str(),
                if e.id.is_empty() { "-" } else { &e.id },
                state(e),
                e.source.display()
            );
            if let Some(err) = &e.error {
                line.push('\t');
                line.push_str(&err.replace(['\n', '\t'], " "));
            }
            line
        })
        .collect()
}

/// `[{kind, id, name, source, valid, visible, error, waits_for}]`.
pub fn list_json(entries: &[Entry]) -> serde_json::Value {
    serde_json::Value::Array(
        entries
            .iter()
            .map(|e| {
                json!({
                    "kind": e.kind.as_str(),
                    "id": e.id,
                    "name": e.name,
                    "source": e.source.display().to_string(),
                    "valid": e.valid,
                    "visible": e.visible,
                    "error": e.error,
                    "waits_for": e.waits_for,
                })
            })
            .collect(),
    )
}

pub async fn list(json: bool, root: Option<PathBuf>) -> ExitCode {
    let roots = root.as_deref().map_or_else(Roots::system, roots_under);
    let mut entries = scan_all(&roots);
    check_owners(&mut entries).await;
    if json {
        println!("{}", list_json(&entries));
    } else if entries.is_empty() {
        println!("no integrations installed");
    } else {
        list_lines(&entries).iter().for_each(|l| println!("{l}"));
    }
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------------------------
// check

static TEXT_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^A-Za-z0-9_.])([A-Za-z_][A-Za-z0-9_]*\.)*(TextEdit|TextInput|Text|Label)")
        .unwrap_or_else(|e| panic!("bad lint regex: {e}"))
});

/// The QML text lint of `scripts/lib-qml-lint.sh`, ported: every `Text`, `Label`, `TextEdit`
/// or `TextInput` object (qualified, aliased, inline, or with the brace on the next non-blank
/// line) and every `Qt.createQmlObject` call, as `(line, text)` sorted by line.
pub fn qml_lint_hits(text: &str) -> Vec<(usize, String)> {
    let mut hits: Vec<(usize, String)> = Vec::new();
    let opens_on_line = |line: &str| {
        TEXT_TYPE
            .find_iter(line)
            .any(|m| line[m.end()..].trim_start().starts_with('{'))
    };
    let ends_line = |line: &str| {
        TEXT_TYPE
            .find_iter(line)
            .any(|m| line[m.end()..].trim().is_empty())
    };
    let mut prev: Option<(usize, &str)> = None;
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if opens_on_line(line) || line.contains("Qt.createQmlObject") {
            hits.push((n, line.to_string()));
        }
        if let Some((pn, pl)) = prev
            && line.trim_start().starts_with('{')
        {
            hits.push((pn, pl.to_string()));
        }
        if line.trim().is_empty() {
            continue;
        }
        prev = ends_line(line).then_some((n, line));
    }
    hits.sort();
    hits.dedup();
    hits
}

/// The `org.uq.settings` components a page may use only with `uq_api = 2` or later.
static API2_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^A-Za-z0-9_.])(SettingsPage|SettingsTab|SettingsCard|HelpTip|UqAppIcon)\s*\{")
        .unwrap_or_else(|e| panic!("bad api2 regex: {e}"))
});

/// Each object of a component that needs `uq_api = 2`, as `(line, component)`.
pub fn api2_hits(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .flat_map(|(i, line)| {
            API2_TYPE
                .captures_iter(line)
                .map(move |c| (i + 1, c[2].to_string()))
        })
        .collect()
}

/// `ctx.command.stream(`, which a page may call only with `uq_api = 3` or later.
static API3_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bcommand\s*\.\s*stream\s*\(").unwrap_or_else(|e| panic!("bad api3 regex: {e}"))
});

/// Each call that needs `uq_api = 3`, as `(line, call)`.
pub fn api3_hits(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .flat_map(|(i, line)| {
            API3_CALL
                .find_iter(line)
                .map(move |_| (i + 1, "ctx.command.stream".to_string()))
        })
        .collect()
}

/// Check a repo's `uq/` folder: its layout, every manifest, the QML text lint, that every
/// command's program exists, provider bus names, service files for activatable providers, and
/// `integrations/<name>/rules.toml` window rules. With `programs` false, whether the programs
/// are installed is not checked (their paths still are, by the manifest rules).
/// Returns the problems and a one-line summary.
pub fn check_dir(dir: &Path, programs: bool) -> (Vec<String>, String) {
    let mut c = Checker {
        problems: Vec::new(),
        activatable: Vec::new(),
        services: Vec::new(),
        counts: [0; 6],
        programs,
    };
    match sorted_entries(dir) {
        Err(e) => c.problems.push(e),
        Ok(list) => {
            for p in list {
                match (name_of(&p).as_str(), p.is_dir()) {
                    ("widgets", true) => c.folders(&p, "widget"),
                    ("pages", true) => c.folders(&p, "page"),
                    ("launcher", true) => c.launcher(&p),
                    ("dbus", true) => c.dbus(&p),
                    ("integrations", true) => c.integrations(&p),
                    ("diag", true) => c.diag(&p),
                    // README.md is for people; native.toml pins the uQ kits the repo vendors
                    // (native/check-uq-native.sh), and nothing is installed from either.
                    ("README.md" | "native.toml", false) => {}
                    _ => c.problem(
                        &p,
                        "not part of a uq/ folder (widgets/, pages/, launcher/, dbus/, \
                         integrations/, diag/, README.md, native.toml)",
                    ),
                }
            }
        }
    }
    for (p, bus) in std::mem::take(&mut c.activatable) {
        if !c.services.contains(&bus) {
            c.problem(
                &p,
                format!("Activatable=true needs dbus/{bus}.service with Name={bus}"),
            );
        }
    }
    let [w, pg, pr, a, s, r] = c.counts;
    let mut summary = format!(
        "{w} widget(s), {pg} page(s), {pr} provider(s), {a} app action(s), {s} service file(s), \
         {r} rule file(s)"
    );
    if !programs {
        summary.push_str(" (programs not checked)");
    }
    (c.problems, summary)
}

pub fn check(dir: &Path, programs: bool) -> ExitCode {
    let (problems, summary) = check_dir(dir, programs);
    if problems.is_empty() {
        println!("ok: {summary}");
        return ExitCode::SUCCESS;
    }
    for p in &problems {
        println!("{p}");
    }
    println!("{} problem(s); checked {summary}", problems.len());
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests;
