use super::crash::{self, Dirs, Kind};
use super::journal::{self, Query};
use super::log::{PanicRecord, save_panic};
use super::redact::Redactor;
use super::report::{self, Input};
use super::*;

const QS_REPORT: &str = include_str!("testdata/qs-report.txt");

#[test]
fn every_code_is_well_formed_unique_and_explained() {
    let r = Registry::builtin();
    assert!(r.all().len() > 40);
    let mut seen = std::collections::HashSet::new();
    for c in r.all() {
        assert!(is_code(&c.id), "{}", c.id);
        assert!(seen.insert(c.id.clone()), "{} twice", c.id);
        assert!(
            !c.title.is_empty() && !c.means.is_empty() && !c.fix.is_empty(),
            "{}",
            c.id
        );
        assert!(
            !c.title.ends_with('.'),
            "{}: titles have no full stop",
            c.id
        );
        assert_eq!(c.owner, "uQ");
    }
    assert_eq!(r.get("hypr-003").unwrap().severity, Severity::Warn);
    assert_eq!(r.get("HYPR-003").unwrap().area(), "HYPR");
}

#[test]
fn codes_are_found_in_messages() {
    assert_eq!(code_in("[HYPR-003] getoption: timed out"), Some("HYPR-003"));
    assert_eq!(code_in("uq[WIN-001] move column: gone"), Some("WIN-001"));
    assert_eq!(
        code_in("qml: uq-settings[SETTINGS-001] page"),
        Some("SETTINGS-001")
    );
    assert_eq!(code_in("[x] [no] [AB-12] [BINDLE-004]"), Some("BINDLE-004"));
    assert_eq!(code_in("arr[3] and [hypr-003]"), None);
    assert!(!is_code("HYPR-0034"));
    assert!(!is_code("-003"));
}

#[test]
fn apps_add_their_own_codes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bindle.toml"),
        r#"app = "Bindle"
units = ["bindle-update.service"]
[[code]]
id = "BINDLE-001"
severity = "error"
title = "An install failed"
means = "m"
fix = "f"
"#,
    )
    .unwrap();
    // Clashes with uQ's own: the whole file is refused.
    std::fs::write(
        dir.path().join("bad.toml"),
        "[[code]]\nid = \"HYPR-001\"\nseverity = \"warn\"\ntitle = \"t\"\nmeans = \"m\"\nfix = \"f\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("broken.toml"), "not toml [").unwrap();
    let r = Registry::with_apps(dir.path());
    let b = r.get("BINDLE-001").unwrap();
    assert_eq!(b.owner, "Bindle");
    assert_eq!(r.get("HYPR-001").unwrap().owner, "uQ");
    assert!(r.units().contains(&"bindle-update.service".to_string()));
    assert!(r.units().contains(&"uqd.service".to_string()));

    // An app's line in the shell's unit is put down to the app.
    let line = r#"{"MESSAGE":"bindle-gui: [BINDLE-001] no window","PRIORITY":"3","_SYSTEMD_USER_UNIT":"uq-shell.service","__REALTIME_TIMESTAMP":"1"}"#;
    let e = crate::diag::journal::parse_line(line, &r).unwrap();
    assert_eq!(e.source, "bindle-gui");
    assert_eq!(e.code.as_deref(), Some("BINDLE-001"));
    let own = r#"{"MESSAGE":"uqd: [HYPR-001] x","PRIORITY":"3","_SYSTEMD_USER_UNIT":"uqd.service","__REALTIME_TIMESTAMP":"1"}"#;
    assert_eq!(
        crate::diag::journal::parse_line(own, &r).unwrap().source,
        "uqd"
    );
}

#[test]
fn quickshell_reports_are_read() {
    let r = crash::parse_qs_report(QS_REPORT);
    assert_eq!(r.signal, "Segmentation fault (11)");
    assert_eq!(r.run_id, "x21fgjuot");
    assert_eq!(r.config_path, "/home/alice/.local/share/notifd/shell.qml");
    assert!(r.quickshell.starts_with("0.3.2"));
    assert!(r.qt.starts_with("6.12.0"));
    assert_eq!(r.gpus.len(), 2);
    assert!(r.frames.len() > 10);
    let top = crash::telling_frames(&r.frames, 3);
    assert_eq!(top[0], "QFile::QFile(QString const&) (libQt6Core.so.6)");
    assert!(top[2].starts_with("QPixmap::load"));
}

#[test]
fn configs_are_told_apart() {
    assert_eq!(crash::classify("/usr/share/uq/shell"), Kind::Shell);
    assert_eq!(
        crash::classify("/usr/share/uq/shell/shell.qml"),
        Kind::Shell
    );
    assert_eq!(crash::classify("/home/a/Projects/uq/shell/"), Kind::Shell);
    assert_eq!(crash::classify("/usr/share/uq/lock"), Kind::Lock);
    assert_eq!(crash::classify("/usr/share/uq/greeter"), Kind::Greeter);
    let notifd = crash::classify("/home/a/.local/share/notifd/shell.qml");
    assert_eq!(notifd, Kind::App("notifd".into()));
    assert_eq!(notifd.code(), "CRASH-003");
    assert_eq!(notifd.label(), "noti-fi's notification daemon");
}

#[test]
fn crashes_are_found_in_both_folders() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().to_string_lossy().into_owned();
    let dirs = Dirs::from_vars(&|k| (k == "HOME").then(|| home.clone()));
    let qs = dirs.quickshell.join("abc123");
    std::fs::create_dir_all(&qs).unwrap();
    std::fs::write(
        qs.join("report.txt"),
        QS_REPORT.replace("notifd", "uq/shell"),
    )
    .unwrap();
    // Still being written: skipped.
    std::fs::create_dir_all(dirs.quickshell.join("half")).unwrap();
    let rec = PanicRecord {
        program: "uqd".into(),
        version: "0.3.34".into(),
        time: 4_000_000_000,
        pid: 7,
        thread: "main".into(),
        message: "oops".into(),
        location: "crates/uqd/src/x.rs:1".into(),
        backtrace: "bt".into(),
    };
    let folder = save_panic(&dirs.uq, &rec).unwrap();
    let all = crash::scan(&dirs);
    assert_eq!(all.len(), 2);
    // The panic is dated in the future, so it is first.
    assert_eq!(all[0].kind, Kind::Program("uqd".into()));
    assert_eq!(all[0].dir, folder);
    assert_eq!(all[0].code, "CRASH-004");
    assert_eq!(all[1].kind, Kind::Shell);
    assert_eq!(all[1].id, "qs-abc123");
    assert!(!all[1].reported);
    assert_eq!(crash::find(&dirs, "abc123").unwrap().id, "qs-abc123");
    assert_eq!(crash::find(&dirs, "latest").unwrap().id, all[0].id);

    report::save_crash(&all[1], "# r").unwrap();
    crash::dismiss(&all[1]).unwrap();
    let again = crash::find(&dirs, "qs-abc123").unwrap();
    assert!(again.reported && again.dismissed);
}

#[test]
fn private_parts_are_taken_out() {
    let r = Redactor::new(
        Some("/home/solon".into()),
        Some("solon".into()),
        Some("solon64".into()),
    );
    let log = r#"Received event: "activewindow>>brave-origin,Log in to your account - Brave Origin" parsed as "activewindow" "brave-origin,Log in to your account - Brave Origin"
Received event: "windowtitlev2>>5abb3ad0,paypal.com, Inc - Brave" parsed as "windowtitlev2" "5abb3ad0,paypal.com, Inc - Brave"
Received event: "openwindow>>5abb3ad0,3,gamescope,My Game" parsed as "openwindow" "5abb3ad0,3,gamescope,My Game"
Received event: "activewindowv2>>5abb3ad0"
toplevel "Bank - Firefox" got state update
Config Path: /home/solon/.local/share/notifd/shell.qml; user solon on solon64; console ok
from 192.168.1.20 and 127.0.0.1, me@example.org, aa:bb:cc:dd:ee:ff"#;
    let out = r.apply(log);
    for leak in [
        "Log in",
        "paypal",
        "My Game",
        "Bank",
        "/home/solon",
        "solon64",
        "192.168",
        "me@example",
        "aa:bb",
    ] {
        assert!(!out.contains(leak), "{leak} leaked:\n{out}");
    }
    assert!(
        out.contains(r#""activewindow>>brave-origin,<title>""#),
        "{out}"
    );
    assert!(
        out.contains(r#""openwindow" "5abb3ad0,3,gamescope,<title>""#),
        "{out}"
    );
    assert!(out.contains("activewindowv2>>5abb3ad0"));
    assert!(out.contains("~/.local/share/notifd/shell.qml"));
    assert!(out.contains("user <user> on <host>; console ok"), "{out}");
    assert!(out.contains("127.0.0.1"));
}

fn jline(unit: &str, prio: u8, msg: &str, t: i64) -> String {
    serde_json::json!({
        "__REALTIME_TIMESTAMP": t.to_string(),
        "PRIORITY": prio.to_string(),
        "_SYSTEMD_USER_UNIT": unit,
        "MESSAGE": msg,
    })
    .to_string()
}

#[test]
fn journal_lines_get_code_severity_and_source() {
    let reg = Registry::builtin();
    let text = [
        jline(
            "uqd.service",
            4,
            "uqd: [HYPR-003] getoption general:gaps_in: timed out",
            1,
        ),
        // Quickshell writes its own level word, in colour, at priority 6.
        jline(
            "uq-shell.service",
            6,
            "\u{1b}[33m  WARN\u{1b}[0m qml: uq[LAUNCH-001] launching firefox failed: x",
            2,
        ),
        jline(
            "uq-shell.service",
            6,
            "  WARN qt.svg: Invalid path data; path truncated",
            3,
        ),
        jline("uq-shell.service", 6, "  INFO: Configuration Loaded", 4),
        jline(
            "uqd.service",
            3,
            "uqd: [HYPR-003] getoption general:gaps_in: timed out",
            5,
        ),
    ]
    .join("\n");
    let all = journal::parse(&text, reg, &Query::default());
    assert_eq!(all.len(), 4, "{all:#?}");
    assert_eq!(all[0].source, "uqd");
    assert_eq!(all[0].code.as_deref(), Some("HYPR-003"));
    assert_eq!(all[1].source, "shell");
    // The registry says LAUNCH-001 is an error, whatever the line's own level.
    assert_eq!(all[1].severity, Severity::Error);
    assert_eq!(
        all[1].message,
        "qml: uq[LAUNCH-001] launching firefox failed: x"
    );
    assert_eq!(all[2].code, None);
    assert_eq!(all[2].severity, Severity::Warn);

    let coded = journal::parse(
        &text,
        reg,
        &Query {
            coded_only: true,
            ..Query::default()
        },
    );
    assert_eq!(coded.len(), 3);
    let hypr = journal::parse(
        &text,
        reg,
        &Query {
            only: Some("hypr".into()),
            ..Query::default()
        },
    );
    assert_eq!(hypr.len(), 2);

    let grouped = journal::group(all.clone());
    assert_eq!(grouped.len(), 3);
    assert_eq!(grouped[0].code.as_deref(), Some("HYPR-003"));
    assert_eq!(
        (grouped[0].count, grouped[0].first_us, grouped[0].time_us),
        (2, 1, 5)
    );
    let runs = journal::collapse_runs(all);
    assert_eq!(runs.len(), 4);
}

#[test]
fn a_report_has_every_section() {
    let reg = Registry::builtin();
    let c = crash::Crash {
        id: "qs-abc".into(),
        time: 1_795_345_309,
        kind: Kind::Shell,
        code: "CRASH-001",
        reason: "Segmentation fault (11)".into(),
        place: vec!["QFile::QFile (libQt6Core.so.6)".into()],
        qs_dir: None,
        dir: "/tmp/x".into(),
        reported: false,
        dismissed: false,
    };
    let input = Input {
        crash: Some(c),
        uq_version: "0.3.34".into(),
        packages: vec!["quickshell 0.3.2-1".into()],
        log_tail: vec!["  WARN qml: uq[WIN-001] move column: gone".into()],
        qs_report: Some("===== Stacktrace =====\n```\n#0".into()),
        problems: journal::parse(
            &jline(
                "uqd.service",
                4,
                "uqd: [HYPR-003] getoption: timed out",
                1_795_345_000_000_000,
            ),
            reg,
            &Query::default(),
        ),
        changes: vec!["2026-11-22 10:00:00 settings appearance: Theme: uq-dark".into()],
        ..Input::default()
    };
    let md = report::render(&input, reg);
    for want in [
        "# uQ's shell crashed — uQ crash report",
        "**Code:** CRASH-001 (uQ's shell crashed)",
        "## Versions",
        "- quickshell 0.3.2-1",
        "## Problems logged in the 15 minutes before",
        "`HYPR-003` warn",
        "## Last setting changes",
        "## The last 1 lines uQ's shell logged",
        "````text\n===== Stacktrace",
    ] {
        assert!(md.contains(want), "missing {want:?} in\n{md}");
    }
}

#[test]
fn cpp_backtraces_say_where() {
    let t = "/usr/bin/reelo(+0xe731f) [0x560de46b231f]\n\
/usr/lib/libc.so.6(+0x3e6f0) [0x7fc818fb36f0]\n\
/usr/bin/reelo(+0x1234) [0x1]\n\
/usr/lib/libc.so.6(ppoll+0x77) [0x7fc819086de7]\n\
/usr/lib/libQt6Core.so.6(_ZN10QEventLoop4execE+0x1a4) [0x7f]\n\
/usr/lib/libQt6Core.so.6(_ZN10QEventLoop4execE+0x1a4) [0x7f]\n";
    assert_eq!(
        crate::diag::crash::glibc_frames(t, 3),
        [
            "reelo+0x1234",
            "ppoll (libc.so.6)",
            "_ZN10QEventLoop4execE (libQt6Core.so.6)"
        ]
    );
    assert!(crate::diag::crash::glibc_frames("", 3).is_empty());
}

#[test]
fn app_lists_are_checked() {
    let code = |id: &str| {
        format!(
            "[[code]]\nid = \"{id}\"\nseverity = \"warn\"\ntitle = \"t\"\nmeans = \"m\"\nfix = \"f\"\n"
        )
    };
    let ok = format!(
        "app = \"Reelo\"\n{}{}",
        code("REELO-001"),
        code("REELO-002")
    );
    assert_eq!(Registry::check_app(&ok), Ok(2));
    let err = |t: &str| Registry::check_app(t).unwrap_err();
    assert!(err(&code("REELO-001")).contains("no `app`"));
    assert!(err(&format!("app = \"x\"\n{}", code("HYPR-001"))).contains("uQ's own areas"));
    assert!(
        err(&format!("app = \"x\"\n{}{}", code("A-001"), code("B-001"))).contains("one prefix")
    );
    assert!(err(&format!("app = \"x\"\n{}{}", code("A-001"), code("A-001"))).contains("twice"));
    assert!(err(&format!("app = \"x\"\nunits = [\"x\"]\n{}", code("A-001"))).contains("units"));
    assert!(err("app = \"x\"\n").contains("no [[code]]"));
    let no_fix = format!(
        "app = \"x\"\n{}",
        code("A-001").replace("fix = \"f\"", "fix = \" \"")
    );
    assert!(err(&no_fix).contains("no fix"));
}
