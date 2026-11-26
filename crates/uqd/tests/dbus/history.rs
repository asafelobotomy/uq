//! Undo, restore points and the origin a write records.

use super::*;

#[tokio::test]
async fn set_section_with_origin_records_it() {
    let s = served().await;
    let mut changed = s.proxy.receive_signal("HistoryChanged").await.unwrap();
    s.set_gaps(9, "uqctl").await;
    let history = s.history().await;
    assert_eq!(history.len(), 1, "{history:?}");
    assert_eq!(history[0]["origin"], "uqctl");
    assert_eq!(history[0]["domain"], "look");
    assert_eq!(history[0]["after"]["gaps_in"], 9);
    tokio::time::timeout(std::time::Duration::from_secs(5), changed.next())
        .await
        .unwrap()
        .unwrap();

    // An empty origin is recorded as unknown; the limit caps the list.
    s.set_gaps(10, "").await;
    let history = s.history().await;
    assert_eq!(history[0]["origin"], "unknown");
    let one: String = s.proxy.call("ListHistory", &(1u32,)).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&one)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn undo_round_trip_over_dbus() {
    let s = served().await;
    let before = s.gaps().await;
    s.set_gaps(9, "settings").await;
    let id = s.history().await[0]["id"].as_str().unwrap().to_string();
    let new_id = s.undo(&id, false).await.unwrap();
    assert_eq!(s.gaps().await, before);
    let history = s.history().await;
    assert_eq!(history[0]["id"], new_id.as_str());
    assert_eq!(history[0]["origin"], "restore");
    assert_eq!(history[0]["domain"], "look");
}

#[tokio::test]
async fn undo_conflict_then_force() {
    let s = served().await;
    let before = s.gaps().await;
    s.set_gaps(9, "settings").await;
    let first = s.history().await[0]["id"].as_str().unwrap().to_string();
    s.set_gaps(12, "uqctl").await;

    let err = s.undo(&first, false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    assert!(
        error_message(&err).ends_with("was changed again after this. Undo anyway?"),
        "{err:?}"
    );
    assert_eq!(s.gaps().await, 12);

    s.undo(&first, true).await.unwrap();
    assert_eq!(s.gaps().await, before);
}

#[tokio::test]
async fn undo_of_a_section_entry_keeps_later_changes_to_other_keys() {
    let s = served().await;
    s.set_look(r#"{"gaps_in":5,"rounding":3}"#, "settings")
        .await;
    let a = s
        .set_look(r#"{"gaps_in":8,"rounding":3}"#, "settings")
        .await;
    s.set_look(r#"{"gaps_in":8,"rounding":6}"#, "settings")
        .await;

    s.undo(&a, true).await.unwrap();
    let look = s.look().await;
    assert_eq!(look["gaps_in"], 5, "{look}");
    assert_eq!(look["rounding"], 6, "{look}");
    let entry = s.history().await[0].clone();
    assert_eq!(entry["summary"], "Gaps between windows: 8 → 5");
}

#[tokio::test]
async fn undone_change_does_not_come_back_when_a_later_one_is_undone() {
    let s = served().await;
    s.set_look(r#"{"gaps_in":5,"rounding":3}"#, "settings")
        .await;
    let gaps = s
        .set_look(r#"{"gaps_in":8,"rounding":3}"#, "settings")
        .await;
    let rounding = s
        .set_look(r#"{"gaps_in":8,"rounding":6}"#, "settings")
        .await;

    // Undo the gaps change from the session sheet: rounding changed since, so it asks first.
    let err = s.undo(&gaps, false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    s.undo(&gaps, true).await.unwrap();
    // Then Ctrl+Z, which forces through the app's own later entries, undoes rounding.
    s.undo(&rounding, true).await.unwrap();

    let look = s.look().await;
    assert_eq!(
        look["gaps_in"], 5,
        "the undone gaps change came back: {look}"
    );
    assert_eq!(look["rounding"], 3, "{look}");
}

#[tokio::test]
async fn undo_of_a_wallpaper_entry_keeps_other_screens() {
    let s = served().await;
    let all = s
        .set_section(
            "wallpaper",
            r#"{"outputs":{"*":{"image":"/a.png"}}}"#,
            "settings",
        )
        .await;
    s.set_section(
        "wallpaper",
        r#"{"outputs":{"*":{"image":"/a.png"},"DP-1":{"image":"/b.png"}}}"#,
        "settings",
    )
    .await;
    s.undo(&all, true).await.unwrap();
    let got: String = s.proxy.call("GetSection", &("wallpaper",)).await.unwrap();
    let got: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(got["outputs"]["DP-1"]["image"], "/b.png", "{got}");
    assert!(got["outputs"].get("*").is_none(), "{got}");
}

#[tokio::test]
async fn undo_unknown_id_is_invalid_argument() {
    let s = served().await;
    let err = s.undo("1700000000-0000", false).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
}

#[tokio::test]
async fn undo_theme_and_schedule_pseudo_domains() {
    let s = served().await;
    let (name, variant): (String, String) = s.proxy.call("GetTheme", &()).await.unwrap();
    let () = s
        .proxy
        .call("SetTheme", &("dracula", "light", "settings"))
        .await
        .unwrap();
    let id = s.history().await[0]["id"].as_str().unwrap().to_string();
    s.undo(&id, false).await.unwrap();
    let now: (String, String) = s.proxy.call("GetTheme", &()).await.unwrap();
    assert_eq!(now, (name, variant));

    let () = s
        .proxy
        .call(
            "SetSchedule",
            &(r#"{"mode":"sun","lat":51.5,"lon":-0.13}"#, "settings"),
        )
        .await
        .unwrap();
    let entry = s.history().await[0].clone();
    assert_eq!(entry["domain"], "schedule");
    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    let got: String = s.proxy.call("GetSchedule", &()).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&got).unwrap()["mode"],
        "off"
    );

    let settings = |json: String| serde_json::from_str::<serde_json::Value>(&json).unwrap();
    let dim_before =
        settings(s.proxy.call("GetSettings", &()).await.unwrap())["idle"]["dim"].clone();
    let () = s
        .proxy
        .call("SetSetting", &("idle.dim", "200", "settings"))
        .await
        .unwrap();
    let entry = s.history().await[0].clone();
    assert_eq!(entry["domain"], "settings");
    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    let got = settings(s.proxy.call("GetSettings", &()).await.unwrap());
    assert_ne!(dim_before, 200);
    assert_eq!(got["idle"]["dim"], dim_before, "{got}");
}

#[tokio::test]
async fn restore_brings_back_old_state_and_is_undoable() {
    let s = served().await;
    s.set_gaps(9, "settings").await;
    let id: String = s.proxy.call("SaveRestorePoint", &("mine",)).await.unwrap();
    s.set_gaps(12, "settings").await;

    s.restore(&id).await.unwrap();
    assert_eq!(s.gaps().await, 9);
    let points = s.points().await;
    assert!(
        points
            .iter()
            .any(|p| p["kind"] == serde_json::json!({"auto": "before_restore"})),
        "{points:?}"
    );
    let entry = s.history().await[0].clone();
    assert_eq!(entry["origin"], "restore");
    assert_eq!(entry["domain"], "look");

    s.undo(entry["id"].as_str().unwrap(), false).await.unwrap();
    assert_eq!(s.gaps().await, 12);
}

#[tokio::test]
async fn restore_with_missing_theme_changes_nothing() {
    let s = served().await;
    let id: String = s.proxy.call("SaveRestorePoint", &("old",)).await.unwrap();
    let path = s.point_file(&id);
    let mut point: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    point.insert("theme".into(), toml::Value::String("uninstalled".into()));
    std::fs::write(&path, toml::to_string(&point).unwrap()).unwrap();
    s.set_gaps(12, "settings").await;
    let points = s.points().await.len();
    let history = s.history().await.len();

    let err = s.restore(&id).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.UnknownTheme");
    assert!(
        error_message(&err).contains("isn't installed any more"),
        "{err:?}"
    );
    assert_eq!(s.gaps().await, 12);
    assert_eq!(s.points().await.len(), points);
    assert_eq!(s.history().await.len(), history);
}

#[tokio::test]
async fn restore_damaged_point_changes_nothing() {
    let s = served().await;
    let id: String = s.proxy.call("SaveRestorePoint", &("old",)).await.unwrap();
    s.set_gaps(12, "settings").await;
    let points = s.points().await.len();
    for damage in [
        "",
        "theme = \"nord\"\n[look\ngaps_in = ",
        "variant = \"purple\"\n",
        "theme = 5\n",
    ] {
        std::fs::write(s.point_file(&id), damage).unwrap();
        let err = s.restore(&id).await.unwrap_err();
        assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
        assert_eq!(error_message(&err), "restore point is damaged");
        assert_eq!(s.gaps().await, 12);
        assert_eq!(s.points().await.len(), points);
        // The damaged file is left where it is, untouched, and still listed.
        assert_eq!(std::fs::read_to_string(s.point_file(&id)).unwrap(), damage);
        assert!(s.points().await.iter().any(|p| p["id"] == id.as_str()));
    }
}

#[tokio::test]
async fn restore_point_crud() {
    let s = served().await;
    assert!(s.points().await.is_empty());
    let id: String = s.proxy.call("SaveRestorePoint", &("a",)).await.unwrap();
    let points = s.points().await;
    assert_eq!(points.len(), 1, "{points:?}");
    assert_eq!(points[0]["id"], id.as_str());
    assert_eq!(points[0]["name"], "a");
    assert_eq!(points[0]["kind"], "manual");

    let () = s.proxy.call("DeleteRestorePoint", &(&id,)).await.unwrap();
    assert!(s.points().await.is_empty());
    assert!(!s.point_file(&id).exists());

    let err = s
        .proxy
        .call::<_, _, ()>("DeleteRestorePoint", &("../x",))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    let err = s.restore(&id).await.unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
}
