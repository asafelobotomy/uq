use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::input::InputState;
use crate::pages::runner::run;
use crate::xkb_lists::XkbLists;
use uq_core::state::State;

async fn rig() -> (Rig, InputState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = InputState::with_xkb(r.client.clone(), XkbLists::default());
    (r, page)
}

fn save(index: i32, gesture: Value) -> Op {
    Op::SaveGesture { index, gesture }
}

fn swipe(field: &str, value: Value) -> Op {
    Op::SetSwipe {
        field: field.into(),
        value,
    }
}

fn close_left() -> Value {
    json!({"fingers": 3, "direction": "left", "action": "close"})
}

fn stored(r: &Rig) -> Option<Vec<Gesture>> {
    r.state.lock().unwrap().settings.input.gestures.clone()
}

#[tokio::test]
async fn defaults_listed_when_null() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["gestures_default"], true);
    let rows = out.state["gestures"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["label"], "3 fingers, swipe sideways");
    assert_eq!(rows[0]["action_label"], "Switch workspace");
    assert_eq!(rows[0]["enabled"], true);
    assert_eq!(rows[0]["gesture"]["action"], "workspace");
    assert_eq!(rows[1]["label"], "4 fingers, swipe up");
    assert_eq!(rows[1]["action_label"], "Open Spotlight");
    assert!(out.state["input"].get("gestures").is_none());
    assert_eq!(stored(&r), None);
    assert_eq!(out.state["clashes"], json!({}));
    assert_eq!(out.state["swipe"], json!({}));
    assert_eq!(out.state["user_lua"], Value::Null);
}

#[tokio::test]
async fn turning_a_default_off_writes_the_list() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::ToggleGesture {
            index: 0,
            enabled: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["gestures_default"], false);
    let list = stored(&r).expect("the list is written");
    assert_eq!(list.len(), 2);
    assert!(!list[0].enabled);
    assert!(list[1].enabled, "the other default stays on");
    assert_eq!(out.state["gestures"][0]["enabled"], false);
}

#[tokio::test]
async fn keyboard_write_keeps_default_gestures() {
    let (r, page) = rig().await;
    for op in [
        Op::SetKeyboard {
            field: "repeat_rate".into(),
            value: json!(40),
        },
        Op::SetTouchpad {
            field: "tap_to_click".into(),
            value: json!(true),
        },
        swipe("distance", json!(400)),
    ] {
        let out = run(&page, op).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["gestures_default"], true);
        assert_eq!(stored(&r), None, "only gesture edits write the list");
    }
    // And a list that was written stays a list through a keyboard write.
    run(
        &page,
        Op::ToggleGesture {
            index: 1,
            enabled: false,
        },
    )
    .await;
    run(
        &page,
        Op::SetKeyboard {
            field: "repeat_rate".into(),
            value: json!(30),
        },
    )
    .await;
    assert_eq!(stored(&r).map(|l| l.len()), Some(2));
}

#[tokio::test]
async fn add_edit_remove_toggle() {
    let (r, page) = rig().await;
    let four_left = json!({"fingers": 4, "direction": "left", "action": "close"});
    let out = run(&page, save(-1, four_left)).await;
    assert_eq!(out.error, None, "{out:?}");
    assert_eq!(out.state["gestures_default"], false);
    assert_eq!(out.state["gestures"].as_array().unwrap().len(), 3);
    assert_eq!(out.state["gestures"][2]["label"], "4 fingers, swipe left");
    assert_eq!(out.state["gestures"][2]["action_label"], "Close the window");

    // Change it: a modifier, a different action and the scratch workspace's name.
    let edited = json!({
        "fingers": 4, "direction": "right", "action": "special",
        "modifier": "SUPER", "workspace": "notes",
    });
    let out = run(&page, save(2, edited)).await;
    assert_eq!(out.error, None, "{out:?}");
    assert_eq!(
        out.state["gestures"][2]["label"],
        "SUPER + 4 fingers, swipe right"
    );
    assert_eq!(out.state["gestures"][2]["gesture"]["workspace"], "notes");
    assert_eq!(stored(&r).unwrap()[2].workspace.as_deref(), Some("notes"));

    let out = run(
        &page,
        Op::ToggleGesture {
            index: 2,
            enabled: false,
        },
    )
    .await;
    assert_eq!(out.state["gestures"][2]["enabled"], false);
    let out = run(
        &page,
        Op::ToggleGesture {
            index: 2,
            enabled: true,
        },
    )
    .await;
    assert_eq!(out.state["gestures"][2]["enabled"], true);

    let out = run(&page, Op::RemoveGesture { index: 2 }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["gestures"].as_array().unwrap().len(), 2);

    // An index that isn't there any more is refused plainly.
    let out = run(&page, Op::RemoveGesture { index: 7 }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("That gesture isn't there any more.")
    );
    let out = run(&page, save(9, close_left())).await;
    assert_eq!(
        out.error.as_deref(),
        Some("That gesture isn't there any more.")
    );
}

#[tokio::test]
async fn bad_gestures_read_plainly() {
    let (_r, page) = rig().await;
    let out = run(
        &page,
        save(
            -1,
            json!({"fingers": 6, "direction": "left", "action": "close"}),
        ),
    )
    .await;
    let text = out.error.unwrap();
    assert!(!text.starts_with("invalid value for"), "{text}");
    assert!(text.contains("3, 4 or 5"), "{text}");
    let out = run(
        &page,
        save(
            -1,
            json!({"fingers": 4, "direction": "pinch_in", "action": "move"}),
        ),
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Moving the window works only with a swipe, not a pinch")
    );
}

#[tokio::test]
async fn draft_clash_warns_before_save() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    let list = effective(&json!({}));
    let draft = Gesture::from_value("g", &close_left()).unwrap();
    let want = "“Close the window” and “Switch workspace” both use 3 fingers, swipe left.";
    assert_eq!(draft_clash(&list, -1, &draft).as_deref(), Some(want));
    // The page asks through its last read, from the draft as JSON.
    assert_eq!(page.draft_clash(-1, &close_left()), want);
    // Editing a gesture into a clash warns too; editing it into nothing does not.
    assert_eq!(draft_clash(&list, 1, &draft).as_deref(), Some(want));
    let alone = Gesture::from_value(
        "g",
        &json!({"fingers": 5, "direction": "up", "action": "float"}),
    )
    .unwrap();
    assert_eq!(draft_clash(&list, -1, &alone), None);
    // A draft that is off clashes with nothing; a gesture's own old self isn't a clash.
    let mut off = draft.clone();
    off.enabled = false;
    assert_eq!(draft_clash(&list, -1, &off), None);
    assert_eq!(draft_clash(&list, 0, &list[0].clone()), None);
    assert_eq!(page.draft_clash(-1, &json!({"fingers": 2})), "");

    // Saving it anyway is refused with the same reason, once, and the list is unchanged.
    let out2 = run(&page, save(-1, close_left())).await;
    assert_eq!(out2.error.as_deref(), Some(want));
    assert_eq!(out2.state["gestures"], out.state["gestures"]);
    assert_eq!(out2.state["gestures_default"], true);
    assert_eq!(stored(&r), None);
}

#[tokio::test]
async fn refused_by_uqd_reads_the_same() {
    // The page's own check is a convenience: uqd's refusal of the same write loses its key too.
    let (r, page) = rig().await;
    run(&page, Op::Refresh).await;
    let list = json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
        {"fingers": 3, "direction": "left", "action": "close", "enabled": true},
    ]);
    let proxy = uq_core::client::DesktopProxy::new(&r.client).await.unwrap();
    let mut section: Value =
        serde_json::from_str(&proxy.get_section("input").await.unwrap()).unwrap();
    section["gestures"] = list;
    let e = proxy
        .set_section("input", &section.to_string(), "settings")
        .await
        .unwrap_err();
    let text = plain_error(e).to_string();
    assert!(text.contains("both use 3 fingers, swipe left"), "{text}");
    assert!(!text.contains("invalid value for"), "{text}");
}

#[tokio::test]
async fn directions_offered_per_action() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    let choices = &out.state["choices"];
    assert_eq!(choices["fingers"], json!([3, 4, 5]));
    let action = |id: &str| {
        choices["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == id)
            .unwrap_or_else(|| panic!("no action {id}"))
            .clone()
    };
    assert_eq!(
        action("workspace")["directions"],
        json!(["horizontal", "vertical"])
    );
    assert_eq!(action("workspace")["label"], "Switch workspace");
    assert_eq!(action("move")["directions"].as_array().unwrap().len(), 6);
    assert_eq!(action("close")["directions"].as_array().unwrap().len(), 8);
    assert!(
        !action("move")["directions"]
            .as_array()
            .unwrap()
            .contains(&json!("pinch_in"))
    );
    assert_eq!(choices["actions"].as_array().unwrap().len(), 9);
    let dirs = choices["directions"].as_array().unwrap();
    assert_eq!(dirs.len(), 8);
    assert_eq!(
        dirs[0],
        json!({"id": "horizontal", "label": "Swipe sideways"})
    );
    assert_eq!(dirs[6], json!({"id": "pinch_in", "label": "Pinch in"}));
    let mods: Vec<&str> = choices["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(mods, ["", "SUPER", "ALT", "CTRL", "SHIFT"]);
    // Every offered id is one the core reads back.
    for m in &mods[1..] {
        let g = json!({"fingers": 4, "direction": "up", "action": "close", "modifier": m});
        assert!(Gesture::from_value("g", &g).is_ok(), "{m}");
    }
}

#[tokio::test]
async fn reset_restores_defaults() {
    let (r, page) = rig().await;
    run(
        &page,
        save(
            -1,
            json!({"fingers": 4, "direction": "left", "action": "close"}),
        ),
    )
    .await;
    run(
        &page,
        Op::ToggleGesture {
            index: 0,
            enabled: false,
        },
    )
    .await;
    assert!(stored(&r).is_some());
    let out = run(&page, Op::ResetGestures).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["gestures_default"], true);
    assert_eq!(out.state["gestures"].as_array().unwrap().len(), 2);
    assert_eq!(out.state["gestures"][0]["enabled"], true);
    assert_eq!(stored(&r), None);
}

#[tokio::test]
async fn swipe_options_round_trip() {
    let (r, page) = rig().await;
    let writes = [
        ("distance", json!(400)),
        ("invert", json!(false)),
        ("create_new", json!(false)),
        ("cancel_ratio", json!(0.3)),
    ];
    assert_eq!(
        writes.len(),
        hypr::SWIPE_OPTIONS.len(),
        "an option has no round trip"
    );
    for (field, value) in &writes {
        let out = run(&page, swipe(field, value.clone())).await;
        assert_eq!(out.error, None, "{field}");
    }
    let fresh = InputState::with_xkb(r.client.clone(), XkbLists::default());
    let state = run(&fresh, Op::Refresh).await.state;
    for (field, value) in &writes {
        assert_eq!(&state["swipe"][field], value, "{field} reads back");
    }
    assert_eq!(
        state["gestures_default"], true,
        "the gestures weren't touched"
    );
    // `null` clears one back to Hyprland's own.
    let out = run(&page, swipe("distance", Value::Null)).await;
    assert!(out.state["swipe"].get("distance").is_none());
    assert_eq!(out.state["swipe"]["invert"], false);
    let out = run(&page, swipe("nope", json!(1))).await;
    assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
}

#[tokio::test]
async fn swipe_out_of_range_refused_plainly() {
    let (r, page) = rig().await;
    let out = run(&page, swipe("distance", json!(5000))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Swipe distance must be between 0 and 2000")
    );
    assert!(out.state["swipe"].get("distance").is_none());
    let out = run(&page, swipe("cancel_ratio", json!(1.5))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("How far to swipe must be between 0 and 1")
    );
    assert!(r.state.lock().unwrap().settings.input.swipe.is_default());
}

#[tokio::test]
async fn user_lua_gestures_reported() {
    let (r, page) = rig().await;
    r.explain.set(
        "input.gestures",
        json!({"source": "user_lua", "detail": {"line": 12, "path": "/home/me/.config/uq/user.lua"}}),
    );
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["user_lua"],
        json!({"line": 12, "path": "/home/me/.config/uq/user.lua"})
    );
    // The swipe rows' sources come with the snapshot, for the page's source lines.
    assert_eq!(
        out.state["sources"]["gesture-swipe-distance"]["source"],
        "default"
    );
    r.explain.fail();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["user_lua"], Value::Null, "no Explain, no line");
}

#[tokio::test]
async fn stored_list_that_clashes_is_reported() {
    // A list from an older or hand-edited file: uqd repairs it on load, but the page still
    // checks what it is given.
    let (r, page) = rig().await;
    let a = Gesture::from_value(
        "g",
        &json!({"fingers": 3, "direction": "horizontal", "action": "workspace"}),
    )
    .unwrap();
    let b = Gesture::from_value("g", &close_left()).unwrap();
    r.state.lock().unwrap().settings.input.gestures = Some(vec![a, b]);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["clashes"]["1"],
        "“Close the window” and “Switch workspace” both use 3 fingers, swipe left."
    );
    assert_eq!(out.state["clashes"].as_object().unwrap().len(), 1);
}
