//! The page snapshot, rows and apps.

use super::*;

#[test]
fn combo_from_qt_maps_keys_and_modifiers() {
    const SHIFT: u32 = 0x0200_0000;
    const CTRL: u32 = 0x0400_0000;
    const ALT: u32 = 0x0800_0000;
    const META: u32 = 0x1000_0000;
    let q = |key, m| combo_from_qt(key, m);
    assert_eq!(q(0x51, META | SHIFT).as_deref(), Some("SUPER + SHIFT + Q"));
    assert_eq!(q(0x20, META).as_deref(), Some("SUPER + Space"));
    assert_eq!(q(0x0100_0005, 0).as_deref(), Some("Return"));
    assert_eq!(q(0x0100_0004, META).as_deref(), Some("SUPER + Return"));
    assert_eq!(q(0x0100_0072, 0).as_deref(), Some("XF86AudioRaiseVolume"));
    assert_eq!(q(0x0100_0016, ALT).as_deref(), Some("ALT + Prior"));
    assert_eq!(
        q(0x0100_0030, CTRL | ALT).as_deref(),
        Some("CTRL + ALT + F1")
    );
    assert_eq!(q(0x0100_0047, 0).as_deref(), Some("F24"));
    assert_eq!(q(0x31, META).as_deref(), Some("SUPER + 1"));
    // Shift + 1 arrives as Key_Exclam; Shift + Tab as Key_Backtab.
    assert_eq!(q(0x21, META | SHIFT).as_deref(), Some("SUPER + SHIFT + 1"));
    assert_eq!(q(0x0100_0002, META).as_deref(), Some("SUPER + SHIFT + Tab"));
    // A modifier alone keeps waiting; so does a key uQ has no name for.
    for lone in [
        0x0100_0022,
        0x0100_0020,
        0x0100_0021,
        0x0100_0023,
        0x0100_0053,
    ] {
        assert_eq!(q(lone, META), None, "{lone:#x}");
    }
    assert_eq!(q(0x0100_0099, 0), None);
    assert_eq!(q(0x21, META), None, "a bare ! is layout-dependent");
    assert_eq!(q(-1, 0), None);
    // Whatever it says, uQ's own parser takes.
    for key in (0x20..0x7f).chain(0x0100_0000..0x0100_0100) {
        for m in [0, META, META | SHIFT] {
            if let Some(text) = q(key, m) {
                assert!(combo::parse(&text).is_ok(), "{key:#x} {text}");
            }
        }
    }
}

#[tokio::test]
async fn profile_next_offered_unbound() {
    let (_r, page) = rig().await;
    let out = op(&page, Op::Refresh).await;
    let next = row(&out.state, "uq.profile.next");
    assert_eq!(next["label"], "Switch to the next profile");
    assert_eq!(next["combo"], Value::Null);
    assert_eq!(next["combo_text"], Value::Null);
    assert_eq!(next["enabled"], true);
    let profiles = out.state["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == "profiles")
        .unwrap();
    assert!(
        profiles["binds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["id"] == "uq.profile.next")
    );
    // Giving it keys saves like any other.
    let out = op(
        &page,
        Op::SetCombo {
            id: "uq.profile.next".into(),
            combo: "SUPER + ALT + P".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        row(&out.state, "uq.profile.next")["combo_text"],
        "Super + Alt + P"
    );
}

#[tokio::test]
async fn snapshot_shape_and_catalogue() {
    let (_r, page) = rig().await;
    let out = op(&page, Op::Refresh).await;
    let groups = out.state["groups"].as_array().unwrap();
    let total: usize = groups
        .iter()
        .map(|g| g["binds"].as_array().unwrap().len())
        .sum();
    assert_eq!(total, 68, "every default, in a group");
    assert_eq!(groups[0]["id"], "apps_and_desktop");
    assert!(groups.iter().all(|g| g["id"] != "custom"));
    let b = row(&out.state, "uq.lock-recovery");
    assert_eq!(b["can_disable"], false);
    for k in [
        "id",
        "label",
        "combo",
        "combo_text",
        "default_combo_text",
        "enabled",
        "is_default",
        "changed",
        "can_disable",
        "action_text",
        "source",
        "line",
        "profile",
    ] {
        assert!(b.get(k).is_some(), "{k}");
    }
    assert_eq!(b["source"], "default");
    assert!(out.state["catalogue"]["keys"].as_array().unwrap().len() > 100);
    assert!(out.state["catalogue"]["actions"]["uq"].is_array());
    assert_eq!(out.state["limits"]["custom"], 100);
    assert_eq!(out.state["section"], Value::Null);
    assert_eq!(out.state["custom"], json!([]));
}

#[tokio::test]
async fn row_says_from_the_profile() {
    let (r, page) = rig().await;
    r.explain.set(
        "keybinds.binds.uq.close",
        json!({"source": "profile", "detail": {"name": "Quiet"}}),
    );
    let out = op(&page, Op::Refresh).await;
    assert_eq!(row(&out.state, "uq.close")["source"], "profile");
    assert_eq!(row(&out.state, "uq.close")["profile"], "Quiet");
    assert_eq!(row(&out.state, "uq.float")["profile"], "");
}

#[tokio::test]
async fn row_source_comes_from_explain() {
    let (r, page) = rig().await;
    r.explain.set(
        "keybinds.binds.uq.close",
        json!({"source": "user_lua", "detail": {"line": 7, "path": "/u/user.lua"}}),
    );
    let out = op(&page, Op::Refresh).await;
    assert_eq!(row(&out.state, "uq.close")["source"], "user_lua");
    assert_eq!(row(&out.state, "uq.close")["line"], 7);
    assert_eq!(row(&out.state, "uq.close")["path"], "/u/user.lua");
    assert_eq!(row(&out.state, "uq.float")["line"], Value::Null);
    // A failed Explain only drops the sources.
    r.explain.fail();
    let out = op(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(row(&out.state, "uq.close")["source"], "default");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = KeybindsState::with_dirs(connect(&bus).await, Vec::new());
    let out = op(&page, Op::Refresh).await;
    assert_eq!(
        out,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    assert_eq!(
        op(
            &page,
            Op::SetEnabled {
                id: "uq.close".into(),
                on: false
            }
        )
        .await,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    // And a pause with uqd down is "unavailable", not a failure.
    let out = op(&page, Op::PauseCapture).await;
    assert_eq!(
        out,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    assert_eq!(page.shown.lock().unwrap().capture.state, "unavailable");
    // check() with nothing read says so plainly instead of guessing.
    let v = page.check(&json!({"combo": "SUPER + F9"}));
    assert_eq!(v["valid"], false);
}

#[tokio::test]
async fn installed_apps_are_offered_for_start_an_app() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("firefox.desktop"),
        "[Desktop Entry]\nType=Application\nName=Firefox\nIcon=firefox\nExec=firefox %u\n",
    )
    .unwrap();
    let r = fake_uqd::rig(State::default()).await;
    let page = KeybindsState::with_dirs(r.client.clone(), vec![dir.path().to_path_buf()]);
    let out = op(&page, Op::Refresh).await;
    assert_eq!(
        out.state["apps"],
        json!([{"id": "firefox.desktop", "name": "Firefox", "icon": "firefox"}])
    );
    // And a shortcut that starts it saves as the action the editor builds.
    let bind = json!({"name": "Browser", "combo": "SUPER + SHIFT + P",
        "action": {"kind": "launch", "app": "firefox.desktop"}});
    let out = op(&page, Op::SaveCustom { bind }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["custom"][0]["action_text"], "Starts Firefox");
}

/// The settings QML tests feed the shortcut editor a few rows of the catalogue; each must still
/// be in the real one.
#[test]
fn fixture_is_a_slice_of_the_catalogue() {
    let file = include_str!("../../../../../../tests/settings-qml/fixtures/keybinds_catalogue.js");
    let lead = "var catalogue = ";
    let start = file.find(lead).expect("the fixture sets `catalogue`") + lead.len();
    let body = file[start..].trim_end().trim_end_matches(';');
    let fixture: Value = serde_json::from_str(body).expect("the fixture is JSON");
    let real = uq_core::keybinds::list::list(&Keybinds::default(), None);
    for key in fixture["keys"].as_array().unwrap() {
        assert!(real["keys"].as_array().unwrap().contains(key), "{key}");
    }
    for (group, rows) in fixture["actions"].as_object().unwrap() {
        for row in rows.as_array().unwrap() {
            assert!(
                real["actions"][group].as_array().unwrap().contains(row),
                "update tests/settings-qml/fixtures/keybinds_catalogue.js: {row}"
            );
        }
    }
}
