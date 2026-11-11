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
            delay: 0
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
            delay: 0
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
            delay: 0
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
            delay: 0
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
            delay: 0
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
            delay: 0
        }
    );
}
