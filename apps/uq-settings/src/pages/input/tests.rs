use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::settings::AccelProfile;
use uq_core::state::State;

const FIXTURE: &str = include_str!("../../../tests/fixtures/evdev.lst");

async fn rig() -> (Rig, InputState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = InputState::with_xkb(r.client.clone(), xkb_lists::parse(FIXTURE));
    (r, page)
}

fn kb(field: &str, value: Value) -> Op {
    Op::SetKeyboard {
        field: field.into(),
        value,
    }
}

fn mouse(field: &str, value: Value) -> Op {
    Op::SetMouse {
        field: field.into(),
        value,
    }
}

fn touchpad(field: &str, value: Value) -> Op {
    Op::SetTouchpad {
        field: field.into(),
        value,
    }
}

fn device(name: &str, field: &str, value: Value) -> Op {
    Op::SetDevice {
        name: name.into(),
        field: field.into(),
        value,
    }
}

#[tokio::test]
async fn snapshot_has_section_devices_and_lists() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["input"]["mouse"]["sensitivity"], 0.0);
    assert_eq!(out.state["devices"]["pointers"][0]["name"], "elan-touchpad");
    assert_eq!(
        out.state["xkb"]["layouts"][1],
        json!(["us", "English (US)"])
    );
}

#[tokio::test]
async fn layouts_add_reorder_remove() {
    let (r, page) = rig().await;
    let out = run(&page, kb("layouts", json!([{"layout": "us"}]))).await;
    assert_eq!(out.error, None);
    let out = run(
        &page,
        kb(
            "layouts",
            json!([{"layout": "us"}, {"layout": "de", "variant": "nodeadkeys"}]),
        ),
    )
    .await;
    assert_eq!(
        out.state["input"]["keyboard"]["layouts"][1]["variant"],
        "nodeadkeys"
    );
    let out = run(
        &page,
        kb(
            "layouts",
            json!([{"layout": "de", "variant": "nodeadkeys"}, {"layout": "us"}]),
        ),
    )
    .await;
    assert_eq!(out.state["input"]["keyboard"]["layouts"][0]["layout"], "de");
    let out = run(&page, kb("layouts", json!([{"layout": "us"}]))).await;
    assert_eq!(out.error, None);
    let layouts = &r.state.lock().unwrap().settings.input.keyboard.layouts;
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].layout, "us");
}

#[tokio::test]
async fn mouse_write_lands_in_mouse_table() {
    let (r, page) = rig().await;
    let out = run(&page, mouse("sensitivity", json!(0.4))).await;
    assert_eq!(out.error, None);
    assert_eq!(r.state.lock().unwrap().settings.mouse.sensitivity, 0.4);
    let out = run(&page, mouse("accel_profile", json!("flat"))).await;
    assert_eq!(out.state["input"]["mouse"]["accel_profile"], "flat");
    assert_eq!(
        out.state["input"]["mouse"]["sensitivity"], 0.4,
        "other fields stay"
    );
    assert_eq!(
        r.state.lock().unwrap().settings.mouse.accel_profile,
        AccelProfile::Flat
    );
    let out = run(&page, mouse("left_handed", Value::Null)).await;
    assert!(out.error.unwrap().contains("needs a value"));
    // History records it under the input page's domain.
    let domains: Vec<_> = r
        .history
        .entries
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.domain.clone())
        .collect();
    assert_eq!(domains, ["input", "input"]);
}

#[tokio::test]
async fn device_override_add_and_clear() {
    let (r, page) = rig().await;
    let out = run(&page, device("elan-touchpad", "sensitivity", json!(-0.3))).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["input"]["devices"]["elan-touchpad"]["sensitivity"],
        -0.3
    );
    run(
        &page,
        device("elan-touchpad", "natural_scroll", json!(true)),
    )
    .await;
    run(&page, device("elan-touchpad", "sensitivity", Value::Null)).await;
    let out = run(
        &page,
        device("elan-touchpad", "natural_scroll", Value::Null),
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["input"]["devices"], json!({}));
    assert!(r.state.lock().unwrap().settings.input.devices.is_empty());
    run(&page, device("logitech-mouse", "left_handed", json!(true))).await;
    let out = run(
        &page,
        Op::RemoveDevice {
            name: "logitech-mouse".into(),
        },
    )
    .await;
    assert_eq!(out.state["input"]["devices"], json!({}));
}

#[tokio::test]
async fn added_device_starts_from_the_mouse_speed() {
    let (_r, page) = rig().await;
    run(&page, mouse("sensitivity", json!(0.4))).await;
    let add = || Op::AddDevice {
        name: "logitech-mouse".into(),
    };
    let out = run(&page, add()).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["input"]["devices"]["logitech-mouse"],
        json!({"sensitivity": 0.4})
    );
    // Adding it again keeps what the device already has.
    run(&page, device("logitech-mouse", "sensitivity", json!(-0.2))).await;
    let out = run(&page, add()).await;
    assert_eq!(
        out.state["input"]["devices"]["logitech-mouse"]["sensitivity"],
        -0.2
    );
}

#[tokio::test]
async fn rejected_verify_restores_state() {
    let (r, page) = rig().await;
    let before = run(&page, Op::Refresh).await.state;
    *r.verify_error.lock().unwrap() = Some("bad option".into());
    let out = run(&page, touchpad("tap_to_click", json!(true))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Hyprland rejected the new config: bad option")
    );
    assert_eq!(out.state, before);
}

#[tokio::test]
async fn bad_layout_refused_with_reason() {
    let (_r, page) = rig().await;
    let out = run(&page, kb("layouts", json!([{"layout": "US!"}]))).await;
    assert!(
        out.error
            .as_deref()
            .unwrap()
            .contains("must be 2 to 8 lowercase letters"),
        "{out:?}"
    );
    assert_eq!(out.state["input"]["keyboard"]["layouts"], json!([]));
    let out = run(&page, kb("nope", json!(1))).await;
    assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
    let out = run(&page, device("x", "tap_to_click", json!(true))).await;
    assert_eq!(out.error.as_deref(), Some("unknown field \"tap_to_click\""));
}

/// Every field the page writes reads back on a fresh page object, so no control resets itself.
#[tokio::test]
async fn round_trip_every_field() {
    let (r, page) = rig().await;
    let writes: Vec<(Op, &str, Value)> = vec![
        (
            kb(
                "layouts",
                json!([{"layout": "us", "variant": "intl"}, {"layout": "de"}]),
            ),
            "/input/keyboard/layouts",
            json!([{"layout": "us", "variant": "intl"}, {"layout": "de"}]),
        ),
        (
            kb("switch_combo", json!("grp:alt_shift_toggle")),
            "/input/keyboard/switch_combo",
            json!("grp:alt_shift_toggle"),
        ),
        (
            kb("repeat_rate", json!(40)),
            "/input/keyboard/repeat_rate",
            json!(40),
        ),
        (
            kb("repeat_delay", json!(250)),
            "/input/keyboard/repeat_delay",
            json!(250),
        ),
        (
            mouse("sensitivity", json!(-0.5)),
            "/input/mouse/sensitivity",
            json!(-0.5),
        ),
        (
            mouse("accel_profile", json!("adaptive")),
            "/input/mouse/accel_profile",
            json!("adaptive"),
        ),
        (
            mouse("natural_scroll", json!(true)),
            "/input/mouse/natural_scroll",
            json!(true),
        ),
        (
            mouse("left_handed", json!(true)),
            "/input/mouse/left_handed",
            json!(true),
        ),
        (
            mouse("scroll_factor", json!(2.5)),
            "/input/mouse/scroll_factor",
            json!(2.5),
        ),
        (
            touchpad("natural_scroll", json!(false)),
            "/input/touchpad/natural_scroll",
            json!(false),
        ),
        (
            touchpad("tap_to_click", json!(true)),
            "/input/touchpad/tap_to_click",
            json!(true),
        ),
        (
            touchpad("disable_while_typing", json!(false)),
            "/input/touchpad/disable_while_typing",
            json!(false),
        ),
        (
            touchpad("scroll_factor", json!(0.5)),
            "/input/touchpad/scroll_factor",
            json!(0.5),
        ),
    ];
    let mut writes = writes;
    for (field, value) in [
        ("sensitivity", json!(0.25)),
        ("accel_profile", json!("flat")),
        ("natural_scroll", json!(true)),
        ("left_handed", json!(true)),
        ("scroll_factor", json!(3.0)),
    ] {
        writes.push((
            device("elan-touchpad", field, value.clone()),
            Box::leak(format!("/input/devices/elan-touchpad/{field}").into_boxed_str()),
            value,
        ));
    }
    assert_eq!(
        writes.len(),
        KEYBOARD_FIELDS.len() + MOUSE_FIELDS.len() + TOUCHPAD_FIELDS.len() + DEVICE_FIELDS.len(),
        "a field was added without a round trip"
    );
    for (op, pointer, _) in &writes {
        let out = run(&page, op.clone()).await;
        assert_eq!(out.error, None, "{pointer}");
    }
    let fresh = InputState::with_xkb(r.client.clone(), XkbLists::default());
    let state = run(&fresh, Op::Refresh).await.state;
    for (_, pointer, want) in &writes {
        assert_eq!(state.pointer(pointer), Some(want), "{pointer} reads back");
    }
}
