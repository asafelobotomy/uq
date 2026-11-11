use super::*;
use std::path::Path;
use uq_core::autostart::FileChoice;
use uq_core::autostart::scan::read_folders;
use uq_core::compat::launch::OVERRIDE_HEADER;
use uq_core::compat::{AppFix, Platform};
use uq_core::env::{Env, Var, environment_d_lines};
use uq_core::startup_explain::OverrideState;

const CODE: &str = "code.desktop";
const SOURCE: &str = "[Desktop Entry]\nType=Application\nName=Code\nExec=/usr/bin/code %F\n";
const MINE: &str = "[Desktop Entry]\nType=Application\nName=Mine\nExec=mine\n";
const APPLET: &str = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";

struct Dirs {
    _dir: tempfile::TempDir,
    root: PathBuf,
    p: StartupPaths,
}

fn dirs() -> Dirs {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let p = StartupPaths::under(&root);
    Dirs { _dir: dir, root, p }
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn vars(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect()
}

/// No leftover temp files next to `path`.
fn no_temps(path: &Path) {
    let dir = path.parent().unwrap();
    for e in std::fs::read_dir(dir).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        assert!(!name.contains(".uq-") && !name.contains(".tmp-"), "{name}");
    }
}

// --- environment.d ---

#[test]
fn environment_d_written_and_rewritten() {
    let d = dirs();
    let path = environment_d_path(&d.p);
    write_environment_d(&d.p, &[]).unwrap();
    assert!(!present(&path), "no file without lines");

    write_environment_d(&d.p, &vars(&[("FOO", "a")])).unwrap();
    let t = read(&path);
    assert!(t.starts_with(ENVIRONMENT_D_HEADER), "{t}");
    assert!(t.contains("FOO='a'\n"), "{t}");
    write_environment_d(&d.p, &vars(&[("FOO", "b"), ("BAR", "it's")])).unwrap();
    let t = read(&path);
    assert!(
        t.contains("FOO='b'\n") && t.contains("BAR=\"it's\"\n"),
        "{t}"
    );
    assert!(!t.contains("FOO='a'"));
    no_temps(&path);

    write_environment_d(&d.p, &[]).unwrap();
    assert!(!present(&path), "uQ's file goes when the list is empty");

    put(&path, "MINE=1\n");
    write_environment_d(&d.p, &[]).unwrap();
    assert_eq!(read(&path), "MINE=1\n", "a file without the header stays");
    write_environment_d(&d.p, &vars(&[("FOO", "a")])).unwrap();
    assert!(
        read(&path).starts_with(ENVIRONMENT_D_HEADER),
        "uqd owns the name"
    );
}

#[test]
fn environment_d_has_no_scale_lines() {
    let d = dirs();
    let env = Env {
        vars: vec![Var {
            name: "FOO".into(),
            value: "1".into(),
        }],
    };
    let compat = Compat {
        sharp_x11: true,
        steam_scale: true,
        qt_wayland: true,
        ..Compat::default()
    };
    write_environment_d(&d.p, &environment_d_lines(&env, &compat)).unwrap();
    let t = read(&environment_d_path(&d.p));
    assert!(
        t.contains("FOO='1'") && t.contains("QT_QPA_PLATFORM="),
        "{t}"
    );
    assert!(
        !t.contains("GDK_SCALE") && !t.contains("STEAM_FORCE"),
        "{t}"
    );
}

// --- flag files ---

fn chromium_on() -> Compat {
    Compat {
        chromium_wayland: true,
        ..Compat::default()
    }
}

fn only(ids: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |id| ids.contains(&id)
}

fn flags(d: &Dirs, name: &str) -> PathBuf {
    d.p.config_home.join(name)
}

#[test]
fn flags_written_only_for_installed_browsers() {
    let d = dirs();
    let r = sync_flags(&d.p, &chromium_on(), &only(&["brave-browser.desktop"]));
    assert_eq!(r, vec![("Brave".to_string(), BrowserFile::Ours)]);
    assert_eq!(read(&flags(&d, "brave-flags.conf")), flags_text());
    assert!(!present(&flags(&d, "chromium-flags.conf")));
    no_temps(&flags(&d, "brave-flags.conf"));
}

#[test]
fn flags_for_browser_installed_later() {
    let d = dirs();
    sync_flags(&d.p, &chromium_on(), &only(&[]));
    assert!(!present(&flags(&d, "chromium-flags.conf")));
    let r = sync_flags(&d.p, &chromium_on(), &only(&["chromium.desktop"]));
    assert_eq!(r, vec![("Chromium".to_string(), BrowserFile::Ours)]);
    assert_eq!(read(&flags(&d, "chromium-flags.conf")), flags_text());
}

#[test]
fn never_touches_unmarked_flags_file() {
    let d = dirs();
    let path = flags(&d, "brave-flags.conf");
    put(&path, "--my-flag\n");
    let installed = only(&["brave-browser.desktop"]);
    let r = sync_flags(&d.p, &chromium_on(), &installed);
    assert_eq!(r, vec![("Brave".to_string(), BrowserFile::OwnFile)]);
    assert_eq!(read(&path), "--my-flag\n");
    sync_flags(&d.p, &Compat::default(), &installed);
    assert_eq!(read(&path), "--my-flag\n");
}

#[test]
fn flags_removed_when_off() {
    let d = dirs();
    let installed = only(&["chromium.desktop", "vivaldi-stable.desktop"]);
    sync_flags(&d.p, &chromium_on(), &installed);
    assert!(present(&flags(&d, "vivaldi-stable.conf")));
    let r = sync_flags(&d.p, &Compat::default(), &installed);
    assert!(r.iter().all(|(_, f)| *f == BrowserFile::None), "{r:?}");
    assert!(!present(&flags(&d, "chromium-flags.conf")));
    assert!(!present(&flags(&d, "vivaldi-stable.conf")));
}

// --- overrides ---

fn source(d: &Dirs) -> PathBuf {
    d.root.join("share-dirs/applications").join(CODE)
}

fn target(d: &Dirs) -> PathBuf {
    d.p.data_home.join("applications").join(CODE)
}

fn fix(elsewhere: bool) -> Compat {
    let mut f = AppFix::new(CODE);
    f.platform = Platform::Wayland;
    f.elsewhere = elsewhere;
    Compat {
        apps: vec![f],
        ..Compat::default()
    }
}

fn state_of(r: &BTreeMap<String, OverrideState>) -> OverrideState {
    r.get(CODE).cloned().unwrap()
}

use std::collections::BTreeMap;

#[test]
fn override_written_with_marker_and_source() {
    let d = dirs();
    put(&source(&d), SOURCE);
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::Written);
    let t = read(&target(&d));
    assert!(t.starts_with(OVERRIDE_HEADER), "{t}");
    assert!(t.contains("X-uQ-Override=true"), "{t}");
    assert!(
        t.contains(&format!("X-uQ-Source={}", source(&d).display())),
        "{t}"
    );
    assert!(t.contains("Exec=env -- "), "{t}");
    assert!(is_ours(&target(&d), MarkerKind::Override));
    no_temps(&target(&d));
}

#[test]
fn override_regenerated_when_source_changes() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    put(
        &source(&d),
        &SOURCE.replace("/usr/bin/code", "/opt/code/bin/code"),
    );
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::Written);
    let t = read(&target(&d));
    assert!(
        t.contains("/opt/code/bin/code") && !t.contains("/usr/bin/code"),
        "{t}"
    );
}

#[test]
fn override_deleted_when_fix_removed() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    assert!(present(&target(&d)));
    let r = sync_overrides(&d.p, &fix(false), false);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert!(!present(&target(&d)), "elsewhere turned off");

    sync_overrides(&d.p, &fix(true), false);
    assert!(present(&target(&d)));
    assert!(remove_overrides(&d.p, [CODE], false).is_empty());
    assert!(!present(&target(&d)), "the fix removed");

    sync_overrides(&d.p, &fix(true), false);
    assert_eq!(marked_overrides(&d.p), vec![CODE.to_string()]);
}

#[test]
fn override_lossy_source_reported() {
    let d = dirs();
    put(&source(&d), &SOURCE.replace("%F", "--file=%f"));
    let r = sync_overrides(&d.p, &fix(true), false);
    assert!(matches!(state_of(&r), OverrideState::Error(_)), "{r:?}");
    assert!(!present(&target(&d)));
}

#[test]
fn override_without_source_reported() {
    let d = dirs();
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::NoSource);
    assert!(!present(&target(&d)));
}

#[test]
fn never_touches_unmarked_override() {
    let d = dirs();
    put(&source(&d), SOURCE);
    put(&target(&d), MINE);
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    sync_overrides(&d.p, &fix(false), false);
    remove_overrides(&d.p, [CODE], false);
    assert_eq!(read(&target(&d)), MINE);
}

#[test]
fn never_follows_override_symlink() {
    let d = dirs();
    put(&source(&d), SOURCE);
    // A marked file outside the folder, linked in: a symlink is never uQ's.
    let outside = d.root.join("elsewhere.desktop");
    let marked = format!("{OVERRIDE_HEADER}\n{SOURCE}X-uQ-Override=true\n");
    put(&outside, &marked);
    std::fs::create_dir_all(target(&d).parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, target(&d)).unwrap();
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    sync_overrides(&d.p, &fix(false), false);
    remove_overrides(&d.p, [CODE], false);
    let meta = std::fs::symlink_metadata(target(&d)).unwrap();
    assert!(meta.file_type().is_symlink());
    assert_eq!(read(&outside), marked);
}

#[test]
fn is_ours_never_reads_a_symlink_swapped_in_after_the_check() {
    let d = dirs();
    let marked = format!("{OVERRIDE_HEADER}\n{SOURCE}X-uQ-Override=true\n");
    let outside = d.root.join("elsewhere.desktop");
    put(&outside, &marked);
    put(&target(&d), &marked);
    assert!(is_ours(&target(&d), MarkerKind::Override));
    // Between the no-follow check and the read, the file becomes a symlink to a marked file.
    let to = outside.clone();
    hook::on_read(move |p| {
        std::fs::remove_file(p).unwrap();
        std::os::unix::fs::symlink(&to, p).unwrap();
    });
    assert!(!is_ours(&target(&d), MarkerKind::Override));
    let meta = std::fs::symlink_metadata(target(&d)).unwrap();
    assert!(meta.file_type().is_symlink());
}

#[test]
fn never_deletes_file_swapped_after_check() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    hook::once(|p| std::fs::write(p, MINE).unwrap());
    let r = sync_overrides(&d.p, &fix(false), false);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert_eq!(read(&target(&d)), MINE);
}

#[test]
fn never_replaces_file_swapped_after_check() {
    // A marked file, swapped for the user's own between the check and the write.
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    put(&source(&d), &SOURCE.replace("/usr/bin/code", "/opt/code"));
    hook::once(|p| {
        std::fs::remove_file(p).unwrap();
        std::fs::write(p, MINE).unwrap();
    });
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    no_temps(&target(&d));

    // A file that appeared between the check and the write.
    let d = dirs();
    put(&source(&d), SOURCE);
    hook::once(|p| std::fs::write(p, MINE).unwrap());
    let r = sync_overrides(&d.p, &fix(true), false);
    assert_eq!(state_of(&r), OverrideState::OwnFile);
    assert_eq!(read(&target(&d)), MINE);
    no_temps(&target(&d));
}

#[test]
fn override_path_stays_in_folder() {
    let d = dirs();
    let dir = d.p.data_home.join("applications");
    assert_eq!(safe_join(&dir, CODE), Some(dir.join(CODE)));
    for bad in [
        "../x.desktop",
        "a/b.desktop",
        "/etc/x.desktop",
        "..desktop",
        ".x.desktop",
        "",
    ] {
        assert_eq!(safe_join(&dir, bad), None, "{bad}");
    }
    assert_eq!(override_target(&d.p, "../../evil.desktop"), None);
}

#[test]
fn dev_session_override_sync_touches_nothing() {
    let d = dirs();
    put(&source(&d), SOURCE);
    sync_overrides(&d.p, &fix(true), false);
    let before = read(&target(&d));
    let r = sync_overrides(&d.p, &fix(false), true);
    assert_eq!(state_of(&r), OverrideState::Off);
    assert!(remove_overrides(&d.p, [CODE], true).is_empty());
    assert_eq!(read(&target(&d)), before, "a marked override stays");

    let d = dirs();
    put(&source(&d), SOURCE);
    let r = sync_overrides(&d.p, &fix(true), true);
    assert!(matches!(state_of(&r), OverrideState::Error(_)), "{r:?}");
    assert!(!present(&target(&d)), "none is written");
}

// --- hidden autostart copies ---

fn system_entry(d: &Dirs, id: &str, text: &str) {
    put(&d.root.join("xdg/autostart").join(id), text);
}

fn user_entry(d: &Dirs, id: &str) -> PathBuf {
    d.p.user_autostart().join(id)
}

fn found(d: &Dirs) -> Vec<uq_core::autostart::scan::Found> {
    read_folders(&d.p.user_autostart(), &d.p.system_autostart())
}

fn off(id: &str) -> Autostart {
    Autostart {
        files: vec![FileChoice {
            id: id.into(),
            enabled: Some(false),
            delay: None,
        }],
        added: Vec::new(),
    }
}

use uq_core::autostart::Autostart;

#[test]
fn hidden_copy_written_and_removed() {
    let d = dirs();
    let id = "nm-applet.desktop";
    system_entry(&d, id, APPLET);
    assert_eq!(sync_hidden_copies(&d.p, &found(&d), &off(id)), None);
    let t = read(&user_entry(&d, id));
    assert!(
        t.contains("Hidden=true") && t.contains("X-uQ-Autostart=true"),
        "{t}"
    );
    no_temps(&user_entry(&d, id));
    assert_eq!(
        sync_hidden_copies(&d.p, &found(&d), &Autostart::default()),
        None
    );
    assert!(!present(&user_entry(&d, id)));
}

#[test]
fn never_touches_unmarked_autostart_file() {
    let d = dirs();
    let id = "nm-applet.desktop";
    system_entry(&d, id, APPLET);
    put(&user_entry(&d, id), MINE);
    sync_hidden_copies(&d.p, &found(&d), &off(id));
    assert_eq!(read(&user_entry(&d, id)), MINE);
    sync_hidden_copies(&d.p, &found(&d), &Autostart::default());
    assert_eq!(read(&user_entry(&d, id)), MINE);
}

#[test]
fn orphan_copy_removed() {
    let d = dirs();
    let id = "gone.desktop";
    system_entry(&d, id, APPLET);
    sync_hidden_copies(&d.p, &found(&d), &off(id));
    assert!(present(&user_entry(&d, id)));
    std::fs::remove_file(d.root.join("xdg/autostart").join(id)).unwrap();
    assert_eq!(sync_hidden_copies(&d.p, &found(&d), &off(id)), None);
    assert!(!present(&user_entry(&d, id)));
}
