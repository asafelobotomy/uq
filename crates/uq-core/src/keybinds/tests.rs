use super::*;
use crate::state::State;
use serde_json::json;

fn write(st: &mut State, v: Value) -> Result<(), SettingError> {
    st.set_section("keybinds", v)
}

fn err(v: Value) -> String {
    let mut st = State::default();
    write(&mut st, v).unwrap_err().to_string()
}

fn custom(name: &str, combo: &str, action: Value) -> Value {
    json!({"name": name, "combo": combo, "action": action})
}

fn shell(name: &str) -> Value {
    json!({"kind": "shell", "name": name})
}

fn term() -> Value {
    json!({"kind": "uq", "name": "terminal"})
}

#[test]
fn spec_shape_parses_and_round_trips() {
    let v = json!({
        "overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"},
                      {"id": "uq.close", "enabled": false}],
        "custom": [{"id": "k-3f9c", "name": "Screenshot", "combo": "SUPER + SHIFT + P",
                    "enabled": true, "locked": false, "repeating": false,
                    "action": {"kind": "command", "command": "grim -g \"$(slurp)\" ~/shot.png"}}]
    });
    let mut st = State::default();
    write(&mut st, v.clone()).unwrap();
    assert_eq!(st.section("keybinds").unwrap(), v);
    let toml = toml::to_string(&st).unwrap();
    assert!(toml.contains("[[keybinds.overrides]]") && toml.contains("[[keybinds.custom]]"));
    let back = State::parse_strict(&toml).unwrap();
    assert_eq!(back.keybinds, st.keybinds);
}

#[test]
fn ids_assigned_and_kept() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"custom": [custom("A", "SUPER + F1", term()), custom("B", "SUPER + F2", term())]}),
    )
    .unwrap();
    let ids = st
        .keybinds
        .ids()
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    assert!(ids.iter().all(|i| valid_custom_id(i)) && ids[0] != ids[1]);
    for bad in ["x-1234", "k-12", "k-GGGG", "k-123456789"] {
        let mut b = custom("A", "SUPER + F1", term());
        b["id"] = json!(bad);
        assert!(
            err(json!({"custom": [b]})).contains("looks like k-"),
            "{bad}"
        );
    }
    let (mut a, mut b) = (
        custom("A", "SUPER + F1", term()),
        custom("B", "SUPER + F2", term()),
    );
    a["id"] = json!("k-0001");
    b["id"] = json!("k-0001");
    assert!(err(json!({"custom": [a, b]})).contains("another shortcut"));
}

#[test]
fn resending_the_section_is_accepted() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.close", "combo": "SUPER + X"}],
               "custom": [custom("A", "SUPER + F1", term())]}),
    )
    .unwrap();
    let again = st.section("keybinds").unwrap();
    let before = st.keybinds.clone();
    write(&mut st, again).unwrap();
    assert_eq!(st.keybinds, before);
}

#[test]
fn override_equal_to_default_is_dropped() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.spotlight", "combo": "WIN + space"},
                             {"id": "uq.close", "enabled": true}]}),
    )
    .unwrap();
    assert!(st.keybinds.overrides.is_empty());
    assert_eq!(
        st.section("keybinds").unwrap(),
        json!({"overrides": [], "custom": []})
    );
}

#[test]
fn override_kind_must_match() {
    let mut st = State::default();
    assert!(
        write(
            &mut st,
            json!({"overrides": [{"id": "uq.window.drag", "combo": "SUPER + Q"}]})
        )
        .is_err()
    );
    assert!(
        write(
            &mut st,
            json!({"overrides": [{"id": "uq.close", "combo": "SUPER + mouse:272"}]})
        )
        .is_err()
    );
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.window.drag", "combo": "SUPER + mouse:274"},
                             {"id": "uq.ws.next", "combo": "SUPER + mouse_left"}]}),
    )
    .unwrap();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + D"}]}),
    )
    .unwrap();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + SUPER_R"}]}),
    )
    .unwrap();
    assert_eq!(
        st.keybinds.overrides[0].combo.as_deref(),
        Some("SUPER + SUPER_R")
    );
}

#[test]
fn unknown_default_id_refused_on_write_dropped_on_load() {
    assert!(
        err(json!({"overrides": [{"id": "uq.nope", "enabled": false}]}))
            .contains("isn't one of uQ's shortcuts")
    );
    let src = r#"
[[keybinds.overrides]]
id = "uq.nope"
enabled = false
[[keybinds.overrides]]
id = "uq.close"
enabled = false
"#;
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.keybinds.overrides.len(), 1);
    assert_eq!(st.keybinds.overrides[0].id, "uq.close");
}

#[test]
fn lock_recovery_stays_on() {
    assert!(
        err(json!({"overrides": [{"id": "uq.lock-recovery", "enabled": false}]}))
            .contains("stays on")
    );
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.lock-recovery", "combo": "SUPER + CTRL + K"}]}),
    )
    .unwrap();
    let st =
        State::parse_strict("[[keybinds.overrides]]\nid = \"uq.lock-recovery\"\nenabled = false\n")
            .unwrap();
    assert!(st.keybinds.overrides.is_empty());
}

#[test]
fn conflicts_refused_with_both_names() {
    let e = err(json!({"custom": [custom("Shot", "SUPER + Space", term())]}));
    assert!(e.contains("Shot") && e.contains("Open Spotlight"), "{e}");
    assert!(e.contains("keybinds.custom[0].combo"), "{e}");
    let e = err(json!({"custom": [custom("Shot", "WIN + space", term())]}));
    assert!(e.contains("Open Spotlight"), "{e}");
    let e = err(json!({"custom": [custom("One", "CTRL + ALT + T", term()),
                                  custom("Two", "ALT + CONTROL + t", term())]}));
    assert!(
        e.contains("One") && e.contains("Two") && e.contains("custom[1]"),
        "{e}"
    );
    let e = err(json!({"overrides": [{"id": "uq.close", "combo": "SUPER + V"}]}));
    assert!(
        e.contains("Close the window") && e.contains("Float or tile the window"),
        "{e}"
    );
    assert!(e.contains("Super + V"), "{e}");
    // An override equal to its default is dropped, so the error key still names the entry as
    // written: here [1], not [0].
    let e = err(
        json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + Space"},
                                     {"id": "uq.close", "combo": "SUPER + V"}]}),
    );
    assert!(e.contains("keybinds.overrides[1].combo"), "{e}");
}

#[test]
fn unchecked_layout_and_media_never_reach_the_shell() {
    let a = Action::Uq {
        name: "layout".into(),
        layout: Some("x; rm -rf ~".into()),
        media: None,
    };
    assert!(a.lua().contains("current dwindle\")"), "{}", a.lua());
    let a = Action::Uq {
        name: "media".into(),
        layout: None,
        media: Some("$(id)".into()),
    };
    assert!(a.lua().contains("playerctl play-pause\")"), "{}", a.lua());
}

#[test]
fn conflicts_ignore_disabled_and_unbound() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.spotlight", "enabled": false}],
               "custom": [custom("Mine", "SUPER + Space", term())]}),
    )
    .unwrap();
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.profile.next", "combo": "SUPER + ALT + P"}]}),
    )
    .unwrap();
    let e = err(json!({"overrides": [{"id": "uq.profile.next", "combo": "SUPER + ALT + D"}]}));
    assert!(
        e.contains("Switch to the next profile") && e.contains("dwindle"),
        "{e}"
    );
    // a disabled custom bind doesn't conflict
    let mut b = custom("Mine", "SUPER + Space", term());
    b["enabled"] = json!(false);
    write(&mut State::default(), json!({"custom": [b]})).unwrap();
}

#[test]
fn find_is_generic() {
    let items = [
        ((3u8, "left"), "g-1", "Three fingers left"),
        ((4u8, "left"), "g-2", "Four fingers left"),
        ((3u8, "left"), "g-3", "Back"),
    ];
    let c = conflict::find(&items);
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].first.as_str(), c[0].second.as_str()), ("g-1", "g-3"));
    assert_eq!(c[0].labels.1, "Back");
}

#[test]
fn lock_and_repeat_limits() {
    let mut b = custom("Vol", "SUPER + F1", shell("vol-up"));
    b["locked"] = json!(true);
    b["repeating"] = json!(true);
    write(&mut State::default(), json!({"custom": [b]})).unwrap();
    for action in [
        shell("spotlight"),
        term(),
        json!({"kind": "launch", "app": "firefox"}),
        json!({"kind": "command", "command": "ls"}),
    ] {
        let mut b = custom("X", "SUPER + F1", action);
        b["locked"] = json!(true);
        assert!(err(json!({"custom": [b]})).contains("over the lock screen"));
    }
    for action in [
        json!({"kind": "launch", "app": "firefox"}),
        json!({"kind": "command", "command": "ls"}),
        term(),
        json!({"kind": "uq", "name": "settings"}),
        json!({"kind": "uq", "name": "layout", "layout": "master"}),
    ] {
        let mut b = custom("X", "SUPER + F1", action);
        b["repeating"] = json!(true);
        assert!(err(json!({"custom": [b]})).contains("over and over"));
    }
    let mut media = custom(
        "Next",
        "XF86AudioStop",
        json!({"kind": "uq", "name": "media", "media": "next"}),
    );
    media["locked"] = json!(true);
    write(&mut State::default(), json!({"custom": [media]})).unwrap();
}

#[test]
fn bare_keys() {
    assert!(err(json!({"custom": [custom("X", "Q", term())]})).contains("Add Super, Ctrl or Alt"));
    assert!(
        err(json!({"custom": [custom("X", "SHIFT + Q", term())]}))
            .contains("Add Super, Ctrl or Alt")
    );
    assert!(err(json!({"overrides": [{"id": "uq.close", "combo": "Q"}]})).contains("Add Super"));
    write(
        &mut State::default(),
        json!({"custom": [custom("X", "XF86AudioStop", term())]}),
    )
    .unwrap();
    write(
        &mut State::default(),
        json!({"custom": [custom("X", "Print", term())]}),
    )
    .unwrap();
    write(
        &mut State::default(),
        json!({"custom": [custom("X", "F13", term())]}),
    )
    .unwrap();
}

#[test]
fn limits() {
    let many: Vec<Value> = (0..101)
        .map(|i| {
            custom(
                &format!("n{i}"),
                &format!("SUPER + F{}", 1 + i % 24),
                term(),
            )
        })
        .collect();
    assert!(err(json!({"custom": many})).contains("at most 100"));
    let name = "a".repeat(101);
    assert!(err(json!({"custom": [custom(&name, "SUPER + F1", term())]})).contains("name"));
    assert!(err(json!({"custom": [custom("", "SUPER + F1", term())]})).contains("name"));
    assert!(err(json!({"custom": [custom("a\nb", "SUPER + F1", term())]})).contains("name"));
    let c = |s: &str| custom("X", "SUPER + F1", json!({"kind": "command", "command": s}));
    assert!(err(json!({"custom": [c("a\nb")]})).contains("command"));
    assert!(err(json!({"custom": [c(&"a".repeat(1025))]})).contains("1024"));
    write(
        &mut State::default(),
        json!({"custom": [c(&"a".repeat(1024))]}),
    )
    .unwrap();
    // 100 distinct combos are fine.
    let ok: Vec<Value> = (0..100)
        .map(|i| {
            let m = [
                "SUPER",
                "SUPER + SHIFT",
                "SUPER + CTRL",
                "SUPER + ALT",
                "SUPER + CTRL + ALT",
            ][i / 24];
            custom(&format!("n{i}"), &format!("{m} + F{}", 1 + i % 24), term())
        })
        .collect();
    write(&mut State::default(), json!({"custom": ok})).unwrap();
}

#[test]
fn refuses_hostile_binds() {
    let base = || custom("X", "SUPER + F1", term());
    let mut b = base();
    b["exec"] = json!("rm -rf ~");
    assert!(err(json!({"custom": [b]})).contains("unknown field"));
    let mut b = base();
    b["action"]["shell_cmd"] = json!("x");
    assert!(err(json!({"custom": [b]})).contains("shell_cmd"));
    assert!(err(json!({"overrides": [{"id": "uq.close", "action": {"kind": "command", "command": "x"}}]}))
        .contains("action"));
    assert!(err(json!({"extra": 1})).contains("extra"));
    for v in [
        json!({"custom": "x"}),
        json!({"custom": [1]}),
        json!({"custom": [{"name": 3, "combo": "SUPER + F1", "action": term()}]}),
        json!({"overrides": [{"id": 3}]}),
        json!({"custom": [custom("X", "code:24", term())]}),
        json!({"custom": [custom("X", "SUPER + mouse:272", term())]}),
        json!([]),
    ] {
        assert!(write(&mut State::default(), v.clone()).is_err(), "{v}");
    }
    // A hostile command is accepted as text, but never reaches an error message.
    let hostile = "x'; $(id) \") hl.exec_cmd(\"x";
    let e = err(
        json!({"custom": [custom("Y", "SUPER + Space", json!({"kind": "command", "command": hostile}))]}),
    );
    assert!(!e.contains("$(id)"), "{e}");
}

#[test]
fn lenient_load_drops_only_the_bad_entry() {
    let src = r#"
theme = "rosewood"
[settings]
pinned = ["a.desktop"]

[[keybinds.custom]]
id = "k-0001"
name = "Bad"
combo = "SUPER + F1"
exec = "nope"
[keybinds.custom.action]
kind = "uq"
name = "terminal"

[[keybinds.custom]]
id = "k-0002"
name = "Conflicting"
combo = "SUPER + Space"
[keybinds.custom.action]
kind = "uq"
name = "terminal"

[[keybinds.custom]]
id = "k-0003"
name = "Good"
combo = "SUPER + F3"
[keybinds.custom.action]
kind = "uq"
name = "terminal"
"#;
    let st = State::parse_strict(src).unwrap();
    assert_eq!(st.keybinds.custom.len(), 1);
    assert_eq!(st.keybinds.custom[0].name, "Good");
    assert_eq!(st.theme, "rosewood");
}

#[test]
fn effective_lists_defaults_then_custom() {
    let mut st = State::default();
    write(
        &mut st,
        json!({"overrides": [{"id": "uq.spotlight", "combo": "SUPER + K"}, {"id": "uq.close", "enabled": false}],
               "custom": [custom("Mine", "SUPER + F1", term())]}),
    )
    .unwrap();
    let eff = st.keybinds.effective();
    assert_eq!(eff.len(), 57);
    let by = |id: &str| eff.iter().find(|e| e.id == id).unwrap();
    assert_eq!(
        combo::canonical(by("uq.spotlight").combo.unwrap()),
        "SUPER + K"
    );
    assert!(!by("uq.close").enabled && by("uq.terminal").enabled);
    assert!(by("uq.appmenu").release && by("uq.window.drag").mouse);
    assert!(!eff.last().unwrap().is_default);
}
