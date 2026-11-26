//! Terminal apps, the docked terminal and its tabs.

use super::*;

#[test]
fn terminal_apps_get_the_fallback_terminal_by_default_and_bad_entries_refuse() {
    let (_t, d) = dirs_with(&[
        HTOP,
        ("h.desktop", "[Desktop Entry]\nExec=h\nHidden=true\n"),
        (
            "m.desktop",
            "[Desktop Entry]\nExec=m\nTryExec=/no/such/bin\n",
        ),
        ("d.desktop", "[Desktop Entry]\nExec=d %d\n"),
    ]);
    let p = htop_plan(&d, None);
    assert!(
        p.command
            .ends_with(&format!("-- '{}' '-e' 'htop'", fallback_terminal())),
        "{}",
        p.command
    );
    for id in ["h", "m", "d", "absent"] {
        assert!(
            plan(
                Some(&json!({"desktop_id":id})),
                &d,
                0,
                &RunState::default(),
                None,
                None
            )
            .is_err(),
            "{id}"
        );
    }
    // Ids that could escape the applications folder never reach the filesystem.
    for id in ["../x", "a/b", "", "a b"] {
        let e = plan(
            Some(&json!({"desktop_id":id})),
            &d,
            0,
            &RunState::default(),
            None,
            None,
        )
        .unwrap_err();
        assert!(e.starts_with("invalid desktop id"), "{e}");
    }
}

#[test]
fn terminal_uses_chosen_entry() {
    let (_t, d) = dirs_with(&[HTOP, ("foot.desktop", "[Desktop Entry]\nExec=foot\n")]);
    for id in ["foot", "foot.desktop"] {
        let p = htop_plan(&d, Some(id));
        assert!(
            p.command.ends_with("-- 'foot' '-e' 'htop'"),
            "{}",
            p.command
        );
    }
}

#[test]
fn terminal_arg_from_x_terminal_arg_exec() {
    let (_t, d) = dirs_with(&[
        HTOP,
        (
            "gnome-terminal.desktop",
            "[Desktop Entry]\nExec=gnome-terminal %U\nX-TerminalArgExec=--\n",
        ),
    ]);
    let p = htop_plan(&d, Some("gnome-terminal"));
    assert!(
        p.command.ends_with("-- 'gnome-terminal' '--' 'htop'"),
        "{}",
        p.command
    );
}

#[test]
fn terminal_falls_back_to_kitty() {
    let (_t, d) = dirs_with(&[
        HTOP,
        ("bad.desktop", "[Desktop Entry]\nExec=bad %d\n"),
        ("hid.desktop", "[Desktop Entry]\nExec=hid\nHidden=true\n"),
        (
            "gone.desktop",
            "[Desktop Entry]\nExec=gone\nTryExec=/no/such/bin\n",
        ),
        ("tty.desktop", "[Desktop Entry]\nExec=tty\nTerminal=true\n"),
        ("noexec.desktop", "[Desktop Entry]\nName=x\n"),
    ]);
    for t in [
        "missing", "bad", "hid", "gone", "tty", "noexec", "../x", "a/b", "",
    ] {
        let p = htop_plan(&d, Some(t));
        assert!(
            p.command
                .ends_with(&format!("-- '{}' '-e' 'htop'", fallback_terminal())),
            "{t}: {}",
            p.command
        );
    }
}

#[test]
fn hostile_terminal_entry_stays_argv() {
    let (_t, d) = dirs_with(&[
        HTOP,
        (
            "evil.desktop",
            "[Desktop Entry]\nExec=evil \"a; rm -rf ~\" %f\nX-TerminalArgExec=$(id)\n",
        ),
    ]);
    let p = htop_plan(&d, Some("evil"));
    assert!(
        p.command
            .ends_with("-- 'evil' 'a; rm -rf ~' '$(id)' 'htop'"),
        "{}",
        p.command
    );
}

#[test]
fn run_in_terminal_keeps_the_window_and_passes_the_command_as_an_argument() {
    let d = Dirs::new(None, vec![]);
    let mut runs = RunState::default();
    run_item("> ls", 9, &mut runs).unwrap();
    let arg = json!({"run_id": "run-9", "terminal": true});
    let p = plan(Some(&arg), &d, 4, &runs, None, None).unwrap();
    assert!(
        p.command.contains(&format!(
            "-- '{}' '-e' 'sh' '-c' 'eval \"$1\"",
            fallback_terminal()
        )),
        "{}",
        p.command
    );
    assert!(p.command.ends_with("'sh' 'ls'"), "{}", p.command);
    let bad = json!({"run_id": "run-9", "terminal": "yes"});
    assert!(plan(Some(&bad), &d, 4, &runs, None, None).is_err());
}

#[test]
fn placed_terminal_run_floats_without_taking_focus() {
    let runs = {
        let mut r = super::super::super::run::RunState::default();
        super::super::super::run::run_item("> ls", 3, &mut r).unwrap();
        r
    };
    let d = Dirs::new(None, vec![]);
    let arg = serde_json::json!({
        "run_id": "run-3", "terminal": true,
        "place": { "x": 320, "y": 150, "w": 640, "h": 240, "monitor": "HEADLESS-2" }
    });
    let p = plan(Some(&arg), &d, 1, &runs, None, None).unwrap();
    assert_eq!(
        exec_lua_placed("c", p.place.as_ref()),
        "return hl.dispatch(hl.dsp.exec_cmd(\"c\", { float = true, monitor = \"HEADLESS-2\", size = { 640, 240 }, move = { 320, 150 }, no_initial_focus = true }))"
    );
    assert_eq!(exec_lua_placed("c", None), exec_lua("c"));
    let evil = serde_json::json!({ "run_id": "run-3", "terminal": true,
        "place": { "x": 0, "y": 0, "w": 5, "h": 5, "monitor": "a\") hl.exec_cmd(\"x" } });
    assert!(plan(Some(&evil), &d, 1, &runs, None, None).is_err());
    // A place needs a terminal, and must be whole numbers with a size.
    let bg = serde_json::json!({ "run_id": "run-3", "place": { "x": 0, "y": 0, "w": 1, "h": 1 } });
    assert!(plan(Some(&bg), &d, 1, &runs, None, None).is_err());
    let bad = serde_json::json!({ "run_id": "run-3", "terminal": true, "place": { "x": 0, "y": 0, "w": 0, "h": 5 } });
    assert!(plan(Some(&bad), &d, 1, &runs, None, None).is_err());
    let none = serde_json::json!({ "run_id": "run-3", "terminal": true });
    assert_eq!(
        plan(Some(&none), &d, 1, &runs, None, None).unwrap().place,
        None
    );
}

#[test]
fn docked_kitty_listens_for_tabs_and_a_tab_goes_through_its_socket() {
    let rt = tempfile::tempdir().unwrap();
    let d = Dirs::new(None, vec![]).with_runtime(Some(rt.path().into()));
    let mut runs = RunState::default();
    run_item("> make", 5, &mut runs).unwrap();
    // Docked: kitty (the fallback) listens on a socket of its own, and the handle comes back.
    let dock = json!({"run_id": "run-5", "terminal": true, "dock": true});
    let p = plan(Some(&dock), &d, 0xab, &runs, None, None).unwrap();
    assert_eq!(p.tabs.as_deref(), Some("kitty-000000ab"));
    let sock = rt.path().join("uq-lookseedo-kitty-000000ab.sock");
    assert!(
        p.command.contains(&format!(
            "-- 'kitty' '-o' 'allow_remote_control=socket-only' '--listen-on' 'unix:{}' '-e' 'sh' '-c'",
            sock.display()
        )),
        "{}",
        p.command
    );
    // Not docked, or no runtime folder: no socket and no handle.
    let plain = json!({"run_id": "run-5", "terminal": true});
    let p = plan(Some(&plain), &d, 0xab, &runs, None, None).unwrap();
    assert!(
        p.tabs.is_none() && !p.command.contains("listen-on"),
        "{}",
        p.command
    );
    let p = plan(Some(&dock), &Dirs::new(None, vec![]), 1, &runs, None, None).unwrap();
    assert!(p.tabs.is_none(), "{}", p.command);

    // A tab: refused while the socket isn't there (the terminal closed), then kitty's client.
    let tab = json!({"run_id": "run-5", "terminal": true, "tab": "kitty-000000ab"});
    assert_eq!(
        plan(Some(&tab), &d, 1, &runs, None, None).unwrap_err(),
        "the docked terminal is closed"
    );
    std::fs::write(&sock, "").unwrap();
    let p = plan(Some(&tab), &d, 1, &runs, None, None).unwrap();
    assert!(
        p.command.starts_with(&format!(
            "'kitty' '@' '--to' 'unix:{}' 'launch' '--type=tab' 'sh' '-c' 'eval \"$1\"",
            sock.display()
        )),
        "{}",
        p.command
    );
    assert!(p.command.ends_with("'sh' 'make'"), "{}", p.command);
    assert!(p.place.is_none() && p.tabs.is_none());

    // Handles are only ever kitty- and 8 hex digits; dock and tab need a terminal.
    for bad in [
        json!({"run_id": "run-5", "terminal": true, "tab": "../x"}),
        json!({"run_id": "run-5", "terminal": true, "tab": "kitty-000000AB"}),
        json!({"run_id": "run-5", "terminal": true, "tab": 3}),
        json!({"run_id": "run-5", "terminal": true, "dock": "yes"}),
        json!({"run_id": "run-5", "dock": true}),
        json!({"run_id": "run-5", "tab": "kitty-000000ab"}),
    ] {
        assert!(plan(Some(&bad), &d, 1, &runs, None, None).is_err(), "{bad}");
    }
}

#[test]
fn a_docked_terminal_that_is_not_kitty_has_no_tabs() {
    let rt = tempfile::tempdir().unwrap();
    let apps = tempfile::tempdir().unwrap();
    std::fs::write(
        apps.path().join("foot.desktop"),
        "[Desktop Entry]\nType=Application\nName=Foot\nExec=foot\n",
    )
    .unwrap();
    let d = Dirs::new(Some(apps.path().into()), vec![]).with_runtime(Some(rt.path().into()));
    let mut runs = RunState::default();
    run_item("> ls", 2, &mut runs).unwrap();
    let dock = json!({"run_id": "run-2", "terminal": true, "dock": true});
    let p = plan(Some(&dock), &d, 1, &runs, Some("foot.desktop"), None).unwrap();
    assert!(p.tabs.is_none(), "{}", p.command);
    assert!(p.command.contains("-- 'foot' '-e' 'sh'"), "{}", p.command);
}

#[test]
fn docked_kyko_listens_for_tabs_and_a_tab_goes_through_kyko() {
    let rt = tempfile::tempdir().unwrap();
    let apps = tempfile::tempdir().unwrap();
    std::fs::write(
        apps.path().join("org.kyko.Kyko.desktop"),
        "[Desktop Entry]\nType=Application\nName=kyko\nExec=kyko\n",
    )
    .unwrap();
    let d = Dirs::new(Some(apps.path().into()), vec![]).with_runtime(Some(rt.path().into()));
    let mut runs = RunState::default();
    run_item("> make", 6, &mut runs).unwrap();
    let term = Some("org.kyko.Kyko.desktop");
    // Docked: kyko listens with its own --listen-on and no kitty -o option.
    let dock = json!({"run_id": "run-6", "terminal": true, "dock": true});
    let p = plan(Some(&dock), &d, 0xcd, &runs, term, None).unwrap();
    assert_eq!(p.tabs.as_deref(), Some("kyko-000000cd"));
    let sock = rt.path().join("uq-lookseedo-kyko-000000cd.sock");
    assert!(
        p.command.contains(&format!(
            "-- 'kyko' '--listen-on' 'unix:{}' '-e' 'sh' '-c'",
            sock.display()
        )),
        "{}",
        p.command
    );
    assert!(!p.command.contains("allow_remote_control"), "{}", p.command);

    // A tab goes through `kyko @`, with `--type tab` (kyko has no `--type=tab` form).
    let tab = json!({"run_id": "run-6", "terminal": true, "tab": "kyko-000000cd"});
    assert_eq!(
        plan(Some(&tab), &d, 1, &runs, term, None).unwrap_err(),
        "the docked terminal is closed"
    );
    std::fs::write(&sock, "").unwrap();
    let p = plan(Some(&tab), &d, 1, &runs, term, None).unwrap();
    assert!(
        p.command.starts_with(&format!(
            "'kyko' '@' '--to' 'unix:{}' 'launch' '--type' 'tab' 'sh' '-c' 'eval \"$1\"",
            sock.display()
        )),
        "{}",
        p.command
    );
    assert!(p.command.ends_with("'sh' 'make'"), "{}", p.command);

    // The handle, not today's terminal setting, picks the client: a kyko dock opened earlier
    // still gets `kyko @` after the setting moved to another terminal.
    let p = plan(Some(&tab), &d, 1, &runs, None, None).unwrap();
    assert!(p.command.starts_with("'kyko' '@'"), "{}", p.command);

    for bad in [
        "kyko-000000CD",
        "kyko000000cd",
        "kykox-000000cd",
        "kyko-00000cd",
    ] {
        let j = json!({"run_id": "run-6", "terminal": true, "tab": bad});
        assert!(plan(Some(&j), &d, 1, &runs, term, None).is_err(), "{bad}");
    }
}
