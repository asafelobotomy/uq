//! The run plan, the hidden copies and the ids, over in-memory listings.

use super::*;

#[test]
fn scan_user_file_shadows_system() {
    let f = Found {
        id: "x.desktop".into(),
        user: Some((format!("{USER}/x.desktop").into(), entry("user-x", ""))),
        system: Some((format!("{SYS}/x.desktop").into(), entry("sys-x", ""))),
    };
    let r = one(f, &Autostart::default());
    assert_eq!(r.source, Source::Yours);
    assert!(r.enabled && r.can_toggle);
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: s(&["user-x"]),
            cwd: None,
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
    let r = one(sys("x.desktop", &entry("sys-x", "")), &Autostart::default());
    assert_eq!(r.source, Source::System);
    assert_eq!(r.name, "Thing");
}

#[test]
fn scan_cleans_names_and_icons() {
    let text =
        "[Desktop Entry]\nType=Application\nName=Ev\u{202E}il\u{7}\nIcon=i\u{2066}c\nExec=x\n";
    let r = one(sys("x.desktop", text), &Autostart::default());
    assert_eq!(r.name, "Ev il ");
    assert_eq!(r.icon.as_deref(), Some("i c"));
    let r = one(
        sys(
            "nameless.desktop",
            "[Desktop Entry]\nType=Application\nExec=x\n",
        ),
        &Autostart::default(),
    );
    assert_eq!(r.name, "nameless");
    assert_eq!(r.icon, None);
}

#[test]
fn scan_marked_copy_is_ignored_and_orphan_reported() {
    let text = entry("app", "");
    let copy = hidden_copy(&text);
    let both = Found {
        id: "a.desktop".into(),
        user: Some((format!("{USER}/a.desktop").into(), copy.clone())),
        system: Some((format!("{SYS}/a.desktop").into(), text.clone())),
    };
    // The copy's Hidden=true is uQ's own; the state decides.
    let r = one(both.clone(), &Autostart::default());
    assert_eq!(r.source, Source::System);
    assert!(matches!(r.plan, Plan::Run { .. }), "{r:?}");
    assert_eq!(r.orphan_copy, None);
    let r = one(both, &off("a.desktop"));
    assert_eq!(r.plan, Plan::Off);
    assert!(r.can_toggle && !r.enabled);

    let orphan = user("gone.desktop", &copy);
    let r = one(orphan, &off("gone.desktop"));
    assert_eq!(
        r.orphan_copy,
        Some(PathBuf::from(format!("{USER}/gone.desktop")))
    );
    assert!(matches!(r.plan, Plan::Skip(_)), "{r:?}");
    assert!(!r.can_toggle);
}

#[test]
fn scan_skips_with_plain_reasons() {
    let cases: [(Found, &str); 10] = [
        (
            sys("a.desktop", &entry("a", "Hidden=true\n")),
            "Turned off by the system",
        ),
        (
            user("a.desktop", &entry("a", "Hidden=true\n")),
            "Turned off by a file in your autostart folder",
        ),
        (
            sys("a.desktop", &entry("a", "OnlyShowIn=GNOME;\n")),
            "Not for this desktop",
        ),
        (
            sys("a.desktop", &entry("a", "NotShowIn=KDE;uQ;\n")),
            "Not for this desktop",
        ),
        (
            sys("a.desktop", &entry("a", "TryExec=nope\n")),
            "Not installed",
        ),
        (
            sys("a.desktop", &entry("a", "Terminal=true\n")),
            "Needs a terminal, which uQ doesn't open at login",
        ),
        (
            sys("a.desktop", &entry("app %d", "")),
            "Its command can't be read",
        ),
        (
            sys("a.desktop", &entry("sh -c 'x y'", "")),
            "Its command can't be read",
        ),
        (
            sys("a.desktop", "[Desktop Entry]\nType=Application\nName=A\n"),
            "Its command can't be read",
        ),
        (
            sys(
                "a.desktop",
                "[Desktop Entry]\nType=Link\nName=A\nURL=https://x\n",
            ),
            "Not an app",
        ),
    ];
    for (found, want) in cases {
        let id = found.id.clone();
        let r = resolve(
            &[found],
            &ok(json!({"files": [{"id": id, "enabled": true}]})),
            &Compat::default(),
            &|_| None,
            &|p| p != "nope",
        )
        .remove(0);
        assert_eq!(r.plan, Plan::Skip(want), "{r:?}");
        assert!(!r.can_toggle && !r.enabled, "{r:?}");
    }
    // A match for this desktop runs.
    let r = one(
        sys(
            "a.desktop",
            &entry("a", "OnlyShowIn=GNOME;uQ;\nTryExec=a\n"),
        ),
        &Autostart::default(),
    );
    assert!(matches!(r.plan, Plan::Run { .. }), "{r:?}");
}

#[test]
fn scan_enabled_and_delay_precedence() {
    let gnome_off = sys(
        "a.desktop",
        &entry("a", "X-GNOME-Autostart-enabled=false\n"),
    );
    let r = one(gnome_off.clone(), &Autostart::default());
    assert_eq!(r.plan, Plan::Off);
    assert!(!r.enabled && r.can_toggle);
    let on = ok(json!({"files": [{"id": "a.desktop", "enabled": true}]}));
    assert!(matches!(one(gnome_off, &on).plan, Plan::Run { .. }));

    let slow = sys("a.desktop", &entry("a", "X-GNOME-Autostart-Delay=700\n"));
    let r = one(slow.clone(), &Autostart::default());
    assert_eq!(r.delay, 600);
    assert!(matches!(r.plan, Plan::Run { delay: 600, .. }));
    let quick = ok(json!({"files": [{"id": "a.desktop", "delay": 5}]}));
    let r = one(slow, &quick);
    assert_eq!(r.delay, 5);
    assert!(r.enabled);
    assert!(matches!(r.plan, Plan::Run { delay: 5, .. }));
    // An off entry keeps its delay for the page.
    let r = one(sys("a.desktop", &entry("a", "")), &off("a.desktop"));
    assert_eq!((r.plan, r.delay), (Plan::Off, 0));
}

#[test]
fn scan_applies_compat_fix() {
    let compat =
        Compat::from_section(&json!({"apps": [{"id": "foo.desktop", "platform": "x11"}]})).unwrap();
    let fix = compat.fix_for("foo.desktop");
    let want = crate::compat::launch::argv(fix, "foo.desktop", s(&["foo", "--bar"])).unwrap();
    assert_eq!(want[0], "env");
    let found = [sys("foo.desktop", &entry("foo --bar", "Path=/srv\n"))];
    let r = resolve(&found, &Autostart::default(), &compat, &|_| None, &|_| true).remove(0);
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: want.clone(),
            cwd: Some(PathBuf::from("/srv")),
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
    // An added app gets the same fix.
    let state = ok(added(json!({"name": "Foo", "app": "foo.desktop"})));
    let apps = |id: &str| {
        (id == "foo.desktop").then(|| {
            (
                PathBuf::from("/usr/share/applications/foo.desktop"),
                entry("foo --bar", "Path=relative\n"),
            )
        })
    };
    let r = resolve(&[], &state, &compat, &apps, &|_| true).remove(0);
    assert_eq!(r.source, Source::Added);
    assert_eq!(r.name, "Foo");
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: want,
            cwd: None,
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
    // A command has no desktop id, so no fix.
    let state = ok(cmd(json!(["foo", "--bar"])));
    let r = resolve(&[], &state, &compat, &apps, &|_| true).remove(0);
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: s(&["foo", "--bar"]),
            cwd: None,
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
}

#[test]
fn scan_added_entries() {
    let state = ok(json!({"added": [
        {"id": "a-0001", "name": "Gone", "app": "gone.desktop"},
        {"id": "a-0002", "name": "Off", "argv": ["x"], "enabled": false, "delay": 7},
        {"id": "a-0003", "name": "Term", "app": "term.desktop"}
    ]}));
    let apps = |id: &str| {
        (id == "term.desktop").then(|| {
            (
                PathBuf::from("/a/term.desktop"),
                entry("t", "Terminal=true\n"),
            )
        })
    };
    let r = resolve(&[], &state, &Compat::default(), &apps, &|_| true);
    assert_eq!(r.len(), 3);
    assert_eq!(r[0].plan, Plan::Skip("Not installed"));
    assert_eq!((r[1].id.as_str(), &r[1].plan), ("a-0002", &Plan::Off));
    assert!(r[1].can_toggle && !r[1].enabled);
    assert_eq!(r[1].delay, 7);
    assert_eq!(
        r[2].plan,
        Plan::Skip("Needs a terminal, which uQ doesn't open at login")
    );
    assert!(r.iter().all(|x| x.source == Source::Added));
}

#[test]
fn scan_expands_field_codes() {
    let text = "[Desktop Entry]\nType=Application\nName=Thing\nIcon=ic\nExec=foo %k %f %i %c %U\n";
    let r = one(sys("a.desktop", text), &Autostart::default());
    let path = format!("{SYS}/a.desktop");
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: s(&["foo", &path, "--icon", "ic", "Thing"]),
            cwd: None,
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
}

#[test]
fn known_ids_cover_found_and_added() {
    let state = ok(json!({"added": [{"id": "a-0001", "name": "X", "argv": ["x"]}]}));
    let ids = known_ids(&[sys("a.desktop", "x"), user("b.desktop", "y")], &state);
    assert_eq!(
        ids.into_iter().collect::<Vec<_>>(),
        s(&["a-0001", "a.desktop", "b.desktop"])
    );
}

#[test]
fn hidden_copy_sets_hidden_and_marker() {
    let text = "# vendor\n[Desktop Entry]\nType=Application\nName=X\nHidden=false\nExec=x --y\n\
                \n[Desktop Action new]\nHidden=false\nExec=x --new\n";
    let copy = hidden_copy(text);
    assert!(
        copy.starts_with(&format!("{HIDDEN_COPY_HEADER}\n")),
        "{copy}"
    );
    let e = read_entry(&copy);
    assert!(e.hidden && e.uq_autostart, "{copy}");
    for kept in [
        "# vendor\n",
        "Name=X\n",
        "Exec=x --y\n",
        "Exec=x --new\n",
        "Hidden=false\n",
    ] {
        assert!(copy.contains(kept), "{kept:?} missing from {copy}");
    }
    assert_eq!(copy.matches("Hidden=true").count(), 1);
    assert_eq!(
        copy.matches("Hidden=false").count(),
        1,
        "the action's own line stays"
    );
    let e = read_entry(&hidden_copy("[Desktop Entry]\nExec=x\n"));
    assert!(e.hidden && e.uq_autostart);
}

#[test]
fn wanted_copies_rules() {
    let text = entry("a", "");
    let copy = hidden_copy(&text);
    let sys_a = sys("a.desktop", &text);
    // Off, no user file: one copy.
    let (w, d) = wanted_copies(std::slice::from_ref(&sys_a), &off("a.desktop"));
    assert_eq!(w, vec![("a.desktop".to_string(), copy.clone())]);
    assert!(d.is_empty());
    // On, or no choice: nothing.
    let (w, d) = wanted_copies(std::slice::from_ref(&sys_a), &Autostart::default());
    assert!(w.is_empty() && d.is_empty());
    // A user file of their own: never a copy, never a delete.
    let own = Found {
        user: Some((format!("{USER}/a.desktop").into(), entry("mine", ""))),
        ..sys_a.clone()
    };
    let (w, d) = wanted_copies(std::slice::from_ref(&own), &off("a.desktop"));
    assert!(w.is_empty() && d.is_empty());
    let (w, d) = wanted_copies(
        &[user("u.desktop", &entry("u", "Hidden=true\n"))],
        &Autostart::default(),
    );
    assert!(w.is_empty() && d.is_empty());
    // Our copy, and the entry on again: delete it.
    let marked = Found {
        user: Some((format!("{USER}/a.desktop").into(), copy.clone())),
        ..sys_a
    };
    let (w, d) = wanted_copies(std::slice::from_ref(&marked), &Autostart::default());
    assert!(w.is_empty());
    assert_eq!(d, vec![PathBuf::from(format!("{USER}/a.desktop"))]);
    // Still off: the copy stays wanted (the sync rewrites it only when the text differs).
    let (w, d) = wanted_copies(&[marked], &off("a.desktop"));
    assert_eq!(w.len(), 1);
    assert!(d.is_empty());
    // An orphan copy goes.
    let (w, d) = wanted_copies(&[user("gone.desktop", &copy)], &off("gone.desktop"));
    assert!(w.is_empty());
    assert_eq!(d, vec![PathBuf::from(format!("{USER}/gone.desktop"))]);
}

#[test]
fn scan_added_override_is_not_fixed_again() {
    let compat = Compat::from_section(
        &json!({"apps": [{"id": "foo.desktop", "args": ["--bar"], "elsewhere": true}]}),
    )
    .unwrap();
    let state = ok(added(json!({"name": "Foo", "app": "foo.desktop"})));
    let apps = |_: &str| {
        Some((
            PathBuf::from("/home/u/.local/share/applications/foo.desktop"),
            entry("foo --bar", "X-uQ-Override=true\n"),
        ))
    };
    let r = resolve(&[], &state, &compat, &apps, &|_| true).remove(0);
    assert_eq!(
        r.plan,
        Plan::Run {
            argv: s(&["foo", "--bar"]),
            cwd: None,
            delay: 0,
            minimize: false,
            retry: false,
        }
    );
}

#[test]
fn scan_minimize_comes_from_the_choice() {
    let f = sys("a.desktop", &entry("a", ""));
    let r = one(f.clone(), &Autostart::default());
    assert!(!r.minimize);
    let hidden = ok(json!({"files": [{"id": "a.desktop", "minimize": true}]}));
    let r = one(f.clone(), &hidden);
    assert!(r.minimize && r.enabled);
    assert!(matches!(r.plan, Plan::Run { minimize: true, .. }), "{r:?}");
    // An entry that is off keeps the choice for the page but doesn't run.
    let both = ok(json!({"files": [{"id": "a.desktop", "enabled": false, "minimize": true}]}));
    let r = one(f, &both);
    assert_eq!((r.plan.clone(), r.minimize), (Plan::Off, true));
}

#[test]
fn staggered_spaces_starts_in_delay_order() {
    use crate::autostart::scan::staggered;
    // No gap: untouched.
    assert_eq!(staggered(&[0, 5, 0], 0), vec![0, 5, 0]);
    // Three apps at once go 4 s apart, in list order.
    assert_eq!(staggered(&[0, 0, 0], 4), vec![0, 4, 8]);
    // Their own delay is never shortened, and the ones after it follow it.
    assert_eq!(staggered(&[10, 0, 0], 4), vec![10, 0, 4]);
    assert_eq!(staggered(&[0, 2, 0], 4), vec![0, 8, 4]);
    assert_eq!(staggered(&[], 4), Vec::<u32>::new());
}

#[test]
fn stagger_changes_the_plan_not_the_page_delay() {
    let found = vec![
        sys("a.desktop", &entry("a", "")),
        sys("b.desktop", &entry("b", "")),
    ];
    let st = ok(json!({"stagger": 3}));
    let rows = resolve(&found, &st, &Compat::default(), &|_| None, &|_| true);
    let at = |i: usize| match &rows[i].plan {
        Plan::Run { delay, .. } => *delay,
        p => panic!("{p:?}"),
    };
    assert_eq!((at(0), at(1)), (0, 3));
    assert_eq!((rows[0].delay, rows[1].delay), (0, 0));
}

#[test]
fn retry_comes_from_the_section() {
    let st = ok(json!({"retry": true}));
    let r = one(sys("a.desktop", &entry("a", "")), &st);
    assert!(matches!(r.plan, Plan::Run { retry: true, .. }), "{r:?}");
    let r = one(sys("a.desktop", &entry("a", "")), &Autostart::default());
    assert!(matches!(r.plan, Plan::Run { retry: false, .. }));
}

#[test]
fn a_known_app_is_given_its_own_hidden_option() {
    use crate::autostart::scan::{own_hidden_flag, with_hidden_flag};
    assert_eq!(own_hidden_flag("/usr/bin/steam"), Some("-silent"));
    assert_eq!(own_hidden_flag("nm-applet"), None);
    assert_eq!(
        with_hidden_flag(s(&["/usr/bin/steam", "%U"])),
        s(&["/usr/bin/steam", "-silent", "%U"])
    );
    // Already there, or not a known program: left as it is.
    assert_eq!(
        with_hidden_flag(s(&["discord", "--start-minimized"])),
        s(&["discord", "--start-minimized"])
    );
    assert_eq!(with_hidden_flag(s(&["nm-applet"])), s(&["nm-applet"]));
    // Only an app set to start hidden gets it.
    let st = ok(json!({"files": [{"id": "steam.desktop", "minimize": true}]}));
    let r = one(sys("steam.desktop", &entry("steam", "")), &st);
    let Plan::Run { argv, .. } = &r.plan else {
        panic!("{r:?}")
    };
    assert_eq!(argv, &s(&["steam", "-silent"]));
    let r = one(
        sys("steam.desktop", &entry("steam", "")),
        &Autostart::default(),
    );
    let Plan::Run { argv, .. } = &r.plan else {
        panic!("{r:?}")
    };
    assert_eq!(argv, &s(&["steam"]));
}

#[test]
fn stagger_and_retry_are_checked() {
    let bad = |v| Autostart::from_section(&v, &Autostart::default()).unwrap_err();
    assert!(bad(json!({"stagger": 31})).to_string().contains("30"));
    assert!(
        bad(json!({"retry": "yes"}))
            .to_string()
            .contains("on or off")
    );
    let a = ok(json!({"stagger": 5, "retry": true}));
    assert_eq!((a.stagger, a.retry), (5, true));
    assert_eq!(
        a.section(),
        json!({"files": [], "added": [], "stagger": 5, "retry": true})
    );
    // Off is not written.
    assert_eq!(
        Autostart::default().section(),
        json!({"files": [], "added": []})
    );
}
