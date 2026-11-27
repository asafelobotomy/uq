//! Logind, greeter, banner and auto-login writes, and what callers may do.

use super::*;

#[tokio::test]
async fn logind_writes_the_template_after_polkit_power() {
    let f = Fixture::new(true);
    f.helper
        .set_logind_power_actions(":1.5", "hibernate", "ignore", "lock")
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.path("etc/systemd/logind.conf.d/50-uq.conf")).unwrap(),
        templates::logind(
            PowerAction::Hibernate,
            PowerAction::Ignore,
            PowerAction::Lock
        )
    );
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Power)]);
}

#[tokio::test]
async fn logind_rejects_anything_outside_the_set_before_asking_polkit() {
    let f = Fixture::new(true);
    for bad in HOSTILE
        .iter()
        .chain(&["reboot", "Suspend", "suspend\n[Login]"])
    {
        for args in [
            (*bad, "suspend", "suspend"),
            ("suspend", *bad, "suspend"),
            ("suspend", "suspend", *bad),
        ] {
            let r = f
                .helper
                .set_logind_power_actions(":1.5", args.0, args.1, args.2)
                .await;
            assert!(is_invalid(r), "{args:?}");
        }
    }
    assert!(f.checks().is_empty());
    assert!(f.written().is_empty());
}

#[tokio::test]
async fn the_startup_banner_switch_is_a_marker_file() {
    let f = Fixture::new(true);
    let marker = f.path("etc/uq/banner-off");
    f.helper.set_startup_banner(":1.5", false).await.unwrap();
    assert!(marker.is_file());
    // Off twice, then on, then on again: each is fine.
    f.helper.set_startup_banner(":1.5", false).await.unwrap();
    f.helper.set_startup_banner(":1.5", true).await.unwrap();
    assert!(!marker.exists());
    f.helper.set_startup_banner(":1.5", true).await.unwrap();
    assert!(f.checks().iter().all(|c| c.1 == Action::Greeter));
    assert_eq!(f.checks().len(), 4);
}

#[tokio::test]
async fn sync_greeter_theme_keeps_the_other_settings() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_appearance(":1.5", "dracula", "light", true, true)
        .await
        .unwrap();
    f.helper
        .sync_greeter_theme(":1.5", "forest", "dark")
        .await
        .unwrap();
    let t: toml::Table = fs::read_to_string(f.path("etc/uq/greeter.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(t["theme"].as_str(), Some("forest"));
    assert_eq!(t["variant"].as_str(), Some("dark"));
    assert_eq!(t["hide_users"].as_bool(), Some(true));
    assert_eq!(t["reduce_motion"].as_bool(), Some(true));
    assert_eq!(
        f.checks().last(),
        Some(&(":1.5".to_string(), Action::GreeterTheme))
    );
    // A theme that isn't shipped is refused before polkit is asked.
    let n = f.checks().len();
    assert!(
        f.helper
            .sync_greeter_theme(":1.5", "ghost", "dark")
            .await
            .is_err()
    );
    assert_eq!(f.checks().len(), n);
}

#[tokio::test]
async fn greeter_appearance_needs_a_shipped_variant() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_appearance(":1.5", "dracula", "light", true, true)
        .await
        .unwrap();
    let t: toml::Table = fs::read_to_string(f.path("etc/uq/greeter.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(t["theme"].as_str(), Some("dracula"));
    assert_eq!(t["variant"].as_str(), Some("light"));
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Greeter)]);

    // Valid names that aren't shipped, or are shipped without tokens.json.
    for (theme, variant) in [
        ("nope", "dark"),
        ("forest", "light"),
        ("dracula-dark", "dark"),
    ] {
        let r = f
            .helper
            .set_greeter_appearance(":1.5", theme, variant, false, false)
            .await;
        assert!(is_invalid(r), "{theme}-{variant}");
    }
    for bad in HOSTILE.iter().chain(&["../rendered/dracula", "Dracula"]) {
        assert!(is_invalid(
            f.helper
                .set_greeter_appearance(":1.5", bad, "dark", false, false)
                .await
        ));
        assert!(is_invalid(
            f.helper
                .set_greeter_appearance(":1.5", "dracula", bad, false, false)
                .await
        ));
    }
    assert_eq!(f.checks().len(), 1);
}

#[tokio::test]
async fn greeter_appearance_ignores_a_tokens_symlink_to_nothing() {
    let f = Fixture::new(true);
    let d = f.path("usr/share/uq/themes/rendered/ghost-dark");
    fs::create_dir_all(&d).unwrap();
    std::os::unix::fs::symlink("/nonexistent/x", d.join("tokens.json")).unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_appearance(":1.5", "ghost", "dark", false, false)
            .await
    ));
}

#[tokio::test]
async fn auto_login_sets_and_removes_initial_session_only() {
    let f = Fixture::new(true);
    f.helper.set_auto_login(":1.5", "alice").await.unwrap();
    let text = fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap();
    assert!(text.starts_with(GREETD));
    let t: toml::Table = text.parse().unwrap();
    assert_eq!(t["initial_session"]["user"].as_str(), Some("alice"));
    assert_eq!(
        t["initial_session"]["command"].as_str(),
        Some("/usr/bin/uq-session")
    );
    f.helper.set_auto_login(":1.5", "bob").await.unwrap();
    let t: toml::Table = fs::read_to_string(f.path("etc/uq/greetd.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(t["initial_session"]["user"].as_str(), Some("bob"));
    f.helper.set_auto_login(":1.5", "").await.unwrap();
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::AutoLogin; 3]
    );
}

#[tokio::test]
async fn auto_login_refuses_system_missing_and_malformed_users() {
    let f = Fixture::new(true);
    for bad in ["root", "daemon", "nobody", "highuid", "svc", "ghost"] {
        assert!(
            is_invalid(f.helper.set_auto_login(":1.5", bad).await),
            "{bad}"
        );
    }
    for bad in HOSTILE.iter().filter(|s| !s.is_empty()).chain(&[
        "Alice",
        "alice\n",
        "alice\nuser = \"root\"",
        "alice\"",
        "1alice",
    ]) {
        assert!(
            is_invalid(f.helper.set_auto_login(":1.5", bad).await),
            "{bad:?}"
        );
    }
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "brokenlookup").await,
        Err(HelperError::Failed(_))
    ));
    assert!(f.checks().is_empty());
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
}

#[tokio::test]
async fn auto_login_refuses_locked_and_expired_accounts() {
    let f = Fixture::new(true);
    for bad in ["locked", "expired"] {
        let r = f.helper.set_auto_login(":1.5", bad).await;
        assert!(is_invalid(r), "{bad}");
    }
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "brokenshadow").await,
        Err(HelperError::Failed(_))
    ));
    assert!(f.checks().is_empty(), "refused before polkit is asked");
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        GREETD
    );
    // An account with a usable shadow entry, and one without any (bob), are fine.
    f.helper.set_auto_login(":1.5", "alice").await.unwrap();
    f.helper.set_auto_login(":1.5", "bob").await.unwrap();
}

#[tokio::test]
async fn auto_login_needs_an_existing_valid_greetd_toml() {
    let f = Fixture::new(true);
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert!(!f.path("etc/uq/greetd.toml").exists());

    fs::write(f.path("etc/uq/greetd.toml"), "[default_session\n").unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert_eq!(
        fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
        "[default_session\n"
    );

    // A symlink there is not followed.
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    let elsewhere = f.path("elsewhere.toml");
    fs::write(&elsewhere, GREETD).unwrap();
    std::os::unix::fs::symlink(&elsewhere, f.path("etc/uq/greetd.toml")).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), GREETD);

    // Oversized.
    fs::remove_file(f.path("etc/uq/greetd.toml")).unwrap();
    let big = format!("{GREETD}# {}\n", "x".repeat(MAX_GREETD_TOML as usize));
    fs::write(f.path("etc/uq/greetd.toml"), &big).unwrap();
    assert!(matches!(
        f.helper.set_auto_login(":1.5", "alice").await,
        Err(HelperError::Failed(_))
    ));
}

/// Every method, with valid arguments, against an authority that refuses (or can't be
/// reached): nothing is written, and each asked for the right action.
#[tokio::test]
async fn an_unauthorised_caller_writes_nothing() {
    for auth in [
        MockAuthority::default(),
        MockAuthority {
            fail: true,
            ..Default::default()
        },
    ] {
        let f = Fixture::with(auth);
        // Pre-existing files must survive untouched too.
        fs::create_dir_all(f.path("var/lib/uq-greeter")).unwrap();
        fs::write(f.path("var/lib/uq-greeter/wallpaper"), b"old").unwrap();
        let h = &f.helper;
        assert!(is_denied(
            h.set_logind_power_actions(":1.9", "suspend", "suspend", "poweroff")
                .await
        ));
        assert!(is_denied(
            h.set_greeter_appearance(":1.9", "dracula", "dark", false, false)
                .await
        ));
        assert!(is_denied(
            h.set_greeter_wallpaper(":1.9", fd(tmpfile(b"new"))).await
        ));
        assert!(is_denied(h.clear_greeter_wallpaper(":1.9").await));
        assert!(is_denied(
            h.set_greeter_monitors(":1.9", r#"{"outputs":{}}"#).await
        ));
        assert!(is_denied(h.clear_greeter_monitors(":1.9").await));
        assert!(is_denied(h.set_auto_login(":1.9", "alice").await));
        assert!(is_denied(h.set_auto_login(":1.9", "").await));
        assert_eq!(f.written(), ["var/lib/uq-greeter/wallpaper"]);
        assert_eq!(
            fs::read(f.path("var/lib/uq-greeter/wallpaper")).unwrap(),
            b"old"
        );
        assert_eq!(
            fs::read_to_string(f.path("etc/uq/greetd.toml")).unwrap(),
            GREETD
        );
        let actions: Vec<Action> = f.checks().iter().map(|c| c.1).collect();
        assert_eq!(
            actions,
            [
                Action::Power,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::Greeter,
                Action::AutoLogin,
                Action::AutoLogin
            ]
        );
        assert!(f.checks().iter().all(|c| c.0 == ":1.9"));
    }
}

#[tokio::test]
async fn nothing_lands_outside_the_fixed_paths() {
    let f = Fixture::new(true);
    let h = &f.helper;
    h.set_logind_power_actions(":1.5", "lock", "lock", "lock")
        .await
        .unwrap();
    h.set_greeter_appearance(":1.5", "forest", "dark", false, false)
        .await
        .unwrap();
    h.set_greeter_wallpaper(":1.5", fd(tmpfile(b"w")))
        .await
        .unwrap();
    h.set_greeter_monitors(":1.5", "{}").await.unwrap();
    h.set_auto_login(":1.5", "alice").await.unwrap();
    assert_eq!(
        f.written(),
        [
            "etc/systemd/logind.conf.d/50-uq.conf",
            "etc/uq/greeter-monitors.toml",
            "etc/uq/greeter.toml",
            "var/lib/uq-greeter/wallpaper",
        ]
    );
}
