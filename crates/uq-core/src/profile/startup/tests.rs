use super::*;
use crate::history::Entry;
use crate::profile::{
    ActiveProfile, Profile, apply, changed_since, key_label, value_text, write_leaves as write,
};
use serde_json::json;
use std::collections::BTreeSet;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn with(body: &str) -> String {
    format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
}

/// The refusal's key and its one-line, plain text.
fn refused(body: &str) -> (String, String) {
    match Profile::parse(&with(body)) {
        Err(e @ ProfileError::Invalid { .. }) => {
            let ProfileError::Invalid { key, .. } = &e else {
                unreachable!()
            };
            let text = e.to_string();
            assert!(
                !text.contains('\n') && !text.chars().any(char::is_control),
                "{text:?}"
            );
            assert!(!text.contains("env.") && !text.contains('{'), "{text:?}");
            (key.clone(), text)
        }
        other => panic!("{body:?} gave {other:?}"),
    }
}

fn state() -> State {
    let mut st = State::default();
    st.set_section(
        "env",
        json!({"vars": [{"name": "A", "value": "1"}, {"name": "B", "value": "2"}]}),
    )
    .unwrap();
    st.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "wayland", "scale": 1.5}]}),
    )
    .unwrap();
    st.set_section(
        "autostart",
        json!({"files": [{"id": "nm-applet.desktop", "enabled": false}],
            "added": [{"id": "a-3f9c", "name": "Syncthing", "app": "syncthing-start.desktop",
                "enabled": true, "delay": 10}]}),
    )
    .unwrap();
    st
}

#[test]
fn env_tree_validation() {
    let p = Profile::parse(&with("[env]\nFOO = \"bar\"\nOLD = false\n")).unwrap();
    assert_eq!(p.values["env"], json!({"FOO": "bar", "OLD": false}));
    assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    let long = format!("[env]\nA = \"{}\"\n", "x".repeat(4097));
    for (body, key) in [
        ("[env]\nfoo = \"x\"\n", "env.foo"),
        ("[env]\nXDG_SESSION_TYPE = \"x\"\n", "env.XDG_SESSION_TYPE"),
        ("[env]\nXDG_CONFIG_HOME = \"x\"\n", "env.XDG_CONFIG_HOME"),
        ("[env]\nUQ_X = false\n", "env.UQ_X"),
        ("[env]\nA = \"$HOME\"\n", "env.A"),
        ("[env]\nA = \"a\\u0001b\"\n", "env.A"),
        (long.as_str(), "env.A"),
        ("[env]\nA = true\n", "env.A"),
        ("[env]\nA = 3\n", "env.A"),
    ] {
        assert_eq!(refused(body).0, key, "{body:?}");
    }
    assert!(
        refused("[env]\nA = true\n")
            .1
            .starts_with("Environment variable A: ")
    );
    assert!(Profile::parse(&with("[env]\nLD_PRELOAD = \"x\"\n")).is_ok());
}

#[test]
fn compat_tree_validation() {
    let ok = "[compat]\nsharp_x11 = true\nqt_wayland = false\n\
              [compat.apps.\"code.desktop\"]\nplatform = \"wayland\"\nscale = 1.5\n";
    let p = Profile::parse(&with(ok)).unwrap();
    assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    let app = |field: &str| format!("[compat.apps.\"code.desktop\"]\n{field}\n");
    for (body, key) in [
        (app("scale = 1.1"), "compat.apps.code.desktop.scale"),
        (
            app("platform = \"mir\""),
            "compat.apps.code.desktop.platform",
        ),
        (app("env = []"), "compat.apps.code.desktop.env"),
        (app("args = [\"--x\"]"), "compat.apps.code.desktop.args"),
        (
            app("elsewhere = true"),
            "compat.apps.code.desktop.elsewhere",
        ),
        (
            "[compat.apps.\"../x\"]\nplatform = \"x11\"\n".to_string(),
            "compat.apps.../x",
        ),
        ("[compat]\nblur = true\n".to_string(), "compat.blur"),
    ] {
        let (k, text) = refused(&body);
        assert_eq!(k, key, "{body:?}");
        assert!(!text.contains(".desktop"), "{text}");
    }
    let (_, text) = refused(&app("args = [\"--x\"]"));
    assert!(
        text.contains("a profile can only set an app's Run on and Size"),
        "{text}"
    );
}

#[test]
fn autostart_tree_validation() {
    for ok in [
        "[autostart.\"nm-applet.desktop\"]\nenabled = false\n",
        "[autostart.\"a-3f9c\"]\ndelay = 10\n",
    ] {
        let p = Profile::parse(&with(ok)).unwrap();
        assert_eq!(Profile::parse(&p.to_toml()), Ok(p));
    }
    let e = |field: &str| format!("[autostart.\"a-3f9c\"]\n{field}\n");
    for (body, key) in [
        (e("argv = [\"x\"]"), "autostart.a-3f9c.argv"),
        (e("app = \"x.desktop\""), "autostart.a-3f9c.app"),
        (e("name = \"x\""), "autostart.a-3f9c.name"),
        (e("delay = 601"), "autostart.a-3f9c.delay"),
        (e("enabled = \"no\""), "autostart.a-3f9c.enabled"),
        (
            "[autostart.\"../x\"]\nenabled = true\n".to_string(),
            "autostart.../x",
        ),
        (
            "[autostart.\"a-ZZ\"]\nenabled = true\n".to_string(),
            "autostart.a-ZZ",
        ),
    ] {
        assert_eq!(refused(&body).0, key, "{body:?}");
    }
    let (_, text) = refused(&e("argv = [\"x\"]"));
    assert!(
        text.contains("a profile can only turn a startup app on or off or set its delay"),
        "{text}"
    );
}

#[test]
fn read_and_write_startup_leaves() {
    let mut st = state();
    // A variable replaced keeps its place; a new one goes last; `false` removes.
    write(
        &mut st,
        "env",
        &[
            (s(&["A"]), Some(json!("9"))),
            (s(&["C"]), Some(json!("3"))),
            (s(&["B"]), Some(json!(false))),
        ],
    )
    .unwrap();
    let names: Vec<_> = st.env.vars.iter().map(|v| (&*v.name, &*v.value)).collect();
    assert_eq!(names, [("A", "9"), ("C", "3")]);
    assert_eq!(read_leaf(&st, "env", &s(&["B"])), Some(json!(false)));
    assert_eq!(read_leaf(&st, "env", &s(&["C"])), Some(json!("3")));
    write(&mut st, "env", &[(s(&["C"]), None)]).unwrap();
    assert_eq!(st.env.vars.len(), 1);
    // A switch.
    write(
        &mut st,
        "compat",
        &[(s(&["steam_scale"]), Some(json!(true)))],
    )
    .unwrap();
    assert!(st.compat.steam_scale);
    assert_eq!(
        read_leaf(&st, "compat", &s(&["steam_scale"])),
        Some(json!(true))
    );
    // A fix created, then emptied: it goes.
    let p = s(&["apps", "gimp.desktop", "platform"]);
    write(&mut st, "compat", &[(p.clone(), Some(json!("x11")))]).unwrap();
    assert_eq!(read_leaf(&st, "compat", &p), Some(json!("x11")));
    assert_eq!(
        read_leaf(&st, "compat", &s(&["apps", "gimp.desktop", "scale"])),
        None
    );
    write(&mut st, "compat", &[(p.clone(), None)]).unwrap();
    assert!(st.compat.fix_for("gimp.desktop").is_none());
    // No fix runs automatically.
    assert_eq!(read_leaf(&st, "compat", &p), Some(json!("auto")));
    assert_eq!(
        read_leaf(&st, "compat", &s(&["apps", "code.desktop", "scale"])),
        Some(json!(1.5))
    );
    // A file choice and an added entry.
    let nm = |f: &str| s(&["nm-applet.desktop", f]);
    write(&mut st, "autostart", &[(nm("delay"), Some(json!(5)))]).unwrap();
    assert_eq!(read_leaf(&st, "autostart", &nm("delay")), Some(json!(5)));
    assert_eq!(
        read_leaf(&st, "autostart", &nm("enabled")),
        Some(json!(false))
    );
    write(
        &mut st,
        "autostart",
        &[(nm("enabled"), None), (nm("delay"), None)],
    )
    .unwrap();
    assert!(st.autostart.choice("nm-applet.desktop").is_none());
    let sync = |f: &str| s(&["a-3f9c", f]);
    write(
        &mut st,
        "autostart",
        &[(sync("enabled"), Some(json!(false)))],
    )
    .unwrap();
    assert!(!st.autostart.added[0].enabled());
    assert_eq!(read_leaf(&st, "autostart", &sync("delay")), Some(json!(10)));
    assert_eq!(
        read_leaf(&st, "autostart", &s(&["other.desktop", "enabled"])),
        None
    );
    // An added id with no entry is refused, and nothing changes.
    let before = st.clone();
    let e = write(
        &mut st,
        "autostart",
        &[(s(&["a-ffff", "enabled"]), Some(json!(true)))],
    );
    assert!(e.is_err());
    assert!(write(&mut st, "env", &[(s(&["lower"]), Some(json!("x")))]).is_err());
    assert!(
        write(
            &mut st,
            "compat",
            &[(s(&["apps", "code.desktop", "args"]), Some(json!([])))]
        )
        .is_err()
    );
    assert_eq!(st, before);
}

#[test]
fn labels_and_values_are_plain() {
    let st = state();
    let names = |id: &str| (id == "code.desktop").then(|| "<b>x</b>".to_string());
    let label = |d: &str, p: &[&str]| startup_label(&st, d, &s(p), &names);
    assert_eq!(label("env", &["FOO"]), "Environment variable FOO");
    assert_eq!(label("compat", &["qt_wayland"]), "Qt apps prefer Wayland");
    assert_eq!(
        label("compat", &["apps", "code.desktop", "platform"]),
        "Run on for \u{201c}<b>x</b>\u{201d}"
    );
    assert_eq!(
        label("compat", &["apps", "gimp.desktop", "scale"]),
        "Size for \u{201c}gimp\u{201d}"
    );
    assert_eq!(
        label("autostart", &["a-3f9c", "enabled"]),
        "Start \u{201c}Syncthing\u{201d} at login"
    );
    assert_eq!(
        label("autostart", &["nm-applet.desktop", "delay"]),
        "Start delay for \u{201c}nm-applet\u{201d}"
    );
    assert_eq!(
        label("autostart", &["gone.desktop", "enabled"]),
        "A startup app that isn't on this computer"
    );
    assert_eq!(
        key_label("autostart", &s(&["nm-applet.desktop", "enabled"])),
        "Start \u{201c}nm-applet\u{201d} at login"
    );
    assert_eq!(key_label("env", &s(&["FOO"])), "Environment variable FOO");
    let v = |d: &str, p: &[&str], v: Value| value_text(d, &s(p), &v);
    assert_eq!(v("env", &["FOO"], json!(false)), "Removed");
    assert_eq!(v("env", &["FOO"], json!("bar")), "bar");
    assert_eq!(v("env", &["FOO"], json!("a\u{202E}b\u{2066}c")), "a b c");
    assert_eq!(
        v("compat", &["apps", "c.desktop", "platform"], json!("auto")),
        "Automatic"
    );
    assert_eq!(
        v("compat", &["apps", "c.desktop", "platform"], json!("x11")),
        "X11"
    );
    assert_eq!(
        v("compat", &["apps", "c.desktop", "scale"], json!(1.5)),
        "150 %"
    );
    assert_eq!(v("autostart", &["a-3f9c", "delay"], json!(10)), "10 s");
    assert_eq!(v("compat", &["sharp_x11"], json!(true)), "On");
    for k in startup_keys_of(&st) {
        for text in [
            startup_label(&st, &k.domain, &k.path, &|_| None),
            key_label(&k.domain, &k.path),
            value_text(&k.domain, &k.path, &k.value),
        ] {
            assert!(
                !text.contains(".desktop") && !text.contains("env.") && !text.contains('{'),
                "{k:?}: {text}"
            );
        }
    }
    assert_eq!(
        missing_startup_notice(1),
        "1 startup app in this profile isn't on this computer, so it was left out."
    );
    assert_eq!(
        missing_startup_notice(2),
        "2 startup apps in this profile aren't on this computer, so they were left out."
    );
    let p = Profile::parse(&with(
        "[env]\nA = \"1\"\n[autostart.\"a-1234\"]\nenabled = true\n",
    ))
    .unwrap();
    assert_eq!(
        startup_notice(&p),
        [
            "Apps you start from now on use this profile's settings; apps already open keep theirs.",
            "Startup apps change next time you log in.",
        ]
    );
}

#[test]
fn startup_keys_of_lists_what_the_state_holds() {
    let keys: Vec<(String, Vec<String>)> = startup_keys_of(&state())
        .into_iter()
        .map(|k| (k.domain, k.path))
        .collect();
    let has = |d: &str, p: &[&str]| keys.contains(&(d.to_string(), s(p)));
    assert!(has("env", &["A"]) && has("env", &["B"]));
    assert!(has("compat", &["sharp_x11"]) && has("compat", &["steam_scale"]));
    assert!(has("compat", &["apps", "code.desktop", "platform"]));
    assert!(has("compat", &["apps", "code.desktop", "scale"]));
    assert!(has("autostart", &["nm-applet.desktop", "enabled"]));
    assert!(!has("autostart", &["nm-applet.desktop", "delay"]));
    assert!(has("autostart", &["a-3f9c", "enabled"]) && has("autostart", &["a-3f9c", "delay"]));
    assert_eq!(keys.len(), 2 + 5 + 2 + 1 + 2);
}

#[test]
fn startup_keys_classify() {
    let (keys, other) = startup_keys(
        "env",
        &json!({"vars": [{"name": "A", "value": "1"}, {"name": "B", "value": "2"},
            {"name": "C", "value": "3"}]}),
        &json!({"vars": [{"name": "C", "value": "3"}, {"name": "A", "value": "9"}]}),
    );
    assert_eq!(
        keys,
        [
            ("env".to_string(), s(&["A"])),
            ("env".to_string(), s(&["B"]))
        ]
    );
    assert_eq!(other, ["order:vars"]);
    let fix =
        |args: Value| json!({"apps": [{"id": "c.desktop", "platform": "auto", "args": args}]});
    let (keys, other) = startup_keys("compat", &fix(json!(["--a"])), &fix(json!(["--b"])));
    assert!(keys.is_empty());
    assert_eq!(other, ["apps:c.desktop"]);
    let added = |list: Value| json!({"files": [], "added": list});
    let e = |id: &str, on: bool| json!({"id": id, "name": "N", "argv": ["n"], "enabled": on});
    let (keys, other) = startup_keys(
        "autostart",
        &added(json!([e("a-0001", true)])),
        &added(json!([e("a-0001", false), e("a-0002", true)])),
    );
    assert_eq!(keys, [("autostart".to_string(), s(&["a-0001", "enabled"]))]);
    assert_eq!(other, ["added:a-0002"]);
    // Through `changed_since`, each other change is counted once.
    let entries = [
        Entry::new(
            3,
            "x",
            "compat",
            "",
            fix(json!(["--a"])),
            fix(json!(["--b"])),
        ),
        Entry::new(
            2,
            "x",
            "compat",
            "",
            fix(json!(["--c"])),
            fix(json!(["--a"])),
        ),
    ];
    assert_eq!(changed_since(&entries, 0), (vec![], 1));
}

#[test]
fn mark_startup_only_overrides_uq() {
    let p = Profile::parse(&with(
        "[env]\nA = \"5\"\nB = \"2\"\n[compat]\nqt_wayland = true\n\
         [compat.apps.\"code.desktop\"]\nscale = 2.0\n[autostart.\"a-3f9c\"]\nenabled = false\n",
    ))
    .unwrap();
    let known: BTreeSet<String> = ["a-3f9c".to_string()].into();
    let (mut st, active): (State, ActiveProfile) = apply(&state(), &p, &known, 0).unwrap();
    let row = |src: &str| json!({"source": src, "live": null, "stored": null, "detail": {}});
    let mut env = json!({"vars.A": row("uq"), "vars.B": row("user_lua")});
    mark_startup(&mut env, "env", &st, &active);
    assert_eq!(env["vars.A"]["source"], "profile");
    assert_eq!(env["vars.A"]["detail"], json!({"name": "X"}));
    assert_eq!(env["vars.B"]["source"], "user_lua");
    let mut compat = json!({"qt_wayland": row("uq"), "sharp_x11": row("uq"),
        "apps.code.desktop": row("uq"), "compat": row("error")});
    mark_startup(&mut compat, "compat", &st, &active);
    assert_eq!(compat["qt_wayland"]["source"], "profile");
    assert_eq!(compat["sharp_x11"]["source"], "uq");
    assert_eq!(compat["apps.code.desktop"]["source"], "profile");
    assert_eq!(compat["compat"]["source"], "error");
    let mut auto = json!({"a-3f9c": row("uq"), "nm-applet.desktop": row("uq")});
    mark_startup(&mut auto, "autostart", &st, &active);
    assert_eq!(auto["a-3f9c"]["source"], "profile");
    assert_eq!(auto["nm-applet.desktop"]["source"], "uq");
    // Changed by hand: no longer the profile's.
    write(&mut st, "env", &[(s(&["A"]), Some(json!("6")))]).unwrap();
    let mut env = json!({"vars.A": row("uq")});
    mark_startup(&mut env, "env", &st, &active);
    assert_eq!(env["vars.A"]["source"], "uq");
}
