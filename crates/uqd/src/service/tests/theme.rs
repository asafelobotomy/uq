//! Themes: the files a theme writes and what it restarts or tells.

use super::*;

#[tokio::test]
async fn set_theme_writes_files_and_emits() {
    let mut r = rig();
    r.desktop
        .set_theme("dracula", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    let tokens = fs::read_to_string(p.theme_dir.join("tokens.json")).unwrap();
    assert!(tokens.contains(r#""id": "dracula""#), "{tokens}");
    let generated = fs::read_to_string(&p.generated_lua).unwrap();
    assert!(generated.contains("Dracula (light)"), "{generated}");
    assert_eq!(State::load(&p.state_file).theme, "dracula");
    assert_eq!(
        r.desktop.get_theme(),
        ("dracula".to_string(), Variant::Light)
    );
    assert!(r.comp.calls().contains(&"reload".to_string()));
    assert_eq!(
        *r.sys.applied.lock().unwrap(),
        vec!["dracula light".to_string()]
    );
    assert_eq!(
        drain(&mut r.events),
        vec![Event::ThemeChanged {
            name: "dracula".into(),
            variant: Variant::Light,
            tokens_path: p.theme_dir.join("tokens.json")
        }]
    );
}

#[tokio::test]
async fn shell_restarts_only_when_its_icon_theme_changes() {
    use std::sync::atomic::Ordering::SeqCst;
    let mut r = rig();
    // First render: no shell.env yet, so the running shell is left alone.
    r.desktop
        .set_theme("dracula", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let env = fs::read_to_string(r.desktop.paths().theme_dir.join("shell.env")).unwrap();
    assert!(env.contains("QS_ICON_THEME=\"Papirus-Dark\""), "{env}");
    assert_eq!(r.sys.shell_restarts.load(SeqCst), 0);
    // Another dark theme: same icons, no restart.
    r.desktop
        .set_theme("gruvbox", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.sys.shell_restarts.load(SeqCst), 0);
    // Light: Papirus-Light, so the shell restarts to pick it up.
    r.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.sys.shell_restarts.load(SeqCst), 1);
}

#[tokio::test]
async fn qt6ct_icon_theme_follows_the_theme_while_hyprland_runs() {
    let mut r = rig();
    let p = r.desktop.paths().clone();
    let conf = uq_core::qt6ct::conf_path(&p);
    let ours = format!(
        "[Appearance]\ncolor_scheme_path={}\nstyle=kvantum\n",
        p.theme_dir.join("qt6ct/colors/uq.conf").display()
    );
    uq_core::atomic::atomic_write(&conf, &ours).unwrap();
    r.desktop
        .set_theme("gruvbox", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    assert!(
        fs::read_to_string(&conf)
            .unwrap()
            .contains("icon_theme=Papirus-Dark\n")
    );
    r.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    let body = fs::read_to_string(&conf).unwrap();
    assert!(body.contains("icon_theme=Papirus-Light\n"), "{body}");
    assert_eq!(body.matches("icon_theme=").count(), 1, "{body}");

    // Without Hyprland (a uqd left running in another desktop) the file isn't touched.
    let mut off = rig_with(false, |_| {});
    let p = off.desktop.paths().clone();
    let conf = uq_core::qt6ct::conf_path(&p);
    let ours = format!(
        "[Appearance]\ncolor_scheme_path={}\n",
        p.theme_dir.join("qt6ct/colors/uq.conf").display()
    );
    uq_core::atomic::atomic_write(&conf, &ours).unwrap();
    off.desktop
        .set_theme("matcha", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(fs::read_to_string(&conf).unwrap(), ours);
}

#[tokio::test]
async fn apps_files_another_app_rewrote_are_themed_again_unless_switched_off() {
    let mut r = rig();
    let p = r.desktop.paths().clone();
    let conf = uq_core::qt6ct::conf_path(&p);
    // What pressing OK in the qt6ct app leaves: Fusion, its own colours, DejaVu.
    let theirs = "[Appearance]\ncolor_scheme_path=/usr/share/qt6ct/colors/darker.conf\ncustom_palette=true\nstyle=Fusion\n\n[Fonts]\ngeneral=\"DejaVu LGC Sans,12,-1,5,400,0,0,0,0,0,0,0,0,0,0,1\"\n";
    uq_core::atomic::atomic_write(&conf, theirs).unwrap();
    let kg = uq_core::kdeglobals::conf_path(&p);
    let breeze =
        "[General]\nColorScheme=BreezeLight\n\n[Colors:Window]\nBackgroundNormal=239,240,241\n";
    uq_core::atomic::atomic_write(&kg, breeze).unwrap();
    r.desktop.startup().await;
    let body = fs::read_to_string(&conf).unwrap();
    assert!(uq_core::qt6ct::is_opted_in(&p), "{body}");
    assert!(
        !body.contains("darker.conf") && !body.contains("DejaVu"),
        "{body}"
    );
    assert!(body.contains("icon_theme="), "{body}");
    assert_eq!(
        fs::read_to_string(conf.with_file_name("qt6ct.conf.before-uq")).unwrap(),
        theirs
    );
    let body = fs::read_to_string(&kg).unwrap();
    assert!(
        body.contains("ColorScheme=uQ") && !body.contains("239,240,241"),
        "{body}"
    );
    assert_eq!(
        fs::read_to_string(kg.with_file_name("kdeglobals.before-uq")).unwrap(),
        breeze
    );

    // Switched off: their files are left as they are.
    r.desktop
        .set_section("appearance", r#"{"theme_apps":false}"#, Origin::Settings)
        .await
        .unwrap();
    uq_core::atomic::atomic_write(&conf, theirs).unwrap();
    uq_core::atomic::atomic_write(&kg, breeze).unwrap();
    r.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(fs::read_to_string(&conf).unwrap(), theirs);
    assert_eq!(fs::read_to_string(&kg).unwrap(), breeze);
}

#[tokio::test]
async fn kde_terminal_is_named_while_hyprland_runs_and_never_replaced() {
    let installed = |bin: &str| {
        std::env::var_os("PATH")
            .is_some_and(|ps| std::env::split_paths(&ps).any(|d| d.join(bin).is_file()))
    };
    // kyko where it is installed, else kitty, else nothing.
    let wanted = uq_core::kdeglobals::TERMINAL_APPS
        .into_iter()
        .find(|t| installed(t));
    let mut r = rig();
    r.desktop.startup().await;
    let conf = uq_core::kdeglobals::conf_path(r.desktop.paths());
    let body = fs::read_to_string(&conf).unwrap();
    match wanted {
        Some(t) => assert!(body.contains(&format!("TerminalApplication={t}")), "{body}"),
        None => assert!(!body.contains("TerminalApplication"), "{body}"),
    }
    // A terminal the user chose stays.
    fs::write(&conf, "[General]\nTerminalApplication=konsole\n").unwrap();
    r.desktop
        .set_theme("gruvbox", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let body = fs::read_to_string(&conf).unwrap();
    assert!(body.contains("TerminalApplication=konsole"), "{body}");
    assert!(!body.contains("kitty"), "{body}");

    // Without Hyprland (a uqd left running in another desktop) nothing is written.
    let mut off = rig_with(false, |_| {});
    off.desktop.startup().await;
    assert!(!uq_core::kdeglobals::conf_path(off.desktop.paths()).exists());
}

#[tokio::test]
async fn the_terminal_chosen_in_settings_reaches_kde_and_gtk_apps() {
    let mut r = rig();
    r.desktop.startup().await;
    let p = r.desktop.paths().clone();
    let apps = p.startup.app_dirs()[0].clone();
    fs::create_dir_all(&apps).unwrap();
    fs::write(
        apps.join("foot.desktop"),
        "[Desktop Entry]\nType=Application\nName=Foot\nExec=foot\n",
    )
    .unwrap();
    let kde = uq_core::kdeglobals::conf_path(&p);
    let list = uq_core::terminal_pref::list_path(&p);
    r.desktop
        .set_section("shell", r#"{"terminal":"foot.desktop"}"#, Origin::Settings)
        .await
        .unwrap();
    let body = fs::read_to_string(&kde).unwrap();
    assert!(
        body.contains("TerminalService=foot.desktop\nTerminalApplication=foot\n"),
        "{body}"
    );
    assert_eq!(fs::read_to_string(&list).unwrap(), "foot.desktop\n");

    // Back to uQ's default: foot goes from both.
    r.desktop
        .set_section("shell", r#"{"terminal":null}"#, Origin::Settings)
        .await
        .unwrap();
    assert!(!fs::read_to_string(&kde).unwrap().contains("foot"));
    assert!(
        !fs::read_to_string(&list)
            .unwrap_or_default()
            .contains("foot")
    );
}

#[tokio::test]
async fn kdeglobals_carries_uqs_look_and_follows_the_theme() {
    let mut r = rig();
    r.desktop
        .set_theme("gruvbox", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let conf = uq_core::kdeglobals::conf_path(r.desktop.paths());
    let body = fs::read_to_string(&conf).unwrap();
    assert!(body.contains("ColorScheme=uQ"), "{body}");
    assert!(body.contains("[Icons]\nTheme=Papirus-Dark"), "{body}");
    assert!(body.contains("widgetStyle="), "{body}");
    assert!(body.contains("[Colors:View]"), "{body}");
    r.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    let body = fs::read_to_string(&conf).unwrap();
    assert!(body.contains("Theme=Papirus-Light"), "{body}");
    assert_eq!(body.matches("[Icons]").count(), 1, "{body}");
}

#[tokio::test]
async fn open_kde_apps_are_told_when_kdeglobals_changes() {
    use std::sync::atomic::Ordering::SeqCst;
    let mut r = rig();
    r.desktop
        .set_theme("gruvbox", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let after_first = r.sys.kde_notifies.load(SeqCst);
    assert!(after_first >= 1, "the first look written is announced");
    r.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.sys.kde_notifies.load(SeqCst), after_first + 1);

    // Without Hyprland nothing is written, so nothing is announced.
    let mut off = rig_with(false, |_| {});
    off.desktop
        .set_theme("gruvbox", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(off.sys.kde_notifies.load(SeqCst), 0);
}

#[tokio::test]
async fn no_shell_restart_without_hyprland() {
    use std::sync::atomic::Ordering::SeqCst;
    let mut r = rig_with(false, |_| {});
    for v in [Variant::Dark, Variant::Light] {
        r.desktop
            .set_theme("matcha", v, Origin::Settings)
            .await
            .unwrap();
    }
    assert_eq!(r.sys.shell_restarts.load(SeqCst), 0);
}

#[tokio::test]
async fn no_system_settings_without_hyprland() {
    let mut r = rig_with(false, |_| {});
    r.desktop
        .set_theme("matcha", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    assert!(
        r.sys.applied.lock().unwrap().is_empty(),
        "gsettings touched outside Hyprland"
    );
}

#[tokio::test]
async fn unknown_theme_rejected() {
    let mut r = rig();
    let err = r
        .desktop
        .set_theme("solarized", Variant::Dark, Origin::Settings)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::UnknownTheme(ref id) if id == "solarized"));
}

#[tokio::test]
async fn stale_theme_in_state_falls_back_to_default() {
    let r = rig_with(true, |p| {
        let s = State {
            theme: "removed".into(),
            ..State::default()
        };
        s.save(&p.state_file).unwrap();
    });
    assert_eq!(
        r.desktop.get_theme(),
        ("graphite".to_string(), Variant::Dark)
    );
}
