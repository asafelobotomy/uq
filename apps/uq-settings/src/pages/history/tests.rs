use super::*;
use crate::pages::fake_uqd::{self, Rig, now};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

/// A rig whose app started a minute ago.
async fn rig() -> (Rig, HistoryState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = HistoryState::new(r.client.clone(), now() - 60);
    (r, page)
}

/// Write `domain`'s `field` the way `origin` would.
async fn write(r: &Rig, origin: &str, domain: &str, field: &str, value: Value) -> String {
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    let mut section = parse_json(&proxy.get_section(domain).await.unwrap()).unwrap();
    section[field] = value;
    proxy
        .set_section(domain, &section.to_string(), origin)
        .await
        .unwrap();
    r.history.last_id()
}

fn section(r: &Rig, domain: &str) -> Value {
    r.state.lock().unwrap().section(domain).unwrap()
}

#[tokio::test]
async fn undo_last_undoes_only_own_entries() {
    let (r, page) = rig().await;
    // Before this session, from the app itself: not ours to undo any more.
    r.history.seed(Entry::new(
        now() - 3600,
        "settings",
        "wallpaper",
        "Wallpaper changed",
        json!({}),
        json!({}),
    ));
    write(&r, "uqctl", "look", "rounding", json!(4)).await;
    let own = write(&r, ORIGIN, "appearance", "font_size", json!(12)).await;
    write(&r, "shell", "shell", "clock_24h", json!(true)).await;

    let out = run(&page, Op::UndoLast).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Change undone");
    assert_eq!(section(&r, "appearance")["font_size"], Value::Null);
    assert_eq!(
        section(&r, "look")["rounding"],
        4,
        "the terminal's change stays"
    );
    assert_eq!(
        section(&r, "shell")["clock_24h"],
        true,
        "the panel's change stays"
    );
    let undo = r.history.entries.lock().unwrap().last().cloned().unwrap();
    assert_eq!(
        (undo.origin.as_str(), undo.domain.as_str()),
        ("restore", "appearance")
    );
    assert_eq!(undo.before["font_size"], 12, "it undid {own}");

    // Nothing of ours is left: Ctrl+Z does nothing, however many foreign entries remain.
    let journal = r.history.origins().len();
    let out = run(&page, Op::UndoLast).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Nothing to undo");
    assert_eq!(
        r.history.origins().len(),
        journal,
        "no further undo was made"
    );
    assert_eq!(section(&r, "look")["rounding"], 4);
    assert_eq!(section(&r, "shell")["clock_24h"], true);
    assert_eq!(out.state["canUndo"], false);
    assert_eq!(out.state["canRedo"], true);
}

#[tokio::test]
async fn undo_last_steps_back_through_own_changes_to_one_domain() {
    let (r, page) = rig().await;
    write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    write(&r, ORIGIN, "look", "rounding", json!(8)).await;
    assert_eq!(run(&page, Op::UndoLast).await.error, None);
    assert_eq!(section(&r, "look")["rounding"], 4);
    // The undo just written changed `look` again, but it was ours: no conflict to ask about.
    let out = run(&page, Op::UndoLast).await;
    assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
    assert_eq!(section(&r, "look")["rounding"], Value::Null);
}

#[tokio::test]
async fn redo_reapplies_after_undo() {
    let (r, page) = rig().await;
    write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    write(&r, ORIGIN, "look", "rounding", json!(8)).await;
    run(&page, Op::UndoLast).await;
    run(&page, Op::UndoLast).await;
    assert_eq!(section(&r, "look")["rounding"], Value::Null);

    let out = run(&page, Op::RedoLast).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Change redone");
    assert_eq!(section(&r, "look")["rounding"], 4);
    let out = run(&page, Op::RedoLast).await;
    assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
    assert_eq!(section(&r, "look")["rounding"], 8);
    assert_eq!(out.state["canRedo"], false);
    assert_eq!(
        run(&page, Op::RedoLast).await.state["notice"]["text"],
        "Nothing to redo"
    );

    // A redone change is undoable again.
    run(&page, Op::UndoLast).await;
    assert_eq!(section(&r, "look")["rounding"], 4);
    run(&page, Op::RedoLast).await;
    assert_eq!(section(&r, "look")["rounding"], 8);
}

#[tokio::test]
async fn new_change_clears_redo() {
    let (r, page) = rig().await;
    write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    let out = run(&page, Op::UndoLast).await;
    assert_eq!(out.state["canRedo"], true);

    // Another app's change keeps redo; a new change of the app's own drops it.
    write(&r, "uqctl", "appearance", "ui_font", json!("Inter")).await;
    assert_eq!(run(&page, Op::Refresh).await.state["canRedo"], true);
    write(&r, ORIGIN, "appearance", "font_size", json!(12)).await;
    let out = run(&page, Op::RedoLast).await;
    assert_eq!(out.state["notice"]["text"], "Nothing to redo");
    assert_eq!(out.state["canRedo"], false);
    assert_eq!(section(&r, "look")["rounding"], Value::Null, "not redone");
    // Ctrl+Z now undoes the new change.
    run(&page, Op::UndoLast).await;
    assert_eq!(section(&r, "appearance")["font_size"], Value::Null);
}

#[tokio::test]
async fn conflict_surfaces_as_error_then_force_applies() {
    let (r, page) = rig().await;
    let own = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    write(&r, "uqctl", "look", "border_width", json!(3)).await;

    let out = run(&page, Op::UndoLast).await;
    assert_eq!(
        out.error, None,
        "the window asks instead of showing an error"
    );
    let conflict = &out.state["conflict"];
    assert_eq!(conflict["id"], own.as_str());
    assert_eq!(conflict["redo"], false);
    // The window asks the question; uqd's own "Undo anyway?" would ask it twice.
    assert_eq!(conflict["message"], "look was changed again after this.");
    assert_eq!(section(&r, "look")["rounding"], 4, "nothing undone yet");

    // "Undo anyway".
    let out = run(
        &page,
        Op::Undo {
            id: own,
            force: true,
        },
    )
    .await;
    assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
    assert_eq!(out.state["notice"]["text"], "Change undone");
    let look = section(&r, "look");
    assert_eq!(look["rounding"], Value::Null);
    assert_eq!(
        look["border_width"], 3,
        "only the undone key goes back, as uqd does; the other change stays"
    );
    assert_eq!(out.state["canRedo"], true, "a forced undo can be redone");
}

#[tokio::test]
async fn redo_conflict_says_redo() {
    let (r, page) = rig().await;
    write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    let undo = {
        run(&page, Op::UndoLast).await;
        r.history.last_id()
    };
    write(&r, "uqctl", "look", "border_width", json!(3)).await;
    let out = run(&page, Op::RedoLast).await;
    let conflict = &out.state["conflict"];
    assert_eq!(conflict["id"], undo.as_str());
    assert_eq!(conflict["redo"], true);
    assert_eq!(conflict["message"], "look was changed again after this.");
    assert_eq!(
        section(&r, "look")["rounding"],
        Value::Null,
        "not redone yet"
    );
}

#[tokio::test]
async fn picked_undo_of_another_change_is_not_ours() {
    let (r, page) = rig().await;
    let other = write(&r, "uqctl", "look", "gaps_in", json!(6)).await;
    let own = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    // The user picks the terminal's change from the list and forces its undo.
    let out = run(
        &page,
        Op::Undo {
            id: other,
            force: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    // That undo wrote `look` after the app's own change, and it was not a Ctrl+Z step: Ctrl+Z
    // must ask, not silently write back a section that still holds the terminal's change.
    let out = run(&page, Op::UndoLast).await;
    assert_eq!(out.state["conflict"]["id"], own.as_str());
    assert_eq!(out.state["conflict"]["redo"], false);
}

#[tokio::test]
async fn picked_undo_keeps_uqds_check_and_joins_the_stack() {
    let (r, page) = rig().await;
    let first = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
    let second = write(&r, ORIGIN, "look", "gaps_in", json!(6)).await;

    // Picking the older one: the newer own change to `look` is still live, so uqd asks.
    let out = run(
        &page,
        Op::Undo {
            id: first,
            force: false,
        },
    )
    .await;
    assert_eq!(
        out.state["conflict"]["message"],
        "look was changed again after this."
    );
    assert_eq!(section(&r, "look")["gaps_in"], 6);

    // Picking the newest from the list counts like Ctrl+Z: it can be redone, and Ctrl+Z
    // goes on to the one before.
    let out = run(
        &page,
        Op::Undo {
            id: second,
            force: false,
        },
    )
    .await;
    assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
    assert_eq!(out.state["canRedo"], true);
    assert_eq!(section(&r, "look")["gaps_in"], Value::Null);
    run(&page, Op::UndoLast).await;
    assert_eq!(section(&r, "look")["rounding"], Value::Null);
    run(&page, Op::RedoLast).await;
    run(&page, Op::RedoLast).await;
    let look = section(&r, "look");
    assert_eq!(
        (&look["rounding"], &look["gaps_in"]),
        (&json!(4), &json!(6))
    );
}

#[tokio::test]
async fn restore_requires_existing_id() {
    let (r, page) = rig().await;
    let saved = State {
        theme: "dracula".into(),
        ..State::default()
    };
    let auto = r.history.add_point(
        "Before today's changes",
        PointKind::Auto(uq_core::history::AutoReason::Daily),
        saved,
    );

    let out = run(
        &page,
        Op::Restore {
            id: "1700000000-abcdef".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("no restore point 1700000000-abcdef")
    );
    assert!(r.history.origins().is_empty(), "nothing was restored");

    let out = run(&page, Op::Restore { id: auto.clone() }).await;
    assert_eq!(out.error, None);
    // The way back is the point uqd saved first: newest, so at the top of the list.
    assert_eq!(
        out.state["notice"]["text"],
        "Settings restored. To go back, restore the top restore point, \u{201c}Saved \
         automatically before a restore\u{201d}, on the History page."
    );
    assert_eq!(
        out.state["points"][0]["kind"],
        json!({"auto": "before_restore"})
    );
    assert_eq!(r.state.lock().unwrap().theme, "dracula");
    assert_eq!(r.history.origins(), ["restore"]);

    // Restoring is not one of the app's own changes: Ctrl+Z leaves it.
    let out = run(&page, Op::UndoLast).await;
    assert_eq!(out.state["notice"]["text"], "Nothing to undo");
    assert_eq!(r.state.lock().unwrap().theme, "dracula");
}

#[tokio::test]
async fn restore_points_save_and_delete_manual_only() {
    let (r, page) = rig().await;
    let auto = r.history.add_point(
        "0.9.0",
        PointKind::Auto(uq_core::history::AutoReason::NewVersion),
        State::default(),
    );
    let out = run(
        &page,
        Op::SaveRestorePoint {
            name: "  Before the party  ".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Restore point saved");
    let points = out.state["points"].as_array().unwrap().clone();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0]["name"], "Before the party");
    assert_eq!(points[0]["kind"], "manual");

    let out = run(&page, Op::SaveRestorePoint { name: " ".into() }).await;
    assert_eq!(out.error.as_deref(), Some("give the restore point a name"));
    let long = "x".repeat(150);
    run(&page, Op::SaveRestorePoint { name: long }).await;

    let out = run(&page, Op::DeleteRestorePoint { id: auto.clone() }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("automatic restore points are removed by uQ itself")
    );
    let manual = points[0]["id"].as_str().unwrap().to_string();
    let out = run(&page, Op::DeleteRestorePoint { id: manual }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Restore point deleted");
    let names: Vec<_> = out.state["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().chars().count())
        .collect();
    assert_eq!(
        names,
        [100, 5],
        "the long name is cut; the auto point stays"
    );
}

#[tokio::test]
async fn snapshot_lists_entries_and_session_start() {
    let (r, page) = rig().await;
    let id = write(&r, "uqctl", "look", "rounding", json!(4)).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["session_start"], page.session_start);
    assert_eq!(out.state["entries"][0]["id"], id.as_str());
    assert_eq!(out.state["entries"][0]["origin"], "uqctl");
    assert_eq!(out.state["entries"][0]["domain"], "look");
    assert_eq!(out.state["notice"], json!({"seq": 0, "text": ""}));
    assert_eq!(out.state["canUndo"], false);
}

#[tokio::test]
async fn snapshot_names_profiles() {
    let (r, page) = rig().await;
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    let toml = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n\n[look]\ngaps_in = 3\n";
    proxy.set_profile(toml, false).await.unwrap();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["profiles"], json!({"gaming": "Gaming"}));
}

#[tokio::test]
async fn snapshot_without_profiles_has_an_empty_map() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["profiles"], json!({}));
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = HistoryState::new(connect(&bus).await, 0);
    assert_eq!(
        run(&page, Op::UndoLast).await,
        Outcome {
            state: json!({}),
            error: None
        }
    );
}
