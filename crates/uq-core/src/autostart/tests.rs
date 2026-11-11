use super::scan::{Found, Plan, Resolved, Source, hidden_copy, known_ids, resolve, wanted_copies};
use super::*;
use crate::compat::Compat;
use crate::desktop_entry::read_entry;
use crate::settings::SettingError;
use crate::state::State;
use serde_json::json;
use std::path::PathBuf;

mod folders;
mod scan;

fn reason(e: SettingError) -> String {
    match e {
        SettingError::Invalid { reason, .. } => reason,
        other => panic!("not Invalid: {other:?}"),
    }
}

fn refused(v: serde_json::Value) -> String {
    reason(Autostart::from_section(&v, &Autostart::default()).expect_err("should be refused"))
}

fn ok(v: serde_json::Value) -> Autostart {
    Autostart::from_section(&v, &Autostart::default()).expect("a valid section")
}

fn added(entry: serde_json::Value) -> serde_json::Value {
    json!({ "added": [entry] })
}

fn cmd(argv: serde_json::Value) -> serde_json::Value {
    added(json!({"name": "Thing", "argv": argv}))
}

const SYS: &str = "/etc/xdg/autostart";
const USER: &str = "/home/u/.config/autostart";

fn sys(id: &str, text: &str) -> Found {
    Found {
        id: id.into(),
        user: None,
        system: Some((PathBuf::from(format!("{SYS}/{id}")), text.into())),
    }
}

fn user(id: &str, text: &str) -> Found {
    Found {
        id: id.into(),
        user: Some((PathBuf::from(format!("{USER}/{id}")), text.into())),
        system: None,
    }
}

fn entry(exec: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName=Thing\nExec={exec}\n{extra}")
}

fn run(found: &[Found], state: &Autostart) -> Vec<Resolved> {
    resolve(found, state, &Compat::default(), &|_| None, &|_| true)
}

fn one(found: Found, state: &Autostart) -> Resolved {
    let mut r = run(&[found], state);
    assert_eq!(r.len(), 1, "{r:?}");
    r.remove(0)
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn off(id: &str) -> Autostart {
    ok(json!({"files": [{"id": id, "enabled": false}]}))
}

#[test]
fn section_round_trips_and_ids_assigned() {
    let a = ok(json!({
        "files": [{"id": "nm-applet.desktop", "enabled": false, "delay": null},
                  {"id": "blueman", "delay": 30}],
        "added": [
            {"name": "Syncthing", "app": "syncthing-start.desktop", "enabled": true, "delay": 10},
            {"id": "a-0b12", "name": "Backup check", "argv": ["backup-check", "--quiet"],
             "enabled": true, "delay": null}
        ]
    }));
    let fresh = a.added[0].id().to_string();
    assert!(valid_added_id(&fresh), "{fresh}");
    assert_ne!(fresh, "a-0b12");
    assert_eq!(a.added[1].id(), "a-0b12");
    assert_eq!(a.files[1].id, "blueman.desktop");
    assert_eq!(a.choice("nm-applet.desktop").unwrap().enabled, Some(false));
    assert!(a.choice("other.desktop").is_none());

    let sec = a.section();
    assert_eq!(
        sec["files"][0],
        json!({"id": "nm-applet.desktop", "enabled": false, "delay": null})
    );
    assert_eq!(
        sec["added"][0],
        json!({"id": fresh, "name": "Syncthing", "app": "syncthing-start.desktop",
               "enabled": true, "delay": 10})
    );
    assert_eq!(
        sec["added"][1],
        json!({"id": "a-0b12", "name": "Backup check", "argv": ["backup-check", "--quiet"],
               "enabled": true, "delay": null})
    );
    // A resend keeps every id.
    assert_eq!(Autostart::from_section(&sec, &a).unwrap(), a);

    // Through state.toml and back.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("state.toml");
    let mut st = State::default();
    st.set_section("autostart", sec.clone()).unwrap();
    st.save(&p).unwrap();
    let back = State::load(&p);
    assert_eq!(back.autostart, a);
    assert_eq!(back.section("autostart").unwrap(), sec);

    // Lenient load: a bad entry costs only itself; a bad id is replaced.
    let st = State::parse_strict(
        "[autostart]\n\
         [[autostart.files]]\nid = \"../x\"\nenabled = false\n\
         [[autostart.files]]\nid = \"ok.desktop\"\nenabled = false\n\
         [[autostart.added]]\nid = \"a-12\"\nname = \"Good\"\nargv = [\"good\"]\nenabled = true\n\
         [[autostart.added]]\nname = \"Both\"\nargv = [\"x\"]\napp = \"y.desktop\"\nenabled = true\n",
    )
    .unwrap();
    assert_eq!(st.autostart.files.len(), 1);
    assert_eq!(st.autostart.added.len(), 1);
    assert!(valid_added_id(st.autostart.added[0].id()));
    assert_eq!(st.autostart.added[0].name(), "Good");
}

#[test]
fn fresh_ids_have_the_prefix() {
    let taken = std::collections::BTreeSet::from(["a-0000".to_string()]);
    for _ in 0..50 {
        let id = crate::rules::fresh_prefixed("a-", &taken);
        assert!(valid_added_id(&id) && id != "a-0000", "{id}");
    }
    assert!(crate::rules::valid_id(&crate::rules::fresh_id(&taken)));
    for bad in [
        "a-",
        "a-123",
        "a-123456789",
        "a-ABCD",
        "r-1234",
        "a-12g4",
        " a-1234",
    ] {
        assert!(!valid_added_id(bad), "{bad}");
    }
    assert!(valid_added_id("a-1234") && valid_added_id("a-12345678"));
}

#[test]
fn added_needs_app_or_argv_not_both() {
    let both = refused(added(
        json!({"name": "X", "app": "x.desktop", "argv": ["x"]}),
    ));
    assert!(both.contains("not both"), "{both}");
    let neither = refused(added(json!({"name": "X"})));
    assert!(neither.contains("an app or a command"), "{neither}");
    refused(added(json!({"name": "X", "app": "../x.desktop"})));
    refused(added(json!({"name": "X", "app": 3})));
    let a = ok(added(json!({"name": "X", "app": "x"})));
    assert!(matches!(&a.added[0], Added::App { app, .. } if app == "x.desktop"));
    assert!(a.added[0].enabled(), "enabled by default");
    refused(added(json!({"name": "X", "argv": ["x"], "more": 1})));
    refused(added(json!({"id": "r-1234", "name": "X", "argv": ["x"]})));
    refused(json!({"added": [
        {"id": "a-1234", "name": "X", "argv": ["x"]},
        {"id": "a-1234", "name": "Y", "argv": ["y"]}
    ]}));
    let many: Vec<_> = (0..51)
        .map(|i| json!({"name": format!("N{i}"), "argv": ["x"]}))
        .collect();
    refused(json!({ "added": many }));
}

#[test]
fn argv_checked() {
    let eq = refused(cmd(json!(["A=B", "x"])));
    assert!(eq.contains('='), "{eq}");
    assert!(ok(cmd(json!(["x", "A=B"]))).added.len() == 1);
    let too_many: Vec<String> = (0..65).map(|i| format!("a{i}")).collect();
    refused(cmd(json!(too_many)));
    let max: Vec<String> = (0..64).map(|i| format!("a{i}")).collect();
    ok(cmd(json!(max)));
    refused(cmd(json!([])));
    refused(cmd(json!([""])));
    refused(cmd(json!(["x", ""])));
    refused(cmd(json!(["x", "a\nb"])));
    refused(cmd(json!(["x", "a\tb"])));
    refused(cmd(json!(["x".repeat(1025)])));
    ok(cmd(json!(["x".repeat(1024)])));
    refused(cmd(json!(["x", 3])));
    refused(cmd(json!("x y")));
}

#[test]
fn delay_range() {
    for good in [json!(0), json!(600), json!(null)] {
        ok(added(json!({"name": "X", "argv": ["x"], "delay": good})));
        ok(json!({"files": [{"id": "a.desktop", "enabled": true, "delay": good}]}));
    }
    for bad in [json!(601), json!(-1), json!(1.5), json!("10")] {
        let r = refused(added(json!({"name": "X", "argv": ["x"], "delay": bad})));
        assert!(r.contains("600"), "{r}");
        refused(json!({"files": [{"id": "a.desktop", "delay": bad}]}));
    }
}

#[test]
fn files_rules() {
    refused(json!({"files": [{"id": "../x.desktop", "enabled": false}]}));
    refused(json!({"files": [{"id": "a.desktop", "enabled": "no"}]}));
    refused(json!({"files": [{"id": "a.desktop"}]}));
    refused(json!({"files": [{"id": "a.desktop", "enabled": null, "delay": null}]}));
    refused(json!({"files": [
        {"id": "a.desktop", "enabled": false}, {"id": "a", "enabled": true}
    ]}));
    let many: Vec<_> = (0..201)
        .map(|i| json!({"id": format!("x{i}.desktop"), "enabled": false}))
        .collect();
    refused(json!({ "files": many }));
    refused(json!({"other": []}));
    refused(json!([]));

    let mut a = ok(json!({"files": [
        {"id": "a.desktop", "enabled": false}, {"id": "b.desktop", "delay": 3}
    ]}));
    a.files[0].enabled = None;
    a.prune_empty_choices();
    assert_eq!(a.files.len(), 1);
    assert_eq!(a.files[0].id, "b.desktop");
}

#[test]
fn name_refuses_control_chars() {
    for bad in ["a\nb", "a\u{7f}", "\t", "a\rb", ""] {
        refused(added(json!({"name": bad, "argv": ["x"]})));
    }
    refused(added(json!({"name": "x".repeat(101), "argv": ["x"]})));
    ok(added(json!({"name": "x".repeat(100), "argv": ["x"]})));
    ok(added(json!({"name": "Sync – files ✓", "argv": ["x"]})));
}

#[test]
fn setters_change_added_entries() {
    let mut a = ok(cmd(json!(["x"])));
    a.added[0].set_enabled(false);
    a.added[0].set_delay(Some(9));
    assert!(!a.added[0].enabled());
    assert_eq!(a.added[0].delay(), Some(9));
    assert_eq!(a.added[0].name(), "Thing");
}

#[test]
fn refusals_are_plain() {
    let bad = [
        json!([]),
        json!({"other": 1}),
        json!({"files": 3}),
        json!({"files": [1]}),
        json!({"files": [{"id": "../x"}]}),
        json!({"files": [{"id": "a.desktop"}]}),
        json!({"files": [{"id": "a.desktop", "delay": 900}]}),
        json!({"files": [{"id": "a.desktop", "enabled": 1}]}),
        json!({"added": 3}),
        json!({"added": [1]}),
        added(json!({"name": "X"})),
        added(json!({"name": "X", "app": "x", "argv": ["x"]})),
        added(json!({"name": "", "argv": ["x"]})),
        added(json!({"name": 3, "argv": ["x"]})),
        added(json!({"argv": ["x"]})),
        added(json!({"name": "X", "app": "a/b"})),
        added(json!({"id": "x", "name": "X", "argv": ["x"]})),
        added(json!({"name": "X", "argv": ["A=B"]})),
        added(json!({"name": "X", "argv": [""]})),
        added(json!({"name": "X", "argv": ["a\u{1b}"]})),
        added(json!({"name": "X", "argv": [1]})),
        added(json!({"name": "X", "argv": ["x"], "enabled": "yes"})),
        added(json!({"name": "X", "argv": ["x"], "delay": -1})),
    ];
    let mut msgs: Vec<String> = bad.into_iter().map(refused).collect();
    msgs.extend(
        [
            "Turned off by the system",
            "Turned off by a file in your autostart folder",
            "Not for this desktop",
            "Not installed",
            "Needs a terminal, which uQ doesn't open at login",
            "Its command can't be read",
            "Not an app",
        ]
        .map(String::from),
    );
    for m in msgs {
        assert!(!m.is_empty());
        for bad in [
            "regex",
            "[a-",
            "state.toml",
            "^",
            "Some(",
            "argv",
            "Exec",
            "lossy",
            "serde",
            "autostart.",
            "invalid type",
            "{",
            "[0",
            "TryExec",
            "OnlyShowIn",
            "Hidden",
        ] {
            assert!(!m.contains(bad), "{m:?} contains {bad:?}");
        }
    }
}
