//! `uqctl integrations check`: the Checker that walks an integration tree and lists its problems.

use super::*;

pub(crate) fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    v.sort();
    Ok(v)
}

pub(crate) fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Problems found in a repo's `uq/` folder (§2.6 layout, §2.7 checks); empty when clean.
pub struct Checker {
    pub(crate) problems: Vec<String>,
    /// Bus names of activatable providers, to match against `dbus/*.service`.
    pub(crate) activatable: Vec<(PathBuf, String)>,
    pub(crate) services: Vec<String>,
    pub(crate) counts: [usize; 6],
    /// Check that each command's and app action's program exists (off with `--no-programs`).
    pub(crate) programs: bool,
}

impl Checker {
    pub(crate) fn problem(&mut self, path: &Path, msg: impl std::fmt::Display) {
        self.problems.push(format!("{}: {msg}", path.display()));
    }

    pub(crate) fn lint_folder(&mut self, dir: &Path) {
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

    /// A page below `uq_api = 3` (or 2) that uses a component or call added later: an older uQ would show
    /// "This page couldn't load" instead of "needs a newer uQ".
    pub(crate) fn api_folder(&mut self, dir: &Path, api: u32) {
        let Ok(files) = sorted_entries(dir) else {
            return;
        };
        for f in files {
            if f.is_dir() {
                self.api_folder(&f, api);
            } else if (f.extension().is_some_and(|x| x == "qml" || x == "js"))
                && let Ok(text) = std::fs::read_to_string(&f)
            {
                if api < 2 && f.extension().is_some_and(|x| x == "qml") {
                    for (n, name) in api2_hits(&text) {
                        self.problems.push(format!(
                            "{}:{n}: {name} needs uq_api = 2 in page.toml (it says {api})",
                            f.display()
                        ));
                    }
                }
                if api < 3 {
                    for (n, name) in api3_hits(&text) {
                        self.problems.push(format!(
                            "{}:{n}: {name} needs uq_api = 3 in page.toml (it says {api})",
                            f.display()
                        ));
                    }
                }
            }
        }
    }

    pub(crate) fn commands(&mut self, path: &Path, commands: &[CommandSpec]) {
        if !self.programs {
            return;
        }
        for c in commands {
            if !Path::new(&c.argv[0]).exists() {
                self.problem(
                    path,
                    format!("command {:?}: {} does not exist", c.id, c.argv[0]),
                );
            }
        }
    }

    pub(crate) fn folders(&mut self, dir: &Path, kind: &str) {
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
                load_widget(&p, &root).map(|l| (l.manifest.commands, None))
            } else {
                load_page(&p, &root).map(|l| (l.manifest.commands, Some(l.manifest.uq_api)))
            };
            match result {
                Ok((cmds, page_api)) => {
                    self.counts[usize::from(kind != "widget")] += 1;
                    self.commands(&p, &cmds);
                    if let Some(api) = page_api.filter(|a| *a < 3) {
                        self.api_folder(&p, api);
                    }
                }
                Err(e) => self.problem(&p, e),
            }
            self.lint_folder(&p);
        }
    }

    pub(crate) fn launcher(&mut self, dir: &Path) {
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

    pub(crate) fn actions(&mut self, dir: &Path) {
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
                    if self.programs && !a.program_exists() {
                        self.problem(&p, format!("Exec program {} does not exist", a.program()));
                    }
                }
                Err(e) => self.problem(&p, e),
            }
        }
    }

    /// `integrations/<name>/rules.toml` (Phase 4 spec §3.3): the folder name is the
    /// integration's name and every rule id starts with it.
    pub(crate) fn integrations(&mut self, dir: &Path) {
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

    pub(crate) fn dbus(&mut self, dir: &Path) {
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

impl Checker {
    /// `diag/<app>.toml`: the app's problem codes (docs/diagnostics.md), installed to
    /// `/usr/share/uq/diag/`.
    pub(crate) fn diag(&mut self, dir: &Path) {
        let list = match sorted_entries(dir) {
            Ok(l) => l,
            Err(e) => return self.problems.push(e),
        };
        for p in list {
            let name = name_of(&p);
            if !p.is_file() || !name.ends_with(".toml") || name == "uq.toml" {
                self.problem(&p, "only <app>.toml files belong in diag/");
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            if let Err(e) = uq_core::diag::Registry::check_app(&text) {
                self.problem(&p, e);
            }
        }
    }
}
