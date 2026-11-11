use super::*;
use crate::integration::manifest::ArgSpec;
use std::time::Instant;

/// The shell and `env` live in /bin or /usr/bin depending on the system; tests allow both.
fn runner() -> Runner {
    Runner::system().with_prefixes(vec!["/bin".into(), "/usr/bin".into()])
}

fn sh() -> String {
    if std::path::Path::new("/usr/bin/sh").exists() {
        "/usr/bin/sh".into()
    } else {
        "/bin/sh".into()
    }
}

fn spec(script: &str, output: Output, timeout_ms: u64) -> CommandSpec {
    CommandSpec {
        id: "t".into(),
        argv: vec![sh(), "-c".into(), script.into(), "sh".into()],
        output,
        timeout_ms,
        args: Vec::new(),
        confirm: None,
    }
}

fn with_args(mut s: CommandSpec, args: &[(&str, &str)]) -> CommandSpec {
    s.args = args
        .iter()
        .map(|(n, p)| ArgSpec::new(n, p).unwrap())
        .collect();
    s
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn argv0_must_be_under_an_allowed_prefix() {
    let r = Runner::system();
    let mut s = spec("true", Output::None, 1000);
    for bad in [
        "/bin/sh",
        "sh",
        "/usr/bin/../../bin/sh",
        "/usr/bin/./sh",
        "/usr/bin",
        "/usr/bin/",
        "//usr/bin/sh",
        "/usr/binx/sh",
        "/usr/lib64/x",
        "/usr/local/bin/x",
    ] {
        s.argv[0] = bad.into();
        assert_eq!(
            r.argv(&s, &[]),
            Err(CommandError::NotAllowed(bad.into())),
            "{bad}"
        );
    }
    s.argv[0] = "/usr/lib/uq/x".into();
    assert!(r.argv(&s, &[]).is_ok());
    s.argv[0] = "/usr/bin/x".into();
    assert_eq!(r.argv(&s, &[]).unwrap()[0], "/usr/bin/x");
}

#[test]
fn args_are_checked_appended_and_never_options() {
    let s = with_args(
        spec("true", Output::None, 1000),
        &[("app", "^[A-Za-z0-9._:@+-]{1,128}$"), ("n", "^[0-9]+$")],
    );
    let r = runner();
    let argv = r.argv(&s, &strings(&["org.x.App", "12"])).unwrap();
    assert_eq!(&argv[argv.len() - 2..], ["org.x.App", "12"]);
    assert_eq!(argv.len(), s.argv.len() + 2);

    for (why, args) in [
        ("too few", strings(&["a"])),
        ("too many", strings(&["a", "1", "2"])),
        ("none", vec![]),
        ("leading dash", strings(&["-rf", "1"])),
        ("leading dash, pattern allows it", strings(&["--x", "1"])),
        ("not matching", strings(&["a b", "1"])),
        ("trailing newline", strings(&["a", "1\n"])),
        ("leading newline", strings(&["a", "\n1"])),
        ("empty", strings(&["", "1"])),
        ("nul", strings(&["a\0", "1"])),
    ] {
        assert!(
            matches!(r.argv(&s, &args), Err(CommandError::BadArgs(_))),
            "{why}: {args:?}"
        );
    }
}

#[test]
fn patterns_match_the_whole_value() {
    // An alternation that a naive search would satisfy with a partial match.
    let a = ArgSpec::new("x", "^a|b$").unwrap();
    assert!(a.regex.is_match("a"));
    assert!(a.regex.is_match("b"));
    assert!(!a.regex.is_match("ab"));
    assert!(!a.regex.is_match("axb"));
    assert!(!a.regex.is_match("a\n"));
    for bad in ["[a-z]+", "^[a-z]+", "[a-z]+$", "^a\\$", "^(unclosed$", ""] {
        assert!(ArgSpec::new("x", bad).is_err(), "{bad:?}");
    }
    assert!(ArgSpec::new("Bad Name", "^a$").is_err());
    assert!(ArgSpec::new("x", &format!("^{}$", "a".repeat(600))).is_err());
}

#[tokio::test]
async fn json_text_and_none_output() {
    let r = runner();
    let json = r
        .run(&spec(r#"echo '{"a": [1, "x"]}'"#, Output::Json, 5000), &[])
        .await
        .unwrap();
    assert_eq!(json.code, 0);
    assert_eq!(json.output, json!({"a": [1, "x"]}));
    assert_eq!(
        json.to_json(),
        json!({"code": 0, "output": {"a": [1, "x"]}})
    );

    let text = r
        .run(&spec("printf 'hi\\nthere'", Output::Text, 5000), &[])
        .await
        .unwrap();
    assert_eq!(text.output, json!("hi\nthere"));

    let none = r
        .run(&spec("echo ignored", Output::None, 5000), &[])
        .await
        .unwrap();
    assert_eq!(none.output, Value::Null);

    let bad = r.run(&spec("echo '{nope'", Output::Json, 5000), &[]).await;
    assert!(matches!(bad, Err(CommandError::BadJson(_))), "{bad:?}");
}

#[tokio::test]
async fn args_reach_the_program_as_separate_entries() {
    let s = with_args(
        spec(r#"printf '%s|' "$@""#, Output::Text, 5000),
        &[("a", "^[a-z ;$()]+$"), ("b", "^.+$")],
    );
    let res = runner()
        .run(&s, &strings(&["two words; $(id)", "*"]))
        .await
        .unwrap();
    assert_eq!(res.output, json!("two words; $(id)|*|"));
}

#[tokio::test]
async fn nonzero_exit_reports_code_and_stderr() {
    let res = runner()
        .run(
            &spec(
                "echo partial; echo 'it broke' >&2; exit 3",
                Output::Json,
                5000,
            ),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(res.code, 3);
    assert_eq!(res.stderr_head, "it broke\n");
    let j = res.to_json();
    assert_eq!(j["code"], 3);
    assert_eq!(j["error"], "exited with code 3: it broke");
    assert_eq!(j["output"], "partial\n", "unparseable JSON kept as text");
}

#[tokio::test]
async fn environment_is_scrubbed() {
    let env_bin = ["/usr/bin/env", "/bin/env"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .expect("env(1)");
    let from: Vec<(OsString, OsString)> = [
        ("HOME", "/home/u"),
        ("USER", "u"),
        ("LANG", "C.UTF-8"),
        ("LC_TIME", "en_GB.UTF-8"),
        ("XDG_RUNTIME_DIR", "/run/user/1"),
        ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/x"),
        ("WAYLAND_DISPLAY", "wayland-1"),
        ("PATH", "/evil"),
        ("LD_PRELOAD", "/evil.so"),
        ("SECRET_TOKEN", "x"),
        ("DISPLAY", ":0"),
        ("UQ_INTEGRATION", "0"),
    ]
    .iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect();
    let r = runner().with_env_from(from);
    let s = CommandSpec {
        argv: vec![env_bin.into()],
        ..spec("", Output::Text, 5000)
    };
    let out = r.run(&s, &[]).await.unwrap().output;
    let mut lines: Vec<&str> = out.as_str().unwrap().lines().collect();
    lines.sort();
    assert_eq!(
        lines,
        [
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/x",
            "HOME=/home/u",
            "LANG=C.UTF-8",
            "LC_TIME=en_GB.UTF-8",
            "PATH=/usr/bin",
            "UQ_INTEGRATION=1",
            "USER=u",
            "WAYLAND_DISPLAY=wayland-1",
            "XDG_RUNTIME_DIR=/run/user/1",
        ]
    );
}

#[tokio::test]
async fn stdin_is_closed() {
    let res = runner()
        .run(&spec("cat; echo done", Output::Text, 5000), &[])
        .await
        .unwrap();
    assert_eq!(res.output, json!("done\n"));
}

/// Whether `pid` still runs (a zombie waiting for its reaper counts as gone).
fn alive(pid: &str) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

async fn wait_gone(pid: &str) -> bool {
    for _ in 0..50 {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn timeout_kills_the_whole_group() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let script = format!("sleep 30 & echo $! > '{}'; sleep 30", pidfile.display());
    let start = Instant::now();
    let res = runner().run(&spec(&script, Output::Text, 300), &[]).await;
    assert_eq!(res, Err(CommandError::Timeout(300)));
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "SIGTERM was enough: {:?}",
        start.elapsed()
    );
    let pid = std::fs::read_to_string(&pidfile).unwrap();
    assert!(
        wait_gone(pid.trim()).await,
        "the background child {pid} survived"
    );
}

#[tokio::test]
async fn sigkill_follows_an_ignored_sigterm() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    // An ignored signal stays ignored across exec, so both shells and both sleeps ignore TERM.
    let script = format!(
        "trap '' TERM; sleep 30 & echo $! > '{}'; sleep 30",
        pidfile.display()
    );
    let start = Instant::now();
    let res = runner().run(&spec(&script, Output::Text, 200), &[]).await;
    assert_eq!(res, Err(CommandError::Timeout(200)));
    let took = start.elapsed();
    assert!(
        took >= KILL_GRACE && took < KILL_GRACE + Duration::from_secs(3),
        "{took:?}"
    );
    let pid = std::fs::read_to_string(&pidfile).unwrap();
    assert!(wait_gone(pid.trim()).await, "{pid} survived SIGKILL");
}

#[tokio::test]
async fn output_caps_stop_the_command() {
    let r = runner();
    let res = r
        .run(
            &spec("head -c 5000000 /dev/zero; sleep 30", Output::None, 20_000),
            &[],
        )
        .await;
    assert_eq!(res, Err(CommandError::StdoutTooLarge));
    let res = r
        .run(
            &spec(
                "head -c 70000 /dev/zero >&2; sleep 30",
                Output::None,
                20_000,
            ),
            &[],
        )
        .await;
    assert_eq!(res, Err(CommandError::StderrTooLarge));
    // Exactly at the cap is fine.
    let res = r
        .run(
            &spec(
                &format!("head -c {STDOUT_CAP} /dev/zero"),
                Output::None,
                20_000,
            ),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(res.code, 0);
}

#[tokio::test]
async fn killed_by_a_signal_is_an_error() {
    let res = runner()
        .run(&spec("kill -9 $$", Output::None, 5000), &[])
        .await;
    assert_eq!(res, Err(CommandError::Signal(9)));
}

#[tokio::test]
async fn missing_program_is_a_spawn_error() {
    let s = CommandSpec {
        argv: vec!["/usr/bin/uq-no-such-program".into()],
        ..spec("", Output::None, 1000)
    };
    let res = Runner::system().run(&s, &[]).await;
    assert!(matches!(res, Err(CommandError::Spawn { .. })), "{res:?}");
}

#[test]
fn busy_guard_is_per_owner_and_command() {
    let busy = BusySet::new();
    let a = busy.try_acquire("page", "clean").unwrap();
    assert_eq!(
        busy.try_acquire("page", "clean").unwrap_err(),
        CommandError::Busy
    );
    let _b = busy.try_acquire("page", "list").unwrap();
    let _c = busy.try_acquire("other", "clean").unwrap();
    drop(a);
    assert!(busy.try_acquire("page", "clean").is_ok());
}

#[test]
fn confirm_text_fills_the_first_argument_as_plain_text() {
    let s = CommandSpec {
        confirm: Some("Remove %s and its leftovers?".into()),
        ..with_args(spec("", Output::None, 1000), &[("app", "^.+$")])
    };
    assert_eq!(
        s.confirm_text(&strings(&["Maps\u{202E}gpj"])).unwrap(),
        "Remove Maps gpj and its leftovers?"
    );
    assert_eq!(spec("", Output::None, 1).confirm_text(&[]), None);
}

#[test]
fn find_names_the_command() {
    let cmds = vec![spec("", Output::None, 1)];
    assert!(find(&cmds, "t").is_ok());
    assert_eq!(
        find(&cmds, "x").unwrap_err(),
        CommandError::Unknown("x".into())
    );
}
