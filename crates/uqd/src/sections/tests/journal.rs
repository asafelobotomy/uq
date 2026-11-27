//! Journalling section and light-path writes.

use super::*;

#[tokio::test]
async fn every_section_write_journals_once() {
    let mut r = rig();
    let before = r.desktop.history(100).len();
    r.desktop
        .set_section("appearance", r#"{"ui_font":"Cantarell"}"#, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.history(100).len(), before + 1);
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.history(100).len(), before + 2);
    r.desktop
        .set_section(
            "input",
            r#"{"keyboard":{"layouts":[{"layout":"us"}]}}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    assert_eq!(r.desktop.history(100).len(), before + 3);
    // Refused writes (bad value, monitors, failed verify) add nothing.
    assert!(
        r.desktop
            .set_section("look", r#"{"gaps_in":999}"#, Origin::Settings)
            .await
            .is_err()
    );
    assert!(
        r.desktop
            .set_section(
                "monitors",
                r#"{"outputs":{"DP-1":{"scale":1.5}}}"#,
                Origin::Settings,
            )
            .await
            .is_err()
    );
    *r.comp.verify_error.lock().unwrap() = Some("no".into());
    assert!(
        r.desktop
            .set_section("look", r#"{"gaps_in":3}"#, Origin::Settings)
            .await
            .is_err()
    );
    assert_eq!(r.desktop.history(100).len(), before + 3);
    let newest = &r.desktop.history(1)[0];
    assert_eq!(
        (newest.domain.as_str(), newest.origin.as_str()),
        ("input", "settings")
    );
}

#[tokio::test]
async fn hostile_device_replies_are_bounded() {
    let r = rig();
    let many: Vec<_> = (0..500)
        .map(|i| serde_json::json!({"name": format!("m{i}"), "defaultSpeed": 0.0}))
        .collect();
    *r.comp.devices_json.lock().unwrap() =
        serde_json::json!({"mice": many, "keyboards": [{"name": 5}, {"name": null}, 7,
            {"name": "a\u{7}b"}], "touch": "nope"})
        .to_string();
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_input_devices().await.unwrap()).unwrap();
    assert_eq!(v["pointers"].as_array().unwrap().len(), 32);
    assert_eq!(v["keyboards"], serde_json::json!([{"name": "ab"}]));
    assert_eq!(v["touch"], serde_json::json!([]));
    for raw in ["[1,2]", "\"str\"", "null", "not json"] {
        *r.comp.devices_json.lock().unwrap() = raw.into();
        assert_eq!(
            r.desktop.list_input_devices().await.unwrap(),
            r#"{"keyboards":[],"pointers":[],"touch":[]}"#,
            "{raw}"
        );
    }
    *r.comp.devices_json.lock().unwrap() =
        r#"{"mice":[{"name":"a","defaultSpeed":"fast"},{"name":"b","defaultSpeed":null}]}"#.into();
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_input_devices().await.unwrap()).unwrap();
    assert_eq!(v["pointers"][0]["default_speed"], 0.0);
    assert_eq!(v["pointers"][1]["default_speed"], 0.0);
    // A number JSON can't hold makes the whole reply unreadable: empty lists, no panic.
    *r.comp.devices_json.lock().unwrap() = r#"{"mice":[{"name":"a","defaultSpeed":1e999}]}"#.into();
    assert_eq!(
        r.desktop.list_input_devices().await.unwrap(),
        r#"{"keyboards":[],"pointers":[],"touch":[]}"#
    );
}

#[tokio::test]
async fn setting_light_path_journals_through_persist() {
    let mut r = rig();
    let before = r.desktop.history(100).len();
    r.desktop
        .set_setting("idle.screen_off", serde_json::json!(500), Origin::Uqctl)
        .await
        .unwrap();
    assert_eq!(r.desktop.history(100).len(), before + 1);
    assert_eq!(r.desktop.history(1)[0].origin, "uqctl");
    let json = r#"{"outputs":{"*":{"image":"/a.png"}}}"#;
    r.desktop
        .set_section("wallpaper", json, Origin::Settings)
        .await
        .unwrap();
    assert_eq!(r.desktop.history(100).len(), before + 2);
    r.desktop
        .set_setting("nope", serde_json::json!(1), Origin::Settings)
        .await
        .unwrap_err();
    assert_eq!(r.desktop.history(100).len(), before + 2);
}
