//! Launching in a systemd scope, quoting and dispatch.

use super::*;

#[test]
fn shell_quote_single_quotes() {
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
    assert_eq!(shell_quote("a b"), "'a b'");
}

#[test]
fn launch_wraps_in_systemd_run_scope() {
    assert_eq!(
        scope_command("org.mozilla.firefox", &v(&["firefox"]), 0xabcd),
        "systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-org_mozilla_firefox-0000abcd -- 'firefox'"
    );
    let long = "x".repeat(100);
    let cmd = scope_command(&format!("{long}/../a b"), &v(&["a"]), 1);
    assert!(
        cmd.contains(&format!("--unit=app-uq-{}-00000001 ", "x".repeat(64))),
        "{cmd}"
    );
}

#[test]
fn exec_lua_escapes() {
    assert_eq!(
        exec_lua("echo \"a\"\nb"),
        r#"return hl.dispatch(hl.dsp.exec_cmd("echo \"a\"\nb"))"#
    );
}

#[test]
fn hostile_desktop_entry_stays_quoted() {
    let argv = parse_exec(r#"evil "a; rm -rf ~""#).unwrap();
    assert_eq!(argv, v(&["evil", "a; rm -rf ~"]));
    let cmd = scope_command("evil", &argv, 0);
    assert!(cmd.ends_with("-- 'evil' 'a; rm -rf ~'"), "{cmd}");
}

#[test]
fn desktop_id_plans_scope_and_records() {
    let (_t, d) = dirs_with(&[(
        "org.a.B.desktop",
        "[Desktop Entry]\nType=Application\nExec=b --x %U\nName=B\n[Desktop Action n]\nExec=zzz\n",
    )]);
    let p = plan(
        Some(&json!({"desktop_id":"org.a.B"})),
        &d,
        1,
        &RunState::default(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        p.command,
        "systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-org_a_B-00000001 -- 'b' '--x'"
    );
    assert_eq!(p.record.as_deref(), Some("org.a.B"));
}

#[test]
fn argv_and_run_id_and_junk() {
    let d = Dirs::new(None, vec![]);
    let p = plan(
        Some(&json!({"argv":["/usr/bin/foot","-e","a b"]})),
        &d,
        2,
        &RunState::default(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        p.command,
        "systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-foot-00000002 -- '/usr/bin/foot' '-e' 'a b'"
    );
    assert_eq!(p.record, None);
    let mut runs = RunState::default();
    run_item("> echo hi", 7, &mut runs).unwrap();
    let p = plan(Some(&json!({"run_id":"run-7"})), &d, 3, &runs, None, None).unwrap();
    assert_eq!(
        p.command,
        "systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-run-00000003 -- 'sh' '-c' 'echo hi'"
    );
    assert_eq!(p.record, None);
    assert_eq!(
        plan(Some(&json!({"run_id":"run-8"})), &d, 0, &runs, None, None).unwrap_err(),
        "unknown run id"
    );
    assert!(plan(Some(&json!({"run_id":5})), &d, 0, &runs, None, None).is_err());
    for bad in [
        json!({"argv":[]}),
        json!({"argv":[1]}),
        json!({}),
        json!("x"),
    ] {
        assert!(
            plan(Some(&bad), &d, 0, &RunState::default(), None, None).is_err(),
            "{bad}"
        );
    }
    assert!(plan(None, &d, 0, &RunState::default(), None, None).is_err());
}

#[tokio::test]
async fn dispatch_sends_eval_and_maps_errors() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("hypr/SIG");
    std::fs::create_dir_all(&sock).unwrap();
    let l = tokio::net::UnixListener::bind(sock.join(".socket.sock")).unwrap();
    let server = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in ["ok", "nope"] {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = s.read(&mut buf).await.unwrap();
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
            s.write_all(reply.as_bytes()).await.unwrap();
        }
        seen
    });
    let h = HyprSocket::new(Some(dir.path().into()), Some("SIG".into()));
    let p = Plan {
        command: "c".into(),
        record: None,
        place: None,
        tabs: None,
    };
    dispatch(&h, &p).await.unwrap();
    assert_eq!(dispatch(&h, &p).await.unwrap_err(), "nope");
    assert_eq!(
        server.await.unwrap()[0],
        "eval return hl.dispatch(hl.dsp.exec_cmd(\"c\"))"
    );
    let off = HyprSocket::new(None, None);
    assert_eq!(
        dispatch(&off, &p).await.unwrap_err(),
        "Hyprland is not running"
    );
}
