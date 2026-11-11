use super::*;
use crate::profile::{KeyMemo, read_leaf, write_leaves};
use serde_json::json;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn with(body: &str) -> String {
    format!("format = 1\nid = \"x\"\nname = \"X\"\n{body}")
}

fn custom(name: &str, combo: &str) -> Value {
    json!({"name": name, "combo": combo, "action": {"kind": "uq", "name": "terminal"}})
}

/// One shortcut of your own on Super + F5 and `uq.close` turned off.
fn state() -> State {
    let mut st = State::default();
    st.set_section(
        "keybinds",
        json!({"overrides": [{"id": "uq.close", "enabled": false},
                             {"id": "uq.float", "combo": "SUPER + SHIFT + F"}],
               "custom": [custom("Terminal too", "SUPER + F5")]}),
    )
    .unwrap();
    st
}

fn custom_id(st: &State) -> String {
    st.keybinds.custom[0].id.clone()
}

#[test]
fn keybinds_tree_validation() {
    let p = Profile::parse(&with(
        "[keybinds]\n\"uq.spotlight\" = false\n\"k-3f9c\" = true\n",
    ))
    .unwrap();
    assert_eq!(
        p.values["keybinds"],
        json!({"uq.spotlight": false, "k-3f9c": true})
    );
    let text = p.to_toml();
    assert!(text.contains("[keybinds]"), "{text}");
    assert_eq!(Profile::parse(&text), Ok(p));
    for (body, key, reason) in [
        (
            "[keybinds]\n\"uq.spotlight\" = \"SUPER + K\"\n",
            "keybinds.uq.spotlight",
            ONLY_ON_OFF,
        ),
        (
            "[keybinds.\"k-3f9c\"]\ncommand = \"x\"\n",
            "keybinds.k-3f9c",
            ONLY_ON_OFF,
        ),
        (
            "[keybinds]\n\"uq.spotlight\" = [true]\n",
            "keybinds.uq.spotlight",
            ONLY_ON_OFF,
        ),
        (
            "[keybinds]\n\"uq.nope\" = true\n",
            "keybinds.uq.nope",
            NOT_AN_ID,
        ),
        ("[keybinds]\n\"k-ZZ\" = true\n", "keybinds.k-ZZ", NOT_AN_ID),
        (
            "[keybinds]\nterminal = true\n",
            "keybinds.terminal",
            NOT_AN_ID,
        ),
        (
            "[keybinds]\n\"uq.lock-recovery\" = false\n",
            "keybinds.uq.lock-recovery",
            RECOVERY_REFUSED,
        ),
    ] {
        let e = Profile::parse(&with(body)).unwrap_err();
        assert_eq!(e, invalid(key, reason), "{body:?}");
        let text = e.to_string();
        assert!(!text.contains('\n'), "{text:?}");
        assert_eq!(text, format!("A shortcut: {reason}"), "{body:?}");
    }
    // Turning the recovery shortcut on is allowed, if pointless.
    assert!(Profile::parse(&with("[keybinds]\n\"uq.lock-recovery\" = true\n")).is_ok());
    // A key holding a control character can't carry it into the error.
    let e = Profile::parse(&with("[keybinds]\n\"uq.\\u001b[2Jx\" = true\n")).unwrap_err();
    let text = e.to_string();
    assert!(!text.chars().any(char::is_control), "{text:?}");
    assert_eq!(text, format!("A shortcut: {NOT_AN_ID}"));
}

#[test]
fn read_and_write_bind_leaves() {
    let mut st = state();
    let id = custom_id(&st);
    assert_eq!(
        read_leaf(&st, "keybinds", &s(&["uq.close"])),
        Some(json!(false))
    );
    assert_eq!(
        read_leaf(&st, "keybinds", &s(&["uq.float"])),
        Some(json!(true))
    );
    assert_eq!(
        read_leaf(&st, "keybinds", &s(&["uq.spotlight"])),
        Some(json!(true))
    );
    assert_eq!(
        read_leaf(&st, "keybinds", std::slice::from_ref(&id)),
        Some(json!(true))
    );
    assert_eq!(read_leaf(&st, "keybinds", &s(&["k-ffff"])), None);
    assert_eq!(read_leaf(&st, "keybinds", &s(&["uq.close", "x"])), None);
    // Turning one of uQ's off writes an override; back on drops it, keeping other changes.
    write_leaves(
        &mut st,
        "keybinds",
        &[
            (s(&["uq.spotlight"]), Some(json!(false))),
            (s(&["uq.close"]), Some(json!(true))),
            (s(&["uq.float"]), Some(json!(false))),
            (vec![id.clone()], Some(json!(false))),
        ],
    )
    .unwrap();
    assert_eq!(
        st.keybinds.overrides,
        vec![
            Override {
                id: "uq.float".into(),
                combo: Some("SUPER + SHIFT + F".into()),
                enabled: Some(false)
            },
            Override {
                id: "uq.spotlight".into(),
                combo: None,
                enabled: Some(false)
            },
        ]
    );
    assert!(!st.keybinds.custom[0].enabled);
    // Only the switches changed.
    assert_eq!(st.keybinds.custom[0].combo, "SUPER + F5");
    // Refused leaves leave everything as it was, including the good one before them.
    let now = st.clone();
    for leaf in [
        (s(&["uq.close"]), Some(json!("on"))),
        (s(&["uq.close"]), None),
        (s(&["k-ffff"]), Some(json!(true))),
        (s(&["uq.close", "x"]), Some(json!(true))),
        (s(&["uq.lock-recovery"]), Some(json!(false))),
    ] {
        let both = [(s(&["uq.float"]), Some(json!(true))), leaf.clone()];
        assert!(
            write_leaves(&mut st, "keybinds", &both).is_err(),
            "{leaf:?}"
        );
        assert_eq!(st, now, "{leaf:?}");
    }
    assert!(set_enabled(&mut st, "uq.close", false));
    assert!(!set_enabled(&mut st, "k-ffff", true));
    assert_eq!(enabled_of(&st, "uq.close"), Some(false));
    assert_eq!(enabled_of(&st, "k-ffff"), None);
}

#[test]
fn a_switch_that_would_share_keys_is_refused_with_both_names() {
    let mut st = state();
    // Spotlight off, then one of your own takes Super + Space.
    write_leaves(
        &mut st,
        "keybinds",
        &[(s(&["uq.spotlight"]), Some(json!(false)))],
    )
    .unwrap();
    let mut section = st.section("keybinds").unwrap();
    section["custom"]
        .as_array_mut()
        .unwrap()
        .push(custom("Mine", "SUPER + Space"));
    st.set_section("keybinds", section).unwrap();
    let before = st.clone();
    let e = write_leaves(
        &mut st,
        "keybinds",
        &[(s(&["uq.spotlight"]), Some(json!(true)))],
    )
    .unwrap_err();
    let SettingError::Invalid { key, reason } = e else {
        panic!("{e:?}")
    };
    assert_eq!(key, "keybinds.uq.spotlight");
    assert!(
        reason.contains("Mine") && reason.contains("Open Spotlight"),
        "{reason}"
    );
    assert_eq!(st, before);
}

#[test]
fn bind_keys_and_labels() {
    let st = state();
    let keys = bind_keys(&st.keybinds);
    assert_eq!(keys.len(), crate::keybinds::defaults::defaults().len() + 1);
    assert!(
        keys.iter()
            .all(|k| k.domain == "keybinds" && k.path.len() == 1)
    );
    let get = |id: &str| {
        keys.iter()
            .find(|k| k.path[0] == id)
            .map(|k| k.value.clone())
    };
    assert_eq!(get("uq.close"), Some(json!(false)));
    assert_eq!(get("uq.spotlight"), Some(json!(true)));
    assert_eq!(get(&custom_id(&st)), Some(json!(true)));
    // uQ's in table order, then yours.
    assert_eq!(keys[0].path[0], crate::keybinds::defaults::defaults()[0].id);
    assert_eq!(keys.last().unwrap().path[0], custom_id(&st));
    assert_eq!(bind_label(&st.keybinds, "uq.close"), "Close the window");
    assert_eq!(
        bind_label(&st.keybinds, &custom_id(&st)),
        "Shortcut \u{201c}Terminal too\u{201d}"
    );
    assert_eq!(
        bind_label(&st.keybinds, "k-ffff"),
        "A shortcut that isn't on this computer"
    );
    // Every key a profile can hold has a plain label; a key's own label is "A shortcut".
    assert_eq!(
        crate::profile::key_label("keybinds", &s(&["uq.close"])),
        "A shortcut"
    );
    // Names from files other programs can edit can't carry a control character.
    let mut st = st;
    st.keybinds.custom[0].name = "a\u{1b}b".into();
    assert!(!bind_label(&st.keybinds, &custom_id(&st)).contains('\u{1b}'));
}

#[test]
fn missing_binds_are_counted_and_worded() {
    let st = state();
    let p = Profile::parse(&with(
        "[keybinds]\n\"uq.close\" = true\n\"k-ffff\" = false\n\"k-fffe\" = true\n",
    ))
    .unwrap();
    assert_eq!(missing_binds(&st, &p), 2);
    assert_eq!(
        missing_binds_notice(1),
        "1 shortcut in this profile isn't on this computer, so it was left out."
    );
    assert_eq!(
        missing_binds_notice(2),
        "2 shortcuts in this profile aren't on this computer, so they were left out."
    );
}

#[test]
fn bind_toggles_classifies() {
    let before = json!({"overrides": [{"id": "uq.close", "enabled": false},
                                       {"id": "uq.float", "combo": "SUPER + SHIFT + F"}],
        "custom": [{"id": "k-0001", "name": "A", "combo": "SUPER + F5", "enabled": true},
                   {"id": "k-0002", "name": "B", "combo": "SUPER + F6"},
                   {"id": "k-0003", "name": "C", "combo": "SUPER + F7"}]});
    let after = json!({"overrides": [{"id": "uq.float", "combo": "SUPER + SHIFT + G"},
                                      {"id": "uq.pin", "enabled": false}],
        "custom": [{"id": "k-0001", "name": "A", "combo": "SUPER + F5", "enabled": false},
                   {"id": "k-0002", "name": "B2", "combo": "SUPER + F6"},
                   {"id": "k-0004", "name": "D", "combo": "SUPER + F8"}]});
    let (toggles, other) = bind_toggles(&before, &after);
    // uq.close back on, uq.pin off, A off.
    assert_eq!(toggles, ["uq.close", "uq.pin", "k-0001"]);
    // New keys for uq.float, B edited, D added, C removed.
    assert_eq!(other, ["uq.float", "k-0002", "k-0004", "k-0003"]);
    // A reorder of your own is one tag; nothing else changed.
    let moved = json!({"custom": [{"id": "k-0002", "name": "B", "combo": "SUPER + F6"},
                                   {"id": "k-0001", "name": "A", "combo": "SUPER + F5"}]});
    let first = json!({"custom": [{"id": "k-0001", "name": "A", "combo": "SUPER + F5"},
                                   {"id": "k-0002", "name": "B", "combo": "SUPER + F6"}]});
    assert_eq!(
        bind_toggles(&first, &moved),
        (vec![], vec!["order:custom".to_string()])
    );
    assert_eq!(bind_toggles(&first, &first), (vec![], vec![]));
    // A turned-off shortcut that also got new keys is still a toggle, and the keys are other.
    let b = json!({"overrides": [{"id": "uq.close", "enabled": false}]});
    let a = json!({"overrides": [{"id": "uq.close", "combo": "SUPER + Z"}]});
    assert_eq!(
        bind_toggles(&b, &a),
        (vec!["uq.close".to_string()], vec!["uq.close".to_string()])
    );
}

#[test]
fn mark_binds_only_overrides_uq_and_default() {
    let st = state();
    let id = custom_id(&st);
    let mut rows = json!({
        "binds.uq.close": {"source": "uq", "detail": null},
        "binds.uq.spotlight": {"source": "default", "detail": null},
        "binds.uq.float": {"source": "user_lua", "detail": {"line": 3}},
        "binds.uq.pin": {"source": "runtime", "detail": null},
        "binds.uq.fullscreen": {"source": "error", "detail": {"message": "x"}},
    });
    rows[format!("binds.{id}")] = json!({"source": "uq", "detail": null});
    let memo = |id: &str, set: bool| KeyMemo {
        domain: "keybinds".into(),
        path: vec![id.into()],
        before: Some(json!(!set)),
        set: json!(set),
    };
    let active = ActiveProfile {
        id: "quiet".into(),
        name: "Quiet".into(),
        since: 0,
        keys: vec![
            memo("uq.close", false),
            memo("uq.spotlight", true),
            memo("uq.float", true),
            memo("uq.pin", true),
            memo("uq.fullscreen", true),
            memo(&id, false),
        ],
        power: None,
    };
    mark_binds(&mut rows, &st.keybinds, &active);
    assert_eq!(rows["binds.uq.close"]["source"], "profile");
    assert_eq!(rows["binds.uq.close"]["detail"], json!({"name": "Quiet"}));
    assert_eq!(rows["binds.uq.spotlight"]["source"], "profile");
    // user.lua, runtime and error say what they say.
    assert_eq!(rows["binds.uq.float"]["source"], "user_lua");
    assert_eq!(rows["binds.uq.pin"]["source"], "runtime");
    assert_eq!(rows["binds.uq.fullscreen"]["source"], "error");
    // The profile set yours off, but it's on now (changed by hand): not the profile's.
    assert_eq!(rows[format!("binds.{id}")]["source"], "uq");
    // And a row the profile never touched stays put.
    let none = ActiveProfile {
        keys: vec![],
        ..active
    };
    let mut again = json!({"binds.uq.close": {"source": "uq", "detail": null}});
    mark_binds(&mut again, &st.keybinds, &none);
    assert_eq!(again["binds.uq.close"]["source"], "uq");
}
