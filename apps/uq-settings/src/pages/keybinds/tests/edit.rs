//! Rebinding, custom shortcuts, checks and capture.

use super::*;

#[tokio::test]
async fn rebind_disable_reset_round_trip() {
    let (r, page) = rig().await;
    let first = op(&page, Op::Refresh).await;
    assert_eq!(first.error, None);
    assert_eq!(
        row(&first.state, "uq.lookseedo")["combo_text"],
        "Super + Space"
    );
    assert_eq!(row(&first.state, "uq.lookseedo")["changed"], false);
    assert_eq!(row(&first.state, "uq.lookseedo")["is_default"], true);

    let out = op(
        &page,
        Op::SetCombo {
            id: "uq.lookseedo".into(),
            combo: "win + k".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let s = row(&out.state, "uq.lookseedo");
    assert_eq!(s["combo"], "SUPER + K");
    assert_eq!(s["combo_text"], "Super + K");
    assert_eq!(s["default_combo_text"], "Super + Space");
    assert_eq!(s["changed"], true);
    assert_eq!(s["source"], "uq");

    let out = op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(row(&out.state, "uq.close")["enabled"], false);
    assert_eq!(row(&out.state, "uq.close")["changed"], true);
    // Reopening gives the same snapshot (§9's round trip).
    let reopened = KeybindsState::with_dirs(r.client.clone(), Vec::new());
    assert_eq!(
        op(&reopened, Op::Refresh).await.state["groups"],
        out.state["groups"]
    );

    // Back on, with nothing else changed: the override goes away.
    let out = op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: true,
        },
    )
    .await;
    assert_eq!(row(&out.state, "uq.close")["changed"], false);
    assert_eq!(r.state.lock().unwrap().keybinds.overrides.len(), 1);

    let out = op(
        &page,
        Op::Reset {
            id: "uq.lookseedo".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        row(&out.state, "uq.lookseedo")["combo_text"],
        "Super + Space"
    );
    assert!(r.state.lock().unwrap().keybinds.overrides.is_empty());
    assert_eq!(
        r.history.origins(),
        ["settings", "settings", "settings", "settings"]
    );
    let entries = r.history.entries.lock().unwrap();
    assert!(entries.iter().all(|e| e.domain == "keybinds"));
    assert!(
        entries[0].summary.contains("Open look=see>do_"),
        "{}",
        entries[0].summary
    );
}

#[tokio::test]
async fn reset_all_keeps_custom() {
    let (r, page) = rig().await;
    op(
        &page,
        Op::SaveCustom {
            bind: custom("Terminal too", "SUPER + F2"),
        },
    )
    .await;
    op(
        &page,
        Op::SetCombo {
            id: "uq.lookseedo".into(),
            combo: "SUPER + K".into(),
        },
    )
    .await;
    op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(r.state.lock().unwrap().keybinds.overrides.len(), 2);
    let out = op(&page, Op::ResetAll).await;
    assert_eq!(out.error, None);
    assert!(r.state.lock().unwrap().keybinds.overrides.is_empty());
    assert_eq!(out.state["custom"].as_array().unwrap().len(), 1);
    assert_eq!(out.state["custom"][0]["name"], "Terminal too");
    assert_eq!(
        row(&out.state, "uq.lookseedo")["combo_text"],
        "Super + Space"
    );
    assert!(
        out.state["notice"]["text"]
            .as_str()
            .unwrap()
            .contains("back")
    );
    // One of your own isn't "reset".
    let id = custom_ids(&r).remove(0);
    assert!(op(&page, Op::Reset { id }).await.error.is_some());
}

#[tokio::test]
async fn save_custom_gets_id_and_journals() {
    let (r, page) = rig().await;
    let out = op(
        &page,
        Op::SaveCustom {
            bind: custom("Open my terminal", "SUPER + F2"),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let id = custom_ids(&r).remove(0);
    assert!(id.starts_with("k-"), "{id}");
    let row = row(&out.state, &id);
    assert_eq!(row["name"], "Open my terminal");
    assert_eq!(row["combo_text"], "Super + F2");
    assert_eq!(row["locked"], false);
    assert_eq!(
        out.state["notice"]["text"],
        "Added the shortcut “Open my terminal”"
    );
    assert_eq!(r.history.origins(), ["settings"]);
    {
        let entries = r.history.entries.lock().unwrap();
        assert_eq!(entries[0].domain, "keybinds");
        assert!(
            entries[0].summary.contains("Open my terminal"),
            "{}",
            entries[0].summary
        );
    }
    // A save with the id replaces it; delete removes it.
    let mut again = custom("Renamed", "SUPER + F3");
    again["id"] = json!(id);
    let out = op(&page, Op::SaveCustom { bind: again }).await;
    assert_eq!(out.error, None);
    assert_eq!(custom_ids(&r), std::slice::from_ref(&id));
    assert_eq!(out.state["custom"][0]["name"], "Renamed");
    let out = op(&page, Op::DeleteCustom { id: id.clone() }).await;
    assert!(out.state["custom"].as_array().unwrap().is_empty());
    assert!(op(&page, Op::DeleteCustom { id }).await.error.is_some());
}

#[tokio::test]
async fn check_warns_as_you_type() {
    let (r, page) = rig().await;
    // The page has read the shortcuts once.
    op(&page, Op::Refresh).await;
    let sent = |r: &Rig| r.history.entries.lock().unwrap().len();
    let before = (sent(&r), r.binds.calls().len());

    for combo in ["SUPER + Space", "WIN + space"] {
        let v = page.check(&json!({"combo": combo, "name": "Mine"}));
        assert_eq!(v["valid"], false, "{combo}");
        assert_eq!(v["conflicts"][0]["label"], "Open look=see>do_");
        assert_eq!(v["conflicts"][0]["id"], "uq.lookseedo");
        assert_eq!(v["combo_text"], "Super + Space");
        let error = v["error"].as_str().unwrap();
        assert!(
            error.contains("Open look=see>do_") && error.contains("Super + Space"),
            "{error}"
        );
    }
    let free = page.check(&json!({"combo": "SUPER + SHIFT + F9"}));
    assert_eq!(free["valid"], true);
    assert_eq!(free["error"], "");
    assert_eq!(free["conflicts"], json!([]));
    assert_eq!(free["combo_text"], "Super + Shift + F9");
    // Editing a default: its own keys are no conflict, another's are.
    let own = page.check(&json!({"id": "uq.lookseedo", "combo": "SUPER + Space"}));
    assert_eq!(own["valid"], true);
    let other = page.check(&json!({"id": "uq.lookseedo", "combo": "SUPER + Q"}));
    assert_eq!(other["valid"], false);
    assert_eq!(other["conflicts"][0]["label"], "Close the window");
    // A bare letter, nothing typed yet, a key that doesn't exist.
    assert_eq!(page.check(&json!({"combo": "K"}))["valid"], false);
    assert_eq!(page.check(&json!({"combo": ""}))["valid"], false);
    assert_eq!(
        page.check(&json!({"combo": "SUPER + Nope"}))["valid"],
        false
    );
    // Switched off, it takes no keys.
    assert_eq!(
        page.check(&json!({"combo": "SUPER + Space", "enabled": false}))["conflicts"],
        json!([])
    );
    // No call reached uqd, and nothing was written.
    assert_eq!((sent(&r), r.binds.calls().len()), before);
    assert!(r.state.lock().unwrap().keybinds.custom.is_empty());
}

#[tokio::test]
async fn check_flags_marked_user_lua_without_refusing() {
    let (r, page) = rig().await;
    r.binds.set_user_lua(
        "-- mine\nhl.bind(\"SUPER + T\", hl.dsp.exec_cmd(\"foot\")) -- uq:bind Terminal (foot)\n",
    );
    let out = op(&page, Op::Refresh).await;
    assert_eq!(out.state["user_lua"][0]["line"], 2);
    assert_eq!(out.state["user_lua"][0]["combo_text"], "Super + T");
    let v = page.check(&json!({"combo": "SUPER + T", "name": "Mine"}));
    assert_eq!(v["valid"], true, "a marked bind warns but saves");
    assert_eq!(v["user_lua"][0]["line"], 2);
    assert!(v["user_lua"][0]["text"].as_str().unwrap().contains("foot"));
    assert_eq!(
        page.check(&json!({"combo": "SUPER + F9"}))["user_lua"],
        json!([])
    );
    let saved = op(
        &page,
        Op::SaveCustom {
            bind: custom("Mine", "SUPER + T"),
        },
    )
    .await;
    assert_eq!(saved.error, None);
}

#[tokio::test]
async fn uqd_refusal_shows_plain_reason() {
    let (r, page) = rig().await;
    *r.refuse_next.lock().unwrap() = Some("invalid value for keybinds: your files changed".into());
    let out = op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: false,
        },
    )
    .await;
    let shown = crate::pages::runner::plain_refusal(&out.error.unwrap());
    assert_eq!(shown, "your files changed");
    assert_eq!(
        row(&out.state, "uq.close")["enabled"],
        true,
        "the switch snaps back"
    );
    // A conflict write is refused with both names and nothing changes.
    let out = op(
        &page,
        Op::SaveCustom {
            bind: custom("Mine", "SUPER + Space"),
        },
    )
    .await;
    let error = out.error.unwrap();
    assert!(
        error.contains("Mine") && error.contains("Open look=see>do_"),
        "{error}"
    );
    assert!(r.state.lock().unwrap().keybinds.custom.is_empty());
    // Hyprland rejecting the render is shown on the page and clears on the next good write.
    *r.verify_error.lock().unwrap() = Some("bad bind".into());
    let out = op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: false,
        },
    )
    .await;
    assert!(out.error.unwrap().contains("bad bind"));
    assert_eq!(out.state["section"]["source"], "error");
    assert!(
        out.state["section"]["message"]
            .as_str()
            .unwrap()
            .contains("bad bind")
    );
    assert_eq!(row(&out.state, "uq.close")["source"], "error");
    let out = op(
        &page,
        Op::SetEnabled {
            id: "uq.close".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["section"], Value::Null);
}

#[tokio::test]
async fn capture_pause_resume_and_unavailable() {
    let (r, page) = rig().await;
    let idle = op(&page, Op::Refresh).await;
    assert_eq!(idle.state["capture"]["state"], "idle");
    assert_eq!(idle.state["capture"]["token"], "");

    let out = op(&page, Op::PauseCapture).await;
    assert_eq!(out.error, None);
    let token = out.state["capture"]["token"].as_str().unwrap().to_string();
    assert_eq!(out.state["capture"]["state"], "paused");
    assert_eq!(token.len(), 32);
    assert!(
        out.state["capture"]["seq"].as_u64().unwrap()
            > idle.state["capture"]["seq"].as_u64().unwrap()
    );

    let out = op(&page, Op::ResumeCapture).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["capture"]["state"], "idle");
    assert_eq!(
        r.binds.calls(),
        ["pause".to_string(), format!("resume {token}")]
    );
    // Resuming when nothing is paused calls nothing.
    op(&page, Op::ResumeCapture).await;
    assert_eq!(r.binds.calls().len(), 2);

    // Hyprland isn't answering: no error, the page types keys instead.
    r.binds.fail_pauses();
    let out = op(&page, Op::PauseCapture).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["capture"]["state"], "unavailable");
    assert_eq!(out.state["capture"]["token"], "");
    op(&page, Op::ResumeCapture).await;
    assert_eq!(r.binds.calls().len(), 3, "nothing to resume");
}

#[tokio::test]
async fn capture_resume_after_uqd_timed_out_is_not_an_error() {
    let (r, page) = rig().await;
    op(&page, Op::PauseCapture).await;
    // uqd's 15 s passed: the token is gone there.
    r.binds.paused.lock().unwrap().take();
    let out = op(&page, Op::ResumeCapture).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["capture"]["state"], "idle");
}
