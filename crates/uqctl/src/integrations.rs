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

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    v.sort();
    Ok(v)
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Problems found in a repo's `uq/` folder (§2.6 layout, §2.7 checks); empty when clean.
pub struct Checker {
    problems: Vec<String>,
    /// Bus names of activatable providers, to match against `dbus/*.service`.
    activatable: Vec<(PathBuf, String)>,
    services: Vec<String>,
    counts: [usize; 6],
}

impl Checker {
    fn problem(&mut self, path: &Path, msg: impl std::fmt::Display) {
        self.problems.push(format!("{}: {msg}", path.display()));
    }

    fn lint_folder(&mut self, dir: &Path) {
        let Ok(files) = sorted_entries(dir) else {
            return;
        };
        for f in files {
            let is_qml = f.extension().is_some_and(|x| x == "qml" || x == "js");
            if f.is_dir() {
                self.lint_folder(&f);
            } else if is_qml && let Ok(text) = std::fs::read_to_string(&f) {
                for (n, line) in qml_lint_hits(&text) {
                    self.problems.push(format!(
                        "{}:{n}: Text, Label, TextEdit, TextInput and Qt.createQmlObject are not \
                         allowed; use the context's UqText (widgets) or UqLabel (pages): {}",
                        f.display(),
                        line.trim()
                    ));
                }
            }
        }
    }

    fn commands(&mut self, path: &Path, commands: &[CommandSpec]) {
        for c in commands {
            if !Path::new(&c.argv[0]).exists() {
                self.problem(
                    path,
                    format!("command {:?}: {} does not exist", c.id, c.argv[0]),
                );
            }
        }
    }

    fn folders(&mut self, dir: &Path, kind: &str) {
        let Ok(list) = sorted_entries(dir) else {
            return self.problem(dir, "can't be read");
        };
        let root = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        for p in list {
            if !p.is_dir() {
                self.problem(&p, format!("only <id>/ folders belong in {kind}s/"));
                continue;
            }
            let result = if kind == "widget" {
                load_widget(&p, &root).map(|l| l.manifest.commands)
            } else {
                load_page(&p, &root).map(|l| l.manifest.commands)
            };
            match result {
                Ok(cmds) => {
                    self.counts[usize::from(kind != "widget")] += 1;
                    self.commands(&p, &cmds);
                }
                Err(e) => self.problem(&p, e),
            }
            self.lint_folder(&p);
        }
    }

    fn launcher(&mut self, dir: &Path) {
        let Ok(list) = sorted_entries(dir) else {
            return self.problem(dir, "can't be read");
        };
        let mut seen: Vec<String> = Vec::new();
        for p in list {
            let name = name_of(&p);
            if p.is_dir() && name == "actions" {
                self.actions(&p);
            } else if p.is_file() && name.ends_with(".provider") {
                let parsed = std::fs::read_to_string(&p)
                    .map_err(|e| e.to_string())
                    .and_then(|t| parse_def(&t, dir));
                match parsed {
                    Ok(d) if seen.contains(&d.bus_name) => {
                        self.problem(&p, format!("BusName {} is defined twice", d.bus_name));
                    }
                    Ok(d) => {
                        self.counts[2] += 1;
                        if d.activatable {
                            self.activatable.push((p.clone(), d.bus_name.clone()));
                        }
                        seen.push(d.bus_name);
                    }
                    Err(e) => self.problem(&p, e),
                }
            } else {
                self.problem(
                    &p,
                    "only <id>.provider files and an actions/ folder belong in launcher/",
                );
            }
        }
    }

    fn actions(&mut self, dir: &Path) {
        let Ok(list) = sorted_entries(dir) else {
            return self.problem(dir, "can't be read");
        };
        for p in list {
            if !(p.is_file() && name_of(&p).ends_with(".appaction")) {
                self.problem(&p, "only <id>.appaction files belong in launcher/actions/");
                continue;
            }
            match load_app_action(&p) {
                Ok(a) => {
                    self.counts[3] += 1;
                    if !a.program_exists() {
                        self.problem(&p, format!("Exec program {} does not exist", a.program()));
                    }
                }
                Err(e) => self.problem(&p, e),
            }
        }
    }

    /// `integrations/<name>/rules.toml` (Phase 4 spec §3.3): the folder name is the
    /// integration's name and every rule id starts with it.
    fn integrations(&mut self, dir: &Path) {
        let Ok(list) = sorted_entries(dir) else {
            return self.problem(dir, "can't be read");
        };
        let root = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        for p in list {
            if !p.is_dir() {
                self.problem(&p, "only <name>/ folders belong in integrations/");
                continue;
            }
            let Ok(files) = sorted_entries(&p) else {
                self.problem(&p, "can't be read");
                continue;
            };
            for f in files.iter().filter(|f| name_of(f) != RULES_FILE) {
                self.problem(
                    f,
                    format!("only {RULES_FILE} belongs in integrations/<name>/"),
                );
            }
            if !files.iter().any(|f| name_of(f) == RULES_FILE) {
                self.problem(&p, format!("has no {RULES_FILE}"));
                continue;
            }
            match load_rules(&p, &root) {
                Ok(_) => self.counts[5] += 1,
                Err(e) => self.problem(&p.join(RULES_FILE), e),
            }
        }
    }

    fn dbus(&mut self, dir: &Path) {
        let Ok(list) = sorted_entries(dir) else {
            return self.problem(dir, "can't be read");
        };
        for p in list {
            if !(p.is_file() && name_of(&p).ends_with(".service")) {
                self.problem(&p, "only <name>.service files belong in dbus/");
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let mut in_group = false;
            let (mut name, mut exec) = (None, None);
            for line in text.lines().map(str::trim) {
                if line.starts_with('[') {
                    in_group = line == "[D-BUS Service]";
                } else if in_group && let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        "Name" => name = Some(v.trim().to_string()),
                        "Exec" => exec = Some(v.trim().to_string()),
                        _ => {}
                    }
                }
            }
            match name {
                Some(n) if valid_bus_name(&n) => {
                    if name_of(&p) != format!("{n}.service") {
                        self.problem(&p, format!("the file should be named {n}.service"));
                    }
                    self.services.push(n);
                }
                Some(n) => self.problem(&p, format!("Name {n:?} is not a valid bus name")),
                None => self.problem(&p, "no Name in [D-BUS Service]"),
            }
            if !exec.is_some_and(|e| e.starts_with('/')) {
                self.problem(&p, "Exec must start with an absolute path");
            }
            self.counts[4] += 1;
        }
    }
}

/// Check a repo's `uq/` folder: its layout, every manifest, the QML text lint, that every
/// command's program exists, provider bus names, service files for activatable providers, and
/// `integrations/<name>/rules.toml` window rules.
/// Returns the problems and a one-line summary.
pub fn check_dir(dir: &Path) -> (Vec<String>, String) {
    let mut c = Checker {
        problems: Vec::new(),
        activatable: Vec::new(),
        services: Vec::new(),
        counts: [0; 6],
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
                    ("README.md", false) => {}
                    _ => c.problem(
                        &p,
                        "not part of a uq/ folder (widgets/, pages/, launcher/, dbus/, \
                         integrations/, README.md)",
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
    let summary = format!(
        "{w} widget(s), {pg} page(s), {pr} provider(s), {a} app action(s), {s} service file(s), \
         {r} rule file(s)"
    );
    (c.problems, summary)
}

pub fn check(dir: &Path) -> ExitCode {
    let (problems, summary) = check_dir(dir);
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
mod tests {
    use super::*;

    #[test]
    fn lint_matches_the_shell_script() {
        let qml = "import QtQuick\n\
                   Item {\n\
                   \x20   Text { text: \"a\" }\n\
                   \x20   UqText { text: \"ok\" }\n\
                   \x20   QQC2.Label {\n\
                   \x20   }\n\
                   \x20   property var x: Qt.createQmlObject(\"\", this)\n\
                   \x20   TextInput\n\
                   \n\
                   \x20   {\n\
                   \x20   }\n\
                   \x20   Label\n\
                   \x20   Item {}\n\
                   \x20   // Text is fine in prose without a brace\n\
                   \x20   Item { Text {} }\n\
                   \x20   ctx.UqText {}\n\
                   \x20   Controls.TextEdit{}\n\
                   }\n";
        let lines: Vec<usize> = qml_lint_hits(qml).into_iter().map(|(n, _)| n).collect();
        assert_eq!(lines, [3, 5, 7, 8, 15, 17]);
    }

    #[test]
    fn list_lines_show_state_and_reason() {
        use uq_core::integration::scan::Kind;
        let e = Entry {
            kind: Kind::AppAction,
            id: "x".into(),
            name: Some("X".into()),
            source: "/s/x.appaction".into(),
            valid: true,
            visible: false,
            error: Some("hidden: /usr/bin/x\nis missing".into()),
            waits_for: None,
            bus: Bus::Session,
        };
        assert_eq!(
            list_lines(std::slice::from_ref(&e)),
            ["app_action\tx\thidden\t/s/x.appaction\thidden: /usr/bin/x is missing"]
        );
        let j = list_json(&[e]);
        assert_eq!(j[0]["kind"], "app_action");
        assert_eq!(j[0]["visible"], false);
    }
}
