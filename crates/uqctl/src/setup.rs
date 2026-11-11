//! `uqctl setup`: install the per-user files that turn a Hyprland session into uQ.
//!
//! Safe to run unattended and repeatedly (bindle runs it for the installing user): it never
//! overwrites a file the user owns, a second run changes nothing, and the outcome is in the exit
//! code (and, with `--json`, on stdout):
//!
//! | exit | meaning |
//! |------|---------|
//! | 0 | uQ's `hyprland.lua` is in place (written now, or already there) |
//! | 1 | an error: uQ's base config is missing (package not installed), or a file couldn't be written |
//! | 2 | refused: run as root (it would set up root's account, not the user's) |
//! | 3 | the user's own `hyprland.lua` was kept, so the session is not uQ until they add the stub |
//!
//! App theme files (`--apps`) are reported but never change the exit code: leaving a user's own
//! kitty or GTK config alone is not a failure.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use uq_core::paths::Paths;
use uq_core::render::kvantum_base;
use uq_core::stub::{
    StubOutcome, consumer_files, install_consumers, install_stub, kvantum_link, stub_contents,
};

pub const EXIT_OK: u8 = 0;
pub const EXIT_ERROR: u8 = 1;
pub const EXIT_ROOT: u8 = 2;
pub const EXIT_KEPT_USER_CONFIG: u8 = 3;

#[derive(Debug, PartialEq, Eq)]
pub enum SetupError {
    /// Running as uid 0.
    Root,
    /// `base.lua` (shipped by the uq package) isn't where the stub would point.
    MissingBase(PathBuf),
    /// Writing or reading a file failed.
    Io(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub main_lua: PathBuf,
    pub hypr: StubOutcome,
    /// `None` when `--apps` wasn't given.
    pub apps: Option<Vec<(PathBuf, StubOutcome)>>,
}

impl Report {
    pub fn exit_code(&self) -> u8 {
        match self.hypr {
            StubOutcome::Installed => EXIT_OK,
            StubOutcome::AlreadyPresent => EXIT_KEPT_USER_CONFIG,
        }
    }
}

/// The real uid, from the owner of `/proc/self` (no libc dependency).
pub fn current_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

pub fn run_setup(
    paths: &Paths,
    apps: bool,
    kvantum: bool,
    uid: Option<u32>,
) -> Result<Report, SetupError> {
    if uid == Some(0) {
        return Err(SetupError::Root);
    }
    if !paths.base_lua.is_file() {
        return Err(SetupError::MissingBase(paths.base_lua.clone()));
    }
    let hypr = install_stub(paths)
        .map_err(|e| SetupError::Io(format!("{}: {e}", paths.main_lua.display())))?;
    let apps = if apps {
        Some(
            install_consumers(paths, kvantum)
                .map_err(|e| SetupError::Io(format!("app theme files: {e}")))?,
        )
    } else {
        None
    };
    Ok(Report {
        main_lua: paths.main_lua.clone(),
        hypr,
        apps,
    })
}

fn outcome_word(o: StubOutcome) -> &'static str {
    match o {
        StubOutcome::Installed => "installed",
        StubOutcome::AlreadyPresent => "kept_user_file",
    }
}

pub fn report_json(r: &Report) -> Value {
    json!({
        "hyprland_lua": { "path": r.main_lua, "outcome": outcome_word(r.hypr) },
        "apps": r.apps.as_ref().map(|list| list.iter().map(|(p, o)| json!({
            "path": p, "outcome": outcome_word(*o)
        })).collect::<Vec<_>>()),
        "exit": r.exit_code(),
    })
}

pub fn error_json(e: &SetupError) -> Value {
    let (code, kind, message) = describe(e);
    json!({ "error": kind, "message": message, "exit": code })
}

fn describe(e: &SetupError) -> (u8, &'static str, String) {
    match e {
        SetupError::Root => (
            EXIT_ROOT,
            "root",
            "refusing to run as root: run `uqctl setup` as the user who will log in to uQ \
             (for example `runuser -l <user> -c 'uqctl setup'`)"
                .into(),
        ),
        SetupError::MissingBase(p) => (
            EXIT_ERROR,
            "missing_base",
            format!(
                "uQ's base config {} is missing; is the uq package installed?",
                p.display()
            ),
        ),
        SetupError::Io(m) => (EXIT_ERROR, "io", m.clone()),
    }
}

fn print_human(paths: &Paths, r: &Report, kvantum: bool) {
    match r.hypr {
        StubOutcome::Installed => {
            println!("uQ config in place at {}", r.main_lua.display());
            println!("Log out and choose Hyprland, or run `hyprctl reload` in a running session.");
        }
        StubOutcome::AlreadyPresent => {
            println!(
                "{} already exists and was left alone.",
                r.main_lua.display()
            );
            println!("To use uQ, put these lines at the top of it:\n");
            print!("{}", stub_contents(paths));
        }
    }
    let Some(apps) = &r.apps else {
        println!(
            "App theming (GTK, qt6ct, terminals) not changed. Run `uqctl setup --apps` to opt in; \
             those files also apply in other desktops."
        );
        return;
    };
    let lines: HashMap<PathBuf, String> = consumer_files(paths, kvantum).into_iter().collect();
    for (path, outcome) in apps {
        match outcome {
            StubOutcome::Installed => println!("themed: {}", path.display()),
            StubOutcome::AlreadyPresent if *path == kvantum_link(paths) => {
                println!("left alone: {} (already exists)", path.display())
            }
            StubOutcome::AlreadyPresent => {
                println!(
                    "left alone: {} (add this to use uQ's theme):\n  {}",
                    path.display(),
                    lines
                        .get(Path::new(path))
                        .map(|l| l.trim_end().replace('\n', "\n  "))
                        .unwrap_or_default()
                );
                if kvantum && path.ends_with("qt6ct/qt6ct.conf") {
                    println!("  (set style=kvantum to use it)");
                }
            }
        }
    }
}

/// `uqctl setup [--apps] [--json | --quiet]`. `--quiet` prints nothing on success (errors still
/// reach stderr), for `uq-session`, which runs it at every login that lacks the stub.
pub fn main(apps: bool, as_json: bool, quiet: bool) -> ExitCode {
    let paths = Paths::from_env();
    let kvantum = kvantum_base(Path::new("/usr/share/Kvantum")).is_some();
    match run_setup(&paths, apps, kvantum, current_uid()) {
        Ok(r) => {
            if as_json {
                println!("{}", report_json(&r));
            } else if !quiet {
                print_human(&paths, &r, kvantum);
            } else if r.exit_code() == EXIT_KEPT_USER_CONFIG {
                eprintln!(
                    "uqctl: {} is your own file and was kept; uQ is not set up in it",
                    r.main_lua.display()
                );
            }
            ExitCode::from(r.exit_code())
        }
        Err(e) => {
            let (code, _, message) = describe(&e);
            if as_json {
                println!("{}", error_json(&e));
            }
            eprintln!("uqctl: {message}");
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn with_base(dir: &Path) -> Paths {
        let p = Paths::under(dir);
        fs::create_dir_all(p.base_lua.parent().unwrap()).unwrap();
        fs::write(&p.base_lua, "-- base\n").unwrap();
        p
    }

    #[test]
    fn fresh_user_gets_the_stub_and_exit_0() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        let r = run_setup(&p, false, false, Some(1000)).unwrap();
        assert_eq!(r.hypr, StubOutcome::Installed);
        assert_eq!(r.exit_code(), EXIT_OK);
        assert_eq!(fs::read_to_string(&p.main_lua).unwrap(), stub_contents(&p));
        assert_eq!(r.apps, None);
    }

    #[test]
    fn second_run_changes_nothing_and_still_exits_0() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        run_setup(&p, true, false, Some(1000)).unwrap();
        let before: Vec<_> = consumer_files(&p, false)
            .into_iter()
            .map(|(f, _)| fs::read_to_string(f).unwrap())
            .collect();
        let r = run_setup(&p, true, false, Some(1000)).unwrap();
        assert_eq!(r.exit_code(), EXIT_OK);
        assert!(
            r.apps
                .unwrap()
                .iter()
                .all(|(_, o)| *o == StubOutcome::Installed)
        );
        let after: Vec<_> = consumer_files(&p, false)
            .into_iter()
            .map(|(f, _)| fs::read_to_string(f).unwrap())
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn users_own_hyprland_lua_is_kept_and_exits_3() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        fs::create_dir_all(&p.hypr_dir).unwrap();
        fs::write(&p.main_lua, "-- mine\n").unwrap();
        let r = run_setup(&p, false, false, Some(1000)).unwrap();
        assert_eq!(r.exit_code(), EXIT_KEPT_USER_CONFIG);
        assert_eq!(fs::read_to_string(&p.main_lua).unwrap(), "-- mine\n");
        assert_eq!(report_json(&r)["hyprland_lua"]["outcome"], "kept_user_file");
    }

    #[test]
    fn kept_app_files_do_not_change_the_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        let (kitty, _) = consumer_files(&p, false)
            .into_iter()
            .find(|(f, _)| f.ends_with("kitty/kitty.conf"))
            .unwrap();
        fs::create_dir_all(kitty.parent().unwrap()).unwrap();
        fs::write(&kitty, "font_size 12\n").unwrap();
        let r = run_setup(&p, true, false, Some(1000)).unwrap();
        assert_eq!(r.exit_code(), EXIT_OK);
        assert_eq!(fs::read_to_string(&kitty).unwrap(), "font_size 12\n");
    }

    #[test]
    fn kept_kvantum_files_do_not_change_the_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        let kvdir = uq_core::stub::kvantum_link(&p)
            .parent()
            .unwrap()
            .to_path_buf();
        fs::create_dir_all(kvdir.join("uq")).unwrap();
        fs::write(kvdir.join("kvantum.kvconfig"), "[General]\ntheme=KvArc\n").unwrap();
        let r = run_setup(&p, true, true, Some(1000)).unwrap();
        assert_eq!(r.exit_code(), EXIT_OK);
        assert_eq!(
            fs::read_to_string(kvdir.join("kvantum.kvconfig")).unwrap(),
            "[General]\ntheme=KvArc\n"
        );
        let apps = r.apps.unwrap();
        assert!(
            apps.iter()
                .any(|(f, o)| f.ends_with("kvantum.kvconfig") && *o == StubOutcome::AlreadyPresent)
        );
    }

    #[test]
    fn root_is_refused_with_exit_2_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        assert_eq!(run_setup(&p, true, false, Some(0)), Err(SetupError::Root));
        assert!(!p.main_lua.exists());
        assert_eq!(error_json(&SetupError::Root)["exit"], EXIT_ROOT);
    }

    #[test]
    fn missing_base_is_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::under(dir.path());
        assert_eq!(
            run_setup(&p, false, false, Some(1000)),
            Err(SetupError::MissingBase(p.base_lua.clone()))
        );
        assert!(!p.main_lua.exists());
        assert_eq!(
            error_json(&SetupError::MissingBase(p.base_lua.clone()))["error"],
            "missing_base"
        );
    }

    #[test]
    fn json_report_shape() {
        let dir = tempfile::tempdir().unwrap();
        let p = with_base(dir.path());
        let v = report_json(&run_setup(&p, true, false, Some(1000)).unwrap());
        assert_eq!(v["exit"], 0);
        assert_eq!(v["hyprland_lua"]["outcome"], "installed");
        assert_eq!(
            v["apps"].as_array().unwrap().len(),
            consumer_files(&p, false).len()
        );
        let v = report_json(&run_setup(&p, false, false, Some(1000)).unwrap());
        assert!(v["apps"].is_null());
    }
}
