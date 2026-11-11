//! `plan` with a per-app fix: the env wrapper, terminals, field codes and marked overrides (R19).

use super::*;
use crate::bridge::run::RunState;
use serde_json::json;
use std::path::PathBuf;
use uq_core::compat::{AppFix, Platform};
use uq_core::env::Var;

/// A data home and one system data dir, each with an `applications` folder.
struct Fx {
    _t: tempfile::TempDir,
    home: PathBuf,
    sys: PathBuf,
    dirs: Dirs,
}

fn fx(home: &[(&str, &str)], sys: &[(&str, &str)]) -> Fx {
    let t = tempfile::tempdir().unwrap();
    let h = t.path().join("home/applications");
    let s = t.path().join("sys/applications");
    for (dir, files) in [(&h, home), (&s, sys)] {
        std::fs::create_dir_all(dir).unwrap();
        for (n, c) in files {
            std::fs::write(dir.join(n), c).unwrap();
        }
    }
    let dirs = Dirs::new(Some(h.clone()), vec![s.clone()]);
    Fx {
        _t: t,
        home: h,
        sys: s,
        dirs,
    }
}

fn wayland(id: &str, args: &[&str]) -> AppFix {
    let mut f = AppFix::new(id);
    f.platform = Platform::Wayland;
    f.args = args.iter().map(|a| a.to_string()).collect();
    f
}

fn cmd(f: &Fx, id: &str, fix: Option<&AppFix>, terminal: Option<&str>) -> String {
    plan(
        Some(&json!({ "desktop_id": id })),
        &f.dirs,
        0,
        &RunState::default(),
        terminal,
        fix,
    )
    .unwrap()
    .command
}

const CODE: (&str, &str) = ("code.desktop", "[Desktop Entry]\nExec=code %F\nName=Code\n");
const WRAP: &str = "-- 'env' '--' 'GDK_BACKEND=wayland,x11' 'QT_QPA_PLATFORM=wayland;xcb' \
                    'SDL_VIDEODRIVER=wayland,x11' 'ELECTRON_OZONE_PLATFORM_HINT=wayland' ";

#[test]
fn launch_applies_fix_env_and_args() {
    let f = fx(&[], &[CODE]);
    let c = cmd(&f, "code", Some(&wayland("code", &["--x"])), None);
    assert!(c.contains(WRAP), "{c}");
    assert!(c.ends_with(&format!("{WRAP}'code' '--x'")), "{c}");
}

#[test]
fn launch_without_fix_unchanged() {
    let f = fx(&[], &[CODE]);
    assert!(cmd(&f, "code", None, None).ends_with("--unit=app-uq-code-00000000 -- 'code'"));
}

#[test]
fn launch_terminal_app_wraps_env_before_terminal() {
    let f = fx(
        &[],
        &[("t.desktop", "[Desktop Entry]\nExec=htop\nTerminal=true\n")],
    );
    let mut fix = AppFix::new("t");
    fix.env = vec![Var {
        name: "FOO".into(),
        value: "bar".into(),
    }];
    let c = cmd(&f, "t.desktop", Some(&fix), None);
    assert!(
        c.ends_with("-- 'env' '--' 'FOO=bar' 'kitty' '-e' 'htop'"),
        "{c}"
    );
}

/// A marked override in the data home, as uqd writes it, naming `source`.
fn marked(source: &str) -> String {
    format!(
        "# Written by uQ\n[Desktop Entry]\nExec=env -- GDK_BACKEND=wayland,x11 code-ovr %F --x\n\
         X-uQ-Override=true\nX-uQ-Source={source}\n"
    )
}

#[test]
fn launch_marked_override_uses_source() {
    let f = fx(&[], &[CODE]);
    let src = f.sys.join("code.desktop");
    std::fs::write(
        f.home.join("code.desktop"),
        marked(&src.display().to_string()),
    )
    .unwrap();
    let c = cmd(&f, "code.desktop", Some(&wayland("code", &["--x"])), None);
    assert!(c.ends_with(&format!("{WRAP}'code' '--x'")), "{c}");
    assert_eq!(c.matches("GDK_BACKEND").count(), 1, "{c}");
    assert!(!c.contains("code-ovr"), "{c}");
}

#[test]
fn launch_marked_override_without_fix_runs_override() {
    let f = fx(&[], &[CODE]);
    let src = f.sys.join("code.desktop");
    std::fs::write(
        f.home.join("code.desktop"),
        marked(&src.display().to_string()),
    )
    .unwrap();
    let c = cmd(&f, "code", None, None);
    assert!(
        c.ends_with("-- 'env' '--' 'GDK_BACKEND=wayland,x11' 'code-ovr' '--x'"),
        "{c}"
    );
}

#[test]
fn launch_marked_override_with_bad_source_uses_override_as_is() {
    let f = fx(
        &[],
        &[CODE, ("other.desktop", "[Desktop Entry]\nExec=other\n")],
    );
    let outside = f._t.path().join("elsewhere/applications");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("code.desktop"), CODE.1).unwrap();
    std::fs::create_dir_all(f.sys.join("dir.desktop")).unwrap();
    let as_is = "-- 'env' '--' 'GDK_BACKEND=wayland,x11' 'code-ovr' '--x'";
    let bad = [
        outside.join("code.desktop"),
        f.home.join("code.desktop"),
        f.sys.join("other.desktop"),
        f.sys.join("missing.desktop"),
        f.sys.join("dir.desktop"),
        f.sys.join("../applications/code.desktop"),
        PathBuf::from("applications/code.desktop"),
    ];
    for src in bad {
        std::fs::write(
            f.home.join("code.desktop"),
            marked(&src.display().to_string()),
        )
        .unwrap();
        let c = cmd(&f, "code", Some(&wayland("code", &["--x"])), None);
        assert!(c.ends_with(as_is), "{}: {c}", src.display());
        assert_eq!(c.matches("GDK_BACKEND").count(), 1, "{c}");
    }
    // A source that is itself a marked override is never fixed again.
    std::fs::write(f.sys.join("code.desktop"), marked("/x")).unwrap();
    let src = f.sys.join("code.desktop");
    std::fs::write(
        f.home.join("code.desktop"),
        marked(&src.display().to_string()),
    )
    .unwrap();
    let c = cmd(&f, "code", Some(&wayland("code", &["--x"])), None);
    assert_eq!(c.matches("GDK_BACKEND").count(), 1, "{c}");
}

#[test]
fn valid_source_checks_dir_id_and_file() {
    let f = fx(&[CODE], &[CODE]);
    let sys = f.sys.join("code.desktop");
    assert_eq!(
        valid_source(&f.dirs, "code", &sys.display().to_string()),
        Some(sys.clone())
    );
    assert_eq!(
        valid_source(&f.dirs, "code.desktop", &sys.display().to_string()),
        Some(sys.clone())
    );
    assert_eq!(
        valid_source(&f.dirs, "other", &sys.display().to_string()),
        None
    );
    let home = f.home.join("code.desktop").display().to_string();
    assert_eq!(valid_source(&f.dirs, "code", &home), None);
    assert_eq!(
        valid_source(&f.dirs, "../code", &sys.display().to_string()),
        None
    );
    // A data dir that is also the data home never counts.
    let same = Dirs::new(Some(f.sys.clone()), vec![f.sys.clone()]);
    assert_eq!(
        valid_source(&same, "code", &sys.display().to_string()),
        None
    );
}

#[test]
fn launch_fix_refused_for_equals_program() {
    let f = fx(&[], &[("w.desktop", "[Desktop Entry]\nExec=FOO=1 app\n")]);
    let c = cmd(&f, "w", Some(&wayland("w", &["--x"])), None);
    assert!(c.ends_with("-- 'FOO=1' 'app'"), "{c}");
    assert!(!c.contains("'env'"), "{c}");
}

#[test]
fn launch_icon_field_code() {
    let f = fx(
        &[],
        &[(
            "i.desktop",
            "[Desktop Entry]\nExec=app %i %c %k\nIcon=x\nName=App One\n",
        )],
    );
    let path = f.sys.join("i.desktop").display().to_string();
    let c = cmd(&f, "i", None, None);
    assert!(
        c.ends_with(&format!("'app' '--icon' 'x' 'App One' '{path}'")),
        "{c}"
    );
    // No icon: `%i` expands to nothing.
    let f = fx(&[], &[("j.desktop", "[Desktop Entry]\nExec=app %i\n")]);
    assert!(cmd(&f, "j", None, None).ends_with("-- 'app'"));
}
