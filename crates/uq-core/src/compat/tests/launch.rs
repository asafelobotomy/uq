//! The launch command and the launcher-file override.

use super::{one_fix, s};
use crate::compat::AppFix;
use crate::compat::launch::{CompatError, OVERRIDE_HEADER, argv, override_text};
use crate::desktop_entry::{Token, exec_tokens, read_entry};
use serde_json::json;

const WAYLAND_ENV: [&str; 4] = [
    "GDK_BACKEND=wayland,x11",
    "QT_QPA_PLATFORM=wayland;xcb",
    "SDL_VIDEODRIVER=wayland,x11",
    "ELECTRON_OZONE_PLATFORM_HINT=wayland",
];

#[test]
fn launch_argv_wayland_with_env_and_args() {
    let fix = one_fix(json!({"id": "code.desktop", "platform": "wayland",
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"]}));
    let mut want = s(&["env", "--"]);
    want.extend(s(&WAYLAND_ENV));
    want.extend(s(&["FOO=1", "code", "--x"]));
    assert_eq!(
        argv(Some(&fix), "code.desktop", s(&["code"])).unwrap(),
        want
    );
    // The id may come without its suffix.
    assert_eq!(argv(Some(&fix), "code", s(&["code"])).unwrap(), want);
}

#[test]
fn launch_argv_x11_and_scale() {
    let fix = one_fix(json!({"id": "gimp.desktop", "platform": "x11", "scale": 1.5}));
    assert_eq!(
        argv(Some(&fix), "gimp.desktop", s(&["gimp", "-n"])).unwrap(),
        s(&[
            "env",
            "--",
            "GDK_BACKEND=x11",
            "QT_QPA_PLATFORM=xcb",
            "SDL_VIDEODRIVER=x11",
            "ELECTRON_OZONE_PLATFORM_HINT=x11",
            "GDK_SCALE=1",
            "QT_SCALE_FACTOR=1.5",
            "gimp",
            "-n",
        ])
    );
}

#[test]
fn launch_argv_user_env_wins() {
    let fix = one_fix(json!({"id": "code.desktop", "platform": "wayland",
        "env": [{"name": "GDK_BACKEND", "value": "x11"}]}));
    let got = argv(Some(&fix), "code.desktop", s(&["code"])).unwrap();
    let ours = got.iter().position(|a| a == "GDK_BACKEND=wayland,x11");
    let users = got.iter().position(|a| a == "GDK_BACKEND=x11");
    assert!(ours.unwrap() < users.unwrap(), "{got:?}");
    assert_eq!(got.last().unwrap(), "code");
}

#[test]
fn launch_argv_browser_gets_flag() {
    let fix = one_fix(json!({"id": "chromium.desktop", "platform": "x11", "scale": 1.5}));
    let got = argv(
        Some(&fix),
        "chromium.desktop",
        s(&["/usr/bin/chromium", "--a"]),
    )
    .unwrap();
    assert_eq!(
        got[got.len() - 4..],
        s(&[
            "/usr/bin/chromium",
            "--a",
            "--ozone-platform=x11",
            "--force-device-scale-factor=1.5"
        ])
    );
    let wl = one_fix(json!({"id": "brave-browser", "platform": "wayland", "args": ["--y"]}));
    let got = argv(Some(&wl), "brave-browser.desktop", s(&["brave"])).unwrap();
    assert_eq!(
        got[got.len() - 3..],
        s(&["brave", "--ozone-platform=wayland", "--y"])
    );
}

#[test]
fn launch_argv_steam_scale() {
    let fix = one_fix(json!({"id": "steam.desktop", "scale": 2.0}));
    assert_eq!(
        argv(Some(&fix), "steam.desktop", s(&["steam"])).unwrap(),
        s(&[
            "env",
            "--",
            "GDK_SCALE=2",
            "QT_SCALE_FACTOR=2",
            "STEAM_FORCE_DESKTOPUI_SCALING=2",
            "steam"
        ])
    );
    let other = one_fix(json!({"id": "x.desktop", "scale": 2.0}));
    let got = argv(Some(&other), "x.desktop", s(&["x"])).unwrap();
    assert!(!got.iter().any(|a| a.starts_with("STEAM_")), "{got:?}");
}

#[test]
fn launch_argv_no_fix_is_unchanged() {
    let exec = s(&["code", "--new-window"]);
    assert_eq!(argv(None, "code.desktop", exec.clone()).unwrap(), exec);
    // A program whose name holds = needs no wrapper here, so it is fine too.
    assert_eq!(argv(None, "x.desktop", s(&["A=B"])).unwrap(), s(&["A=B"]));
}

#[test]
fn launch_argv_args_only_has_no_wrapper() {
    let fix = one_fix(json!({"id": "code.desktop", "args": ["--x"]}));
    assert_eq!(
        argv(Some(&fix), "code.desktop", s(&["code"])).unwrap(),
        s(&["code", "--x"])
    );
    let browser = one_fix(json!({"id": "chromium.desktop", "args": ["--x"]}));
    assert_eq!(
        argv(Some(&browser), "chromium.desktop", s(&["chromium"])).unwrap(),
        s(&["chromium", "--x"])
    );
    // No env wrapper, so an = in the program's name is harmless.
    assert_eq!(
        argv(Some(&fix), "code.desktop", s(&["A=B"])).unwrap(),
        s(&["A=B", "--x"])
    );
}

#[test]
fn launch_argv_refuses_equals_program() {
    let fix = one_fix(json!({"id": "code.desktop", "platform": "wayland"}));
    let e = argv(Some(&fix), "code.desktop", s(&["FOO=bar", "code"])).unwrap_err();
    assert!(matches!(e, CompatError::Unsafe(_)), "{e:?}");
    let e = argv(Some(&fix), "code.desktop", Vec::new()).unwrap_err();
    assert!(matches!(e, CompatError::BadExec(_)), "{e:?}");
}

const FIXTURE: &str = "\
[Desktop Entry]
Type=Application
Name=Code
Name[de]=Kode
Exec=code %F
DBusActivatable=true

[Desktop Action new-window]
Name=New Window
Exec=code --new-window %F
";

fn code_fix() -> AppFix {
    one_fix(json!({"id": "code.desktop", "platform": "wayland",
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"], "elsewhere": true}))
}

fn exec_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|l| l.strip_prefix("Exec="))
        .collect()
}

#[test]
fn override_text_wraps_every_exec() {
    let text = override_text(
        FIXTURE,
        "/usr/share/applications/code.desktop",
        &code_fix(),
        "code.desktop",
    )
    .unwrap()
    .expect("a file to write");
    let wrapper = "env -- GDK_BACKEND=wayland,x11 \"QT_QPA_PLATFORM=wayland;xcb\" \
                   SDL_VIDEODRIVER=wayland,x11 ELECTRON_OZONE_PLATFORM_HINT=wayland FOO=1";
    assert_eq!(
        exec_lines(&text),
        [
            format!("{wrapper} code --x %F"),
            format!("{wrapper} code --new-window --x %F"),
        ]
    );
    let first = exec_tokens(exec_lines(&text)[0]).unwrap();
    let mut want: Vec<Token> = s(&["env", "--"])
        .into_iter()
        .chain(s(&WAYLAND_ENV))
        .chain(s(&["FOO=1", "code", "--x"]))
        .map(Token::Arg)
        .collect();
    want.push(Token::Field('F'));
    assert_eq!(first.tokens, want);
    assert!(!first.lossy);

    assert!(text.starts_with(&format!("{OVERRIDE_HEADER}\n")), "{text}");
    assert!(text.contains("\nDBusActivatable=false\n"), "{text}");
    assert!(!text.contains("DBusActivatable=true"));
    assert!(text.contains("\nName[de]=Kode\n"));
    assert!(text.contains("\nName=New Window\n"));
    let e = read_entry(&text);
    assert!(e.uq_override);
    assert!(!e.dbus_activatable);
    assert_eq!(
        e.uq_source.as_deref(),
        Some("/usr/share/applications/code.desktop")
    );
    // The marker keys sit in [Desktop Entry], not in the action group.
    let action = text.split("[Desktop Action new-window]").nth(1).unwrap();
    assert!(!action.contains("X-uQ-"), "{action}");
}

#[test]
fn override_text_inserts_args_before_field_code() {
    let fix = one_fix(json!({"id": "chromium.desktop", "platform": "wayland",
        "args": ["--y z"]}));
    let src = "[Desktop Entry]\nExec=chromium --a %U --b\n";
    let text = override_text(
        src,
        "/usr/share/applications/chromium.desktop",
        &fix,
        "chromium",
    )
    .unwrap()
    .unwrap();
    let line = exec_lines(&text)[0];
    assert!(
        line.ends_with("chromium --a --ozone-platform=wayland \"--y z\" %U --b"),
        "{line}"
    );
    // No field code: the args go at the end.
    let fix = one_fix(json!({"id": "x.desktop", "args": ["--x"]}));
    let text = override_text(
        "[Desktop Entry]\nExec=x --a\n",
        "/s/x.desktop",
        &fix,
        "x.desktop",
    )
    .unwrap()
    .unwrap();
    // Args only: no env wrapper (F25).
    assert_eq!(exec_lines(&text), ["x --a --x"]);
    assert!(!text.contains("DBusActivatable"));
}

#[test]
fn override_text_without_changes_is_none() {
    // A fix whose only effect would be nothing on the command line can't exist; an app fix
    // built by hand with nothing set writes no file.
    let empty = AppFix::new("code.desktop");
    assert_eq!(
        override_text(FIXTURE, "/s/code.desktop", &empty, "code.desktop").unwrap(),
        None
    );
}

#[test]
fn override_text_refuses_lossy_exec() {
    for exec in [
        "sh -c 'a b'",
        "app --f=%F",
        "app \"%f\"x",
        "\"unterminated",
        "app %d",
    ] {
        let src = format!("[Desktop Entry]\nExec={exec}\n");
        let e = override_text(&src, "/s/code.desktop", &code_fix(), "code.desktop").unwrap_err();
        assert!(matches!(e, CompatError::BadExec(_)), "{exec}: {e:?}");
    }
    // An action group's bad Exec refuses the whole file too.
    let src = "[Desktop Entry]\nExec=code\n\n[Desktop Action a]\nExec=sh -c 'x'\n";
    assert!(matches!(
        override_text(src, "/s/code.desktop", &code_fix(), "code.desktop"),
        Err(CompatError::BadExec(_))
    ));
    let src = "[Desktop Entry]\nExec=A=B code\n";
    assert!(matches!(
        override_text(src, "/s/code.desktop", &code_fix(), "code.desktop"),
        Err(CompatError::Unsafe(_))
    ));
}

#[test]
fn override_text_never_writes_a_newline() {
    let src = "[Desktop Entry]\nExec=app \"a\\nb\"\n";
    let fix = one_fix(json!({"id": "app.desktop", "platform": "x11"}));
    let text = override_text(src, "/s/app.desktop", &fix, "app.desktop")
        .unwrap()
        .unwrap();
    // The header, the two source lines and the two marker keys.
    assert_eq!(text.lines().count(), 5, "{text}");
    let tokens = exec_tokens(&unescape_exec(exec_lines(&text)[0])).unwrap();
    assert_eq!(tokens.tokens.last(), Some(&Token::Arg("a\nb".into())));
    // A source path is one line too.
    let text = override_text(src, "/s/a\nb.desktop", &fix, "app.desktop")
        .unwrap()
        .unwrap();
    assert_eq!(text.lines().count(), 5, "{text}");
}

/// The key-value unescape `read_entry` applies, for one `Exec` value.
fn unescape_exec(v: &str) -> String {
    read_entry(&format!("[Desktop Entry]\nExec={v}\n"))
        .exec
        .unwrap()
}
