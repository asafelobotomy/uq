use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn arg(s: &str) -> Token {
    Token::Arg(s.into())
}

#[test]
fn exec_tokens_keep_field_codes() {
    let t = exec_tokens("code %F").unwrap();
    assert_eq!(t.tokens, vec![arg("code"), Token::Field('F')]);
    assert!(!t.lossy);
    let t = exec_tokens(r#"app "%f""#).unwrap();
    assert_eq!(t.tokens, vec![arg("app"), Token::Field('f')]);
    assert!(!t.lossy);
}

#[test]
fn exec_tokens_mark_lossy() {
    for exec in ["app --f=%F", "sh -c 'a b'", r#"app "x%fy""#] {
        assert!(exec_tokens(exec).unwrap().lossy, "{exec}");
    }
    assert!(exec_tokens("app %d").is_err());
    assert!(exec_tokens("app %z").is_err());
}

#[test]
fn exec_argv_expands_icon_name_and_path() {
    let ctx = FieldCtx {
        icon: Some("x"),
        name: Some("My App"),
        path: Some("/p"),
    };
    assert_eq!(
        exec_argv("app %i %c %k", &ctx).unwrap(),
        v(&["app", "--icon", "x", "My App", "/p"])
    );
    assert_eq!(
        exec_argv("app %i %c", &FieldCtx::default()).unwrap(),
        v(&["app"])
    );
}

#[test]
fn exec_argv_matches_parse_exec_cases() {
    assert_eq!(parse_exec("firefox %u").unwrap(), v(&["firefox"]));
    assert_eq!(
        parse_exec("app --f=%F x %%").unwrap(),
        v(&["app", "--f=", "x", "%"])
    );
    assert_eq!(
        parse_exec(r#"sh -c "echo \"hi\" \$HOME""#).unwrap(),
        v(&["sh", "-c", r#"echo "hi" $HOME"#])
    );
    for code in ["%d", "%D", "%n", "%N", "%v", "%m", "%z"] {
        assert!(parse_exec(&format!("app {code}")).is_err(), "{code}");
    }
    assert!(parse_exec("").is_err());
    assert!(parse_exec(r#"app "unterminated"#).is_err());
}

#[test]
fn split_command_keeps_percent() {
    assert_eq!(split_command("echo 100%").unwrap(), v(&["echo", "100%"]));
    assert!(split_command("sh -c 'a b'").is_err());
}

const HOSTILE: [&str; 11] = [
    "a b", "\"", "\\", "$HOME", "`x`", "50%", "''", ";rm -rf", "é", "\"\"", "--x=%U",
];

#[test]
fn write_exec_round_trips() {
    let tokens: Vec<Token> = std::iter::once(arg("prog"))
        .chain(HOSTILE.iter().map(|a| arg(a)))
        .collect();
    let line = write_exec(&tokens);
    assert!(!line.contains('\n'));
    let back = exec_argv(&unescape_value(&line), &FieldCtx::default()).unwrap();
    let mut want = vec!["prog".to_string()];
    want.extend(HOSTILE.iter().map(|s| s.to_string()));
    assert_eq!(back, want);
    assert_eq!(write_exec(&[arg("code"), Token::Field('F')]), "code %F");
}

#[test]
fn write_exec_escapes_control_chars() {
    for a in ["a\nb", "x\tY=1", "c\rd"] {
        let line = write_exec(&[arg("p"), arg(a)]);
        assert!(!line.contains(['\n', '\r', '\t']), "{line:?}");
        let back = exec_argv(&unescape_value(&line), &FieldCtx::default()).unwrap();
        assert_eq!(back, v(&["p", a]));
    }
}

#[test]
fn quote_command_round_trips() {
    let argv = v(&["backup-check", "--quiet", "a b", "100%", "c\\d"]);
    let text = quote_command(&argv);
    assert!(text.contains("100%") && !text.contains("100%%"), "{text}");
    assert_eq!(split_command(&text).unwrap(), argv);
}

#[test]
fn clean_display_strips_controls_and_bidi() {
    assert_eq!(
        clean_display("a\u{7}b\nc\u{202e}d\u{2066}e", 50),
        "a b c d e"
    );
    assert_eq!(clean_display("abcdef", 3), "abc");
    assert_eq!(
        clean_display("a\u{2028}b\u{2029}c\u{200e}d\u{200f}e\u{61c}f", 50),
        "a b c d e f"
    );
}

#[test]
fn read_entry_reads_lists_and_gnome_keys() {
    let e = read_entry(
        "[Desktop Entry]\nType=Application\nName=X\nExec=x %U\nOnlyShowIn=GNOME;KDE;\n\
         NotShowIn=uQ\nX-GNOME-Autostart-Delay=5\nX-GNOME-Autostart-enabled=false\n\
         X-uQ-Override=true\nStartupWMClass=Code\nTerminal=true\nHidden=false\n\
         [Desktop Action a]\nName=Other\nExec=y\n",
    );
    assert_eq!(e.kind.as_deref(), Some("Application"));
    assert_eq!(e.name.as_deref(), Some("X"));
    assert_eq!(e.exec.as_deref(), Some("x %U"));
    assert_eq!(e.only_show_in, Some(v(&["GNOME", "KDE"])));
    assert_eq!(e.not_show_in, Some(v(&["uQ"])));
    assert_eq!(e.gnome_delay, Some(5));
    assert_eq!(e.gnome_enabled, Some(false));
    assert!(e.uq_override && e.terminal && !e.hidden);
    assert_eq!(e.wm_class.as_deref(), Some("Code"));
}

fn show(text: &str) -> bool {
    shows_in(
        &read_entry(&format!("[Desktop Entry]\n{text}")),
        &UQ_DESKTOPS,
    )
}

#[test]
fn shows_in_rules() {
    assert!(show("Name=x\n"));
    assert!(show("OnlyShowIn=\n"));
    assert!(show("NotShowIn=\n"));
    assert!(!show("OnlyShowIn=GNOME;\n"));
    assert!(show("OnlyShowIn=Hyprland;\n"));
    assert!(!show("NotShowIn=uQ;\n"));
    assert!(show("NotShowIn=GNOME;\n"));
    assert!(show("OnlyShowIn=uQ;\nNotShowIn=uQ;\n"));
    assert!(!show("OnlyShowIn=GNOME;\nNotShowIn=KDE;\n"));
}

#[test]
fn desktop_ids_refuse_traversal() {
    let long = "a".repeat(129);
    for bad in [
        "../x.desktop",
        "..desktop",
        ".hidden.desktop",
        "a/b.desktop",
        "x.desktop\0",
        "",
        long.as_str(),
    ] {
        assert!(!valid_desktop_id(bad), "{bad:?}");
    }
    assert!(valid_desktop_id("org.mozilla.firefox.desktop"));
    assert!(valid_desktop_id("code"));
    assert_eq!(normalise_id("code"), "code.desktop");
    assert_eq!(normalise_id("code.desktop"), "code.desktop");
}

#[test]
fn set_key_replaces_only_that_key() {
    let text = "# top\n[Desktop Entry]\nName=A\nName[de]=B\nExec=old\n# c\n\n[Other]\nExec=keep\n";
    assert_eq!(
        set_key(text, "Desktop Entry", "Exec", "new"),
        "# top\n[Desktop Entry]\nName=A\nName[de]=B\nExec=new\n# c\n\n[Other]\nExec=keep\n"
    );
    assert_eq!(
        set_key(text, "Desktop Entry", "Hidden", "true"),
        "# top\n[Desktop Entry]\nName=A\nName[de]=B\nExec=old\n# c\nHidden=true\n\n[Other]\nExec=keep\n"
    );
    assert_eq!(
        set_key("[Desktop Entry]\nName=A", "Desktop Entry", "Name[de]", "Z"),
        "[Desktop Entry]\nName=A\nName[de]=Z\n"
    );
    assert_eq!(
        set_key("[Desktop Entry]\nName=A\n", "New", "K", "v"),
        "[Desktop Entry]\nName=A\n\n[New]\nK=v\n"
    );
}

#[test]
fn groups_with_exec_lists_groups() {
    let text = "[Desktop Entry]\nExec=a\n[Desktop Action x]\nName=n\n[Desktop Action y]\nExec=b\n[Other]\nExec=c\n";
    assert_eq!(
        groups_with_exec(text),
        v(&["Desktop Entry", "Desktop Action y"])
    );
}

#[test]
fn unit_id_matches_old_scope_names() {
    assert_eq!(unit_id("org.mozilla.firefox"), "org_mozilla_firefox");
    let long = "x".repeat(100);
    assert_eq!(unit_id(&format!("{long}/../a b")), "x".repeat(64));
}

#[test]
fn written_exec_survives_set_key_and_read_entry() {
    let argv = ["my app", "a\"b", "100%", "x\ty", "$HOME", "--x=%U"];
    let mut tokens: Vec<Token> = argv.iter().map(|a| arg(a)).collect();
    tokens.push(Token::Field('F'));
    let text = "[Desktop Entry]\nName=A\nExec=old\n";
    let out = set_key(text, "Desktop Entry", "Exec", &write_exec(&tokens));
    let exec = read_entry(&out).exec.unwrap();
    let t = exec_tokens(&exec).unwrap();
    assert!(!t.lossy);
    assert_eq!(t.tokens, tokens);
}
