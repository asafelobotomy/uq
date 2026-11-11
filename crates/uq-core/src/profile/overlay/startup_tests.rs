//! The overlay with startup apps, environment variables and app fixes (D1, F28).

use super::*;
use crate::history::Entry;
use serde_json::json;
use std::collections::BTreeSet;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn profile(body: &str) -> Profile {
    Profile::parse(&format!("format = 1\nid = \"p\"\nname = \"P\"\n{body}")).unwrap()
}

fn known(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|x| x.to_string()).collect()
}

fn env_state(vars: &[(&str, &str)]) -> State {
    let mut st = State::default();
    let vars: Vec<Value> = vars
        .iter()
        .map(|(n, v)| json!({"name": n, "value": v}))
        .collect();
    st.set_section("env", json!({ "vars": vars })).unwrap();
    st
}

fn names(st: &State) -> Vec<(String, String)> {
    st.env
        .vars
        .iter()
        .map(|v| (v.name.clone(), v.value.clone()))
        .collect()
}

#[test]
fn apply_skips_unknown_startup_ids() {
    let mut cur = State::default();
    cur.set_section(
        "autostart",
        json!({"files": [], "added": [{"id": "a-3f9c", "name": "Sync", "argv": ["sync"]}]}),
    )
    .unwrap();
    let p = profile(
        "[autostart.\"nm-applet.desktop\"]\nenabled = false\n\
         [autostart.\"gone.desktop\"]\nenabled = false\ndelay = 5\n",
    );
    let text = p.to_toml();
    let k = known(&["nm-applet.desktop", "a-3f9c"]);
    let (next, active) = apply(&cur, &p, &k, 0).unwrap();
    assert_eq!(
        read_leaf(&next, "autostart", &s(&["nm-applet.desktop", "enabled"])),
        Some(json!(false))
    );
    assert_eq!(next.autostart.choice("gone.desktop"), None);
    assert_eq!(
        active.keys,
        vec![KeyMemo {
            domain: "autostart".into(),
            path: s(&["nm-applet.desktop", "enabled"]),
            before: None,
            set: json!(false),
        }]
    );
    assert_eq!(super::super::missing_startup(&k, &p), 1);
    assert_eq!(p.to_toml(), text);
    // The added entry turned off and back.
    let p = profile("[autostart.\"a-3f9c\"]\nenabled = false\ndelay = 10\n");
    let (on, active) = apply(&cur, &p, &k, 0).unwrap();
    assert!(!on.autostart.added[0].enabled());
    assert_eq!(on.autostart.added[0].delay(), Some(10));
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert_eq!(off, cur);
    // Deleted while the profile was on: nothing to put back, nothing kept.
    let mut gone = on.clone();
    gone.set_section("autostart", json!({"files": [], "added": []}))
        .unwrap();
    let (off, kept) = turn_off(&gone, &active).unwrap();
    assert!(kept.is_empty() && off == gone, "{kept:?}");
}

#[test]
fn turn_off_env_k3() {
    let cur = env_state(&[("FOO", "x"), ("BAR", "y")]);
    let p = profile("[env]\nFOO = \"a\"\nBAR = false\nNEW = \"1\"\n");
    let (on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert_eq!(
        names(&on),
        vec![("FOO".into(), "a".into()), ("NEW".into(), "1".into())]
    );
    let bar = active.keys.iter().find(|m| m.path == s(&["BAR"])).unwrap();
    assert_eq!(
        (bar.before.clone(), bar.set.clone()),
        (Some(json!("y")), json!(false))
    );
    let new = active.keys.iter().find(|m| m.path == s(&["NEW"])).unwrap();
    assert_eq!(new.before, Some(json!(false)));
    // By hand: FOO changed to b.
    let mut on = on;
    write_leaves(&mut on, "env", &[(s(&["FOO"]), Some(json!("b")))]).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert_eq!(
        names(&off),
        vec![("FOO".into(), "b".into()), ("BAR".into(), "y".into())]
    );
    assert_eq!(
        kept,
        vec![Kept {
            domain: "env".into(),
            path: s(&["FOO"]),
            label: "Environment variable FOO".into(),
        }]
    );
}

#[test]
fn turn_off_compat_created_fix() {
    let cur = State::default();
    let p = profile("[compat.apps.\"code.desktop\"]\nplatform = \"x11\"\n");
    let (on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert_eq!(on.compat.apps.len(), 1);
    assert_eq!(active.keys.len(), 1);
    assert_eq!(active.keys[0].before, Some(json!("auto")));
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert!(off.compat.apps.is_empty());
    assert_eq!(off, cur);
    // Options added by hand meanwhile stay; only the profile's platform goes.
    let mut on = on;
    let mut c = on.section("compat").unwrap();
    c["apps"][0]["args"] = json!(["--x"]);
    on.set_section("compat", c).unwrap();
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    let fix = off.compat.fix_for("code.desktop").unwrap();
    assert_eq!(fix.args, vec!["--x".to_string()]);
    assert_eq!(fix.platform, crate::compat::Platform::Auto);
}

#[test]
fn turn_off_compat_switch_and_scale() {
    let mut cur = State::default();
    cur.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "wayland", "elsewhere": true}]}),
    )
    .unwrap();
    let p = profile("[compat]\nsharp_x11 = true\n[compat.apps.\"code.desktop\"]\nscale = 1.5\n");
    let (on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert!(on.compat.sharp_x11);
    let fix = on.compat.fix_for("code.desktop").unwrap();
    assert_eq!((fix.scale, fix.elsewhere), (Some(1.5), true));
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty());
    assert_eq!(off, cur);
}

#[test]
fn too_many_variables_refused() {
    let vars: Vec<(String, String)> = (0..100).map(|i| (format!("V{i}"), "x".into())).collect();
    let refs: Vec<(&str, &str)> = vars.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let cur = env_state(&refs);
    let p = profile("[env]\nEXTRA = \"1\"\n");
    let e = apply(&cur, &p, &known(&[]), 0).unwrap_err();
    assert_eq!(e, ProfileError::TooManyVars);
    assert_eq!(
        e.to_string(),
        "This profile would make more than 100 environment variables."
    );
    // Replacing one, or removing one, is fine at the limit.
    let p = profile("[env]\nV3 = \"y\"\nV4 = false\n");
    let (next, _) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert_eq!(next.env.vars.len(), 99);
}

#[test]
fn a_created_fix_counts_toward_the_limit() {
    let apps: Vec<Value> = (0..100)
        .map(|i| json!({"id": format!("a{i}.desktop"), "platform": "x11"}))
        .collect();
    let mut cur = State::default();
    cur.set_section("compat", json!({ "apps": apps })).unwrap();
    let p = profile("[compat.apps.\"new.desktop\"]\nplatform = \"x11\"\n");
    let e = apply(&cur, &p, &known(&[]), 0).unwrap_err();
    assert!(
        matches!(&e, ProfileError::Invalid { reason, .. } if reason.contains("100")),
        "{e:?}"
    );
    assert!(!e.to_string().contains(".desktop"), "{e}");
}

fn entry(time: i64, domain: &str, before: Value, after: Value) -> Entry {
    Entry::new(time, "settings", domain, "", before, after)
}

#[test]
fn changed_since_offers_startup_keys() {
    let vars = |v: Value| json!({ "vars": v });
    let entries = vec![
        entry(
            30,
            "env",
            vars(json!([{"name": "A", "value": "1"}, {"name": "B", "value": "2"}])),
            vars(json!([{"name": "A", "value": "3"}])),
        ),
        entry(
            25,
            "compat",
            json!({"qt_wayland": false, "apps": [{"id": "code.desktop", "platform": "auto",
                "scale": null, "args": ["--a"]}]}),
            json!({"qt_wayland": true, "apps": [{"id": "code.desktop", "platform": "x11",
                "scale": null, "args": ["--b"]}]}),
        ),
        entry(
            20,
            "autostart",
            json!({"files": [], "added": []}),
            json!({"files": [{"id": "nm-applet.desktop", "enabled": false, "delay": null}],
                "added": [{"id": "a-1234", "name": "X", "argv": ["x"], "enabled": true}]}),
        ),
    ];
    let (keys, skipped) = changed_since(&entries, 10);
    assert_eq!(
        keys,
        vec![
            ("env".to_string(), s(&["A"])),
            ("env".to_string(), s(&["B"])),
            ("compat".to_string(), s(&["qt_wayland"])),
            (
                "compat".to_string(),
                s(&["apps", "code.desktop", "platform"])
            ),
            (
                "autostart".to_string(),
                s(&["nm-applet.desktop", "enabled"])
            ),
        ]
    );
    // The fix's options and the added entry.
    assert_eq!(skipped, 2);
}

#[test]
fn auto_platform_removes_a_fix_and_back_puts_it_back() {
    let mut cur = State::default();
    cur.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "x11"}]}),
    )
    .unwrap();
    let p = profile("[compat.apps.\"code.desktop\"]\nplatform = \"auto\"\n");
    let (on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert!(on.compat.apps.is_empty());
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty(), "not changed by hand: {kept:?}");
    assert_eq!(off, cur);
}

#[test]
fn a_fix_working_elsewhere_is_never_removed() {
    let mut cur = State::default();
    cur.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "x11", "elsewhere": true}]}),
    )
    .unwrap();
    let p = profile(
        "[compat]\nqt_wayland = true\n[compat.apps.\"code.desktop\"]\nplatform = \"auto\"\n",
    );
    assert_eq!(super::super::emptied_elsewhere(&cur, &p), ["code.desktop"]);
    let (on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    assert_eq!(on.compat.apps, cur.compat.apps);
    assert!(on.compat.qt_wayland);
    assert_eq!(active.keys.len(), 1);
    let (off, kept) = turn_off(&on, &active).unwrap();
    assert!(kept.is_empty());
    assert_eq!(off, cur);
    // Setting a size keeps the fix, so that applies.
    let p = profile("[compat.apps.\"code.desktop\"]\nplatform = \"auto\"\nscale = 2.0\n");
    assert!(super::super::emptied_elsewhere(&cur, &p).is_empty());
}

#[test]
fn back_to_normal_over_the_limits_says_what_to_do() {
    let cur = env_state(&[("BAR", "y")]);
    let p = profile("[env]\nBAR = false\n");
    let (mut on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    let vars: Vec<Value> = (0..100)
        .map(|i| json!({"name": format!("V{i}"), "value": "x"}))
        .collect();
    on.set_section("env", json!({ "vars": vars })).unwrap();
    let e = turn_off(&on, &active).unwrap_err();
    assert_eq!(e, ProfileError::OffTooManyVars);
    assert!(
        e.to_string()
            .ends_with("Remove one on the Startup page first."),
        "{e}"
    );

    let mut cur = State::default();
    cur.set_section(
        "compat",
        json!({"apps": [{"id": "code.desktop", "platform": "x11"}]}),
    )
    .unwrap();
    let p = profile("[compat.apps.\"code.desktop\"]\nplatform = \"auto\"\n");
    let (mut on, active) = apply(&cur, &p, &known(&[]), 0).unwrap();
    let apps: Vec<Value> = (0..100)
        .map(|i| json!({"id": format!("a{i}.desktop"), "platform": "x11"}))
        .collect();
    on.set_section("compat", json!({ "apps": apps })).unwrap();
    let e = turn_off(&on, &active).unwrap_err();
    assert_eq!(e, ProfileError::OffTooManyFixes);
    assert!(
        e.to_string()
            .ends_with("Remove one on the Apps and scaling page first."),
        "{e}"
    );
}
