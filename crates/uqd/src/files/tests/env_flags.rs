//! environment.d, browser flags and hidden autostart copies.

use super::*;

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
