//! Scripts, page hiding, fades and the window look.

use super::*;

#[test]
fn fade_switch_follows_inheritance() {
    let leaf = |name: &str, o: bool, speed: f64, bezier: &str| json!({"name": name, "overridden": o, "bezier": bezier, "enabled": true, "speed": speed, "style": ""});
    let reply = |leaves: Vec<Value>| json!([leaves, []]).to_string();
    let r = reply(vec![
        leaf("global", true, 10.0, "default"),
        leaf("fade", true, 3.03, "quick"),
        leaf("fadeSwitch", false, 0.0, ""),
    ]);
    assert_eq!(
        fade_switch(&r),
        Some((true, 3.03, "quick".into())),
        "from fade"
    );
    let r = reply(vec![
        leaf("global", true, 10.0, "default"),
        leaf("fade", false, 0.0, ""),
        leaf("fadeSwitch", true, 2.0, "linear"),
    ]);
    assert_eq!(
        fade_switch(&r),
        Some((true, 2.0, "linear".into())),
        "its own"
    );
    let r = reply(vec![leaf("global", true, 10.0, "default")]);
    assert_eq!(
        fade_switch(&r),
        Some((true, 10.0, "default".into())),
        "from global"
    );
    assert_eq!(fade_switch("nope"), None);
    let r = reply(vec![leaf("global", true, 1.0, "a\" .. os.exit()")]);
    assert_eq!(fade_switch(&r), None, "an odd bezier name isn't passed on");
    assert!(
        fade_lua(Some(&(true, 3.03, "quick".into()))).contains("speed = 3.03, bezier = \"quick\"")
    );
    assert_eq!(fade_lua(None), "uq_pages_fade = nil\n");
}

#[test]
fn page_hiding() {
    const B: &str = "0x55d0c0ffef";
    let act = Action::from_call(
        "page_hiding",
        &json!({"workspaces": [{"id": 1, "anchor": A,
                                 "windows": [{"address": A, "hide": [B]}, {"address": B, "hide": []}]}],
                "reset": [A]}),
    )
    .unwrap();
    let s = script(&act);
    assert!(s.contains(&format!("reset = {{ \"{A}\" }}")), "{s}");
    assert!(s.contains(&format!("[\"{A}\"] = {{ \"{B}\" }}")), "{s}");
    assert!(s.contains("hl.on(\"window.active\""), "{s}");
    assert!(
        s.contains("\"opacity_inactive\"") && s.contains("\"no_anim\""),
        "{s}"
    );
    for bad in [
        json!({"workspaces": [{"id": 1, "anchor": A, "windows": []}]}),
        json!({"workspaces": [{"id": 1, "anchor": "nope", "windows": [{"address": A, "hide": []}]}]}),
        json!({"workspaces": [{"id": 1, "anchor": A, "windows": [{"address": A, "hide": ["x"]}]}]}),
        json!({"workspaces": "x"}),
    ] {
        assert!(Action::from_call("page_hiding", &bad).is_err(), "{bad}");
    }
    assert!(
        Action::from_call("page_hiding", &json!({"workspaces": []})).is_ok(),
        "nothing to hide"
    );
}

#[test]
fn one_script_per_action() {
    let w = r#""address:0x55d0c0ffee""#;
    let plain = Action::from_call("close", &json!({"address": A})).unwrap();
    let s = script(&plain);
    assert!(s.contains(&format!(
        "do local m = fail(hl.dispatch(hl.dsp.window.close({{ window = {w} }}))); if m then return m end end"
    )));
    assert!(!s.contains("hl.get_config"), "no pointer keeping");
    assert!(!s.contains("get_cursor_pos"));
    // Steps keep their order.
    let s = script(&Action::from_call("minimise", &json!({"address": A, "from_ws": 4})).unwrap());
    assert!(s.find("uq_ws_4").unwrap() < s.find("special:minimized").unwrap());
    // Restore ends by focusing the window, with the pointer kept still: the column policy
    // would otherwise focus it to size it and give focus back to the window before.
    let s = script(&Action::from_call("restore", &json!({"address": A, "to_ws": 4})).unwrap());
    assert!(s.find("uq_ws_4").unwrap() < s.find("hl.dsp.focus").unwrap());
    assert!(s.contains("no_warps = true"), "the pointer stays put");
    // With `after`, the last window on the workspace is focused first, so the restored
    // window lands behind it.
    let a2 = "0x55d0c0ffe2";
    let s = script(
        &Action::from_call("restore", &json!({"address": A, "to_ws": 4, "after": a2})).unwrap(),
    );
    assert!(s.find(a2).unwrap() < s.find("uq_ws_4").unwrap());
    assert!(
        Action::from_call("restore", &json!({"address": A, "to_ws": 4, "after": "x"})).is_err()
    );
    // keep_pointer: no_warps switched on, and back after the steps whatever they did.
    let s = script(
        &Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true})).unwrap(),
    );
    let on = s.find("no_warps = true").unwrap();
    let run = s.find("pcall(run)").unwrap();
    let off = s.find("no_warps = false").unwrap();
    let raise = s.find("if not ran then").unwrap();
    assert!(on < run && run < off && off < raise);
    assert!(s.contains("hl.get_cursor_pos") && s.contains("hl.dsp.cursor.move"));
    // ColumnWidth: every fullscreen state gets `column_lua`'s steps, then the refocus.
    let s = script(
        &Action::from_call(
            "column_width",
            &json!({"address": A, "width": "half", "keep_focus": true}),
        )
        .unwrap(),
    );
    assert!(s.contains("if fs == 1 then") && s.contains("elseif fs == 2 then"));
    assert_eq!(s.matches("colresize 0.5").count(), 2, "fs 1 and fs 0");
    assert!(s.contains(r#"w.address ~= "0x55d0c0ffee""#));
}

#[test]
fn rejected_step_reads_as_hyprland_reply() {
    let step = CompositorError::Rejected(
        "error: uq-step:warning: =[C]:-1: hl.focus: window not found".into(),
    );
    assert_eq!(
        script_err(step),
        "warning: =[C]:-1: hl.focus: window not found"
    );
    let lua = CompositorError::Rejected("error: [string \"x\"]:1: boom".into());
    assert_eq!(script_err(lua), "error: [string \"x\"]:1: boom");
    assert_eq!(
        script_err(CompositorError::NotRunning),
        "Hyprland is not running"
    );
}

/// Against the live compositor, on a window that doesn't exist (nothing real is touched):
/// the reply is what the per-statement `eval` gave, and `cursor:no_warps` is as before.
#[tokio::test]
#[ignore = "needs a running Hyprland session"]
async fn script_against_live_hyprland() {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let hypr = HyprSocket::from_env();
    let no_warps = || async {
        let r = hypr.request("j/getoption cursor:no_warps").await.unwrap();
        serde_json::from_str::<Value>(&r).unwrap()["bool"].clone()
    };
    let before = no_warps().await;
    let gone = "0x1";
    let single = hypr
        .eval(&format!(
            "return hl.dispatch(hl.dsp.focus({{ window = {} }}))",
            lua_string(&format!("address:{gone}"))
        ))
        .await
        .map_err(eval_err)
        .unwrap_err();
    for keep_pointer in [false, true] {
        let a = Action::from_call(
            "focus_window",
            &json!({"address": gone, "keep_pointer": keep_pointer}),
        )
        .unwrap();
        assert_eq!(run(&hypr, &a).await.unwrap_err(), single);
        assert_eq!(no_warps().await, before);
    }
}

#[test]
fn look_options() {
    assert_eq!(
        parse_look_option(r#"{"option": "general:border_size", "int": 4, "set": true}"#),
        Some(json!(4))
    );
    assert_eq!(
        parse_look_option(r#"{"option": "general:gaps_out", "css": "4 4 4 4", "set": true}"#),
        Some(json!([4, 4, 4, 4]))
    );
    assert_eq!(
        parse_look_option(r#"{"css": "10 20"}"#),
        Some(json!([10, 20, 10, 20]))
    );
    assert_eq!(
        parse_look_option(r#"{"gradient": "aa625e51 0deg"}"#),
        Some(json!("#aa625e51"))
    );
    assert_eq!(
        parse_look_option(r#"{"gradient": "ffD79921 ff000000 45deg"}"#),
        Some(json!("#ffd79921"))
    );
    assert_eq!(parse_look_option(r#"{"gradient": "red"}"#), None);
    assert_eq!(
        parse_look_option(r#"{"option": "general:layout", "str": "scrolling"}"#),
        Some(json!("scrolling"))
    );
    assert_eq!(parse_look_option(r#"{"str": "a b"}"#), None);
    assert_eq!(parse_look_option(r#"{"css": "a b"}"#), None);
    assert_eq!(parse_look_option("not json"), None);
}

#[tokio::test]
async fn window_look_in_one_batch_or_one_by_one() {
    let want = json!({"border": 2, "gaps_out": [4, 8, 4, 8], "layout": "scrolling"});
    let (_d, h, seen) = fake_hypr(batch_reply);
    assert_eq!(window_look_from(&h).await.unwrap(), want);
    assert_eq!(seen.lock().unwrap().len(), 1, "one connection");

    let (_d, h, seen) = fake_hypr(single_reply);
    assert_eq!(window_look_from(&h).await.unwrap(), want);
    assert_eq!(seen.lock().unwrap().len(), 1 + LOOK_OPTIONS.len());

    let gone = HyprSocket::new(Some("/nonexistent".into()), Some("sig".into()));
    assert_eq!(
        window_look_from(&gone).await.unwrap_err(),
        "Hyprland is not running"
    );
    assert_eq!(split_batch("a\n\n\nb", 2), Some(vec!["a", "b"]));
    assert_eq!(split_batch("a\n\n\nb", 3), None);
}

/// Every generated statement must load under Hyprland's Lua. CI runs this with `--ignored`.
#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn dispatch_lua_parses_under_hyprland() {
    let all = [
        ("focus_workspace", json!({"id": 3})),
        ("focus_window", json!({"address": A})),
        ("close", json!({"address": A})),
        ("toggle_float", json!({"address": A})),
        ("pin", json!({"address": A})),
        ("fullscreen", json!({"address": A, "mode": "full"})),
        ("fullscreen", json!({"address": A, "mode": "maximized"})),
        ("move", json!({"address": A, "workspace": 2})),
        ("move", json!({"address": A, "workspace": "scratch"})),
        ("minimise", json!({"address": A, "from_ws": 4})),
        ("restore", json!({"address": A, "to_ws": 4})),
        ("dpms", json!({"on": true})),
        ("dpms", json!({"on": false})),
        ("column_width", json!({"address": A, "width": "full"})),
        (
            "resize",
            json!({"windows": [{"address": A, "w": 600, "h": 400}]}),
        ),
        (
            "columns",
            json!({"windows": [{"address": A, "fraction": 0.5}], "pin": {"address": A, "x": 16}}),
        ),
        ("move_column", json!({"address": A, "steps": -1})),
        ("own_column", json!({"address": A})),
        ("scroll", json!({"address": A, "x": 16})),
        (
            "page_hiding",
            json!({"workspaces": [{"id": 1, "anchor": A, "windows": [{"address": A, "hide": [A]}]}],
                   "reset": [A]}),
        ),
        ("column_width", json!({"address": A, "width": "half"})),
        (
            "place",
            json!({"address": A, "x": -20, "y": 72, "w": 800, "h": 600}),
        ),
        (
            "dock",
            json!({"address": A, "x": 320, "y": 230, "w": 640, "h": 220}),
        ),
    ];
    let mut body = String::new();
    let mut actions: Vec<Action> = all
        .iter()
        .map(|(name, args)| Action::from_call(name, args).unwrap())
        .collect();
    actions.push(
        Action::from_call("focus_window", &json!({"address": A, "keep_pointer": true})).unwrap(),
    );
    actions.push(
        Action::from_call(
            "column_width",
            &json!({"address": A, "width": "half", "keep_focus": true}),
        )
        .unwrap(),
    );
    for a in &actions {
        body.push_str(&format!(
            "hl.on(\"hyprland.start\", function()\n{}end)\n",
            script(a)
        ));
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("user.lua");
    std::fs::write(&file, body).unwrap();
    let out = std::process::Command::new("Hyprland")
        .arg("--verify-config")
        .arg("-c")
        .arg(&file)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
}
