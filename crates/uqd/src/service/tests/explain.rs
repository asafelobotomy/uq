//! Explain: where a setting value comes from.

use super::*;

#[tokio::test]
async fn explain_reports_user_lua_override_with_line() {
    let mut r = rig();
    r.desktop
        .set_section("look", r#"{"gaps_in":5}"#, Origin::Settings)
        .await
        .unwrap();
    user_lua(
        &r,
        "-- mine\nhl.config({ general = {\n    gaps_in = 2,\n} })\n",
    );
    r.comp
        .set_option("general:gaps_in", serde_json::json!("2 2 2 2"));
    let v = r.desktop.explain("look").await.unwrap();
    let row = &v["gaps_in"];
    assert_eq!(row["source"], "user_lua", "{v}");
    assert_eq!(row["stored"], 5);
    assert_eq!(row["live"], "2 2 2 2");
    assert_eq!(row["detail"]["line"], 3);
    assert_eq!(
        row["detail"]["path"],
        r.desktop.paths().user_lua.to_string_lossy().as_ref()
    );
}

#[tokio::test]
async fn explain_input_gestures_row() {
    let mut r = rig();
    let line7 = "-- a\n-- b\n-- c\n-- d\n-- e\n-- f\nhl.gesture({ fingers = 3 })\n";
    user_lua(&r, line7);
    let v = r.desktop.explain("input").await.unwrap();
    let row = &v["gestures"];
    assert_eq!(row["source"], "user_lua", "{v}");
    assert_eq!(row["detail"]["line"], 7);
    assert_eq!(
        row["detail"]["path"],
        r.desktop.paths().user_lua.to_string_lossy().as_ref()
    );
    // In a comment it doesn't count: uQ's own default.
    user_lua(&r, "-- hl.gesture({ fingers = 3 })\n");
    let v = r.desktop.explain("input").await.unwrap();
    assert_eq!(v["gestures"]["source"], "default", "{v}");
    assert!(v["gestures"]["stored"].is_null());
    // A stored list is uQ's.
    r.desktop
        .set_section(
            "input",
            r#"{"gestures":[{"fingers":3,"direction":"horizontal","action":"workspace","enabled":true}]}"#,
            Origin::Settings,
        )
        .await
        .unwrap();
    let v = r.desktop.explain("input").await.unwrap();
    assert_eq!(v["gestures"]["source"], "uq", "{v}");
    assert_eq!(v["gestures"]["stored"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn explain_input_swipe_rows() {
    let mut r = rig();
    r.desktop
        .set_section("input", r#"{"swipe":{"distance":400}}"#, Origin::Settings)
        .await
        .unwrap();
    r.comp
        .set_option("gestures:workspace_swipe_distance", serde_json::json!(400));
    let v = r.desktop.explain("input").await.unwrap();
    assert_eq!(v["swipe.distance"]["source"], "uq", "{v}");
    // Something else changed it at runtime.
    r.comp
        .set_option("gestures:workspace_swipe_distance", serde_json::json!(250));
    let v = r.desktop.explain("input").await.unwrap();
    assert_eq!(v["swipe.distance"]["source"], "runtime", "{v}");
    assert_eq!(v["swipe.distance"]["live"], 250);
}

#[tokio::test]
async fn explain_reports_runtime_when_user_lua_silent() {
    let mut r = rig();
    r.desktop
        .set_section("look", r#"{"gaps_in":5,"rounding":4}"#, Origin::Settings)
        .await
        .unwrap();
    r.comp
        .set_option("general:gaps_in", serde_json::json!("8 8 8 8"));
    r.comp
        .set_option("decoration:rounding", serde_json::json!(4));
    r.sys
        .set_value("org.gnome.desktop.interface", "cursor-size", "48");
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "runtime", "{v}");
    assert_eq!(v["gaps_in"]["detail"], serde_json::Value::Null);
    assert_eq!(v["rounding"]["source"], "uq", "{v}");
    assert_eq!(v["rounding"]["live"], 4);
    // A gsettings key changed by another app is runtime too.
    let v = r.desktop.explain("appearance").await.unwrap();
    assert_eq!(v["cursor_size"]["source"], "runtime", "{v}");
    assert_eq!(v["cursor_size"]["live"], "48");
}

#[tokio::test]
async fn explain_null_key_reports_theme_or_default() {
    let r = rig();
    // graphite's gap_out token is 12; blur is left to base.lua.
    r.comp
        .set_option("general:gaps_out", serde_json::json!("12 12 12 12"));
    r.comp
        .set_option("decoration:blur:enabled", serde_json::json!(true));
    r.sys
        .set_value("org.gnome.desktop.interface", "font-name", "Inter 11");
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_out"]["source"], "theme", "{v}");
    assert_eq!(v["gaps_out"]["stored"], serde_json::Value::Null);
    assert_eq!(v["blur"]["source"], "default", "{v}");
    assert_eq!(v["blur"]["live"], true);
    assert_eq!(v["animation_speed"]["source"], "default", "{v}");
    let v = r.desktop.explain("layouts").await.unwrap();
    assert_eq!(v["dwindle.preserve_split"]["source"], "default", "{v}");
    let v = r.desktop.explain("appearance").await.unwrap();
    assert_eq!(v["ui_font"]["source"], "theme", "{v}");
    assert_eq!(v["font_size"]["source"], "theme", "{v}");
    // A base.lua default that user.lua sets is still found.
    user_lua(
        &r,
        "hl.config({ decoration = { blur = { enabled = false } } })\n",
    );
    r.comp
        .set_option("decoration:blur:enabled", serde_json::json!(false));
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["blur"]["source"], "user_lua", "{v}");
    assert_eq!(v["blur"]["detail"]["line"], 1);
    assert!(matches!(
        r.desktop.explain("bogus").await,
        Err(DesktopError::InvalidArgument(_))
    ));
    assert_eq!(
        r.desktop.explain("wallpaper").await.unwrap(),
        serde_json::json!({})
    );
}

#[tokio::test]
async fn explain_after_rejected_write_reports_error_then_clears() {
    let mut r = rig();
    r.comp
        .set_option("general:gaps_in", serde_json::json!("5 5 5 5"));
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: bad".into());
    let err = r
        .desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "error", "{v}");
    assert_eq!(v["gaps_in"]["detail"]["message"], "user.lua:3: bad");
    assert_eq!(v["rounding"]["source"], "error", "{v}");
    let other = r.desktop.explain("layouts").await.unwrap();
    assert_ne!(other["default"]["source"], "error", "{other}");
    *r.comp.verify_error.lock().unwrap() = None;
    r.desktop.reapply("look").await.unwrap();
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "theme", "{v}");
}

#[tokio::test]
async fn explain_without_compositor_uses_state() {
    let mut r = rig_with(false, |_| {});
    r.desktop
        .set_section("look", r#"{"gaps_in":9}"#, Origin::Settings)
        .await
        .unwrap();
    user_lua(&r, "general.gaps_in = 2\n");
    r.comp
        .set_option("general:gaps_in", serde_json::json!("2 2 2 2"));
    r.sys
        .set_value("org.gnome.desktop.interface", "cursor-size", "48");
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "uq", "{v}");
    assert_eq!(v["gaps_in"]["live"], serde_json::Value::Null);
    assert_eq!(v["gaps_out"]["source"], "theme", "{v}");
    assert_eq!(v["blur"]["source"], "default", "{v}");
    let v = r.desktop.explain("appearance").await.unwrap();
    assert_eq!(v["cursor_size"]["source"], "theme", "{v}");
}
