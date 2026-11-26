use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{reapply, run};
use uq_core::state::State;

/// The value base.lua sets for `key` inside `table = {...}`, as JSON; `None` when it sets none.
fn lua_value(src: &str, table: &str, key: &str) -> Option<Value> {
    let mut inside = false;
    for line in src.lines().map(str::trim) {
        if !inside {
            inside = line.starts_with(&format!("{table} ")) && line.ends_with('{');
        } else if line.starts_with('}') {
            return None;
        } else if let Some(rest) = line.strip_prefix(key)
            && let Some(v) = rest.trim_start().strip_prefix('=')
        {
            let v = v.split("--").next().unwrap().trim().trim_end_matches(',');
            return serde_json::from_str(v).ok();
        }
    }
    None
}

#[test]
fn defaults_match_base_lua() {
    let base = include_str!("../../../../../config/hypr/base.lua");
    let d = defaults();
    // (state path, base.lua table, key): base.lua sets these, so its value is the default.
    for (section, group, field, table, key) in [
        ("look", "", "blur", "blur", "enabled"),
        ("look", "", "blur_size", "blur", "size"),
        ("look", "", "blur_passes", "blur", "passes"),
        ("look", "", "shadow", "shadow", "enabled"),
        ("look", "", "animations", "animations", "enabled"),
        ("layouts", "", "default", "general", "layout"),
        (
            "layouts",
            "dwindle",
            "preserve_split",
            "dwindle",
            "preserve_split",
        ),
        (
            "layouts",
            "scrolling",
            "fullscreen_on_one_column",
            "scrolling",
            "fullscreen_on_one_column",
        ),
    ] {
        let seed = if group.is_empty() {
            &d[section][field]
        } else {
            &d[section][group][field]
        };
        assert_eq!(
            lua_value(base, table, key).as_ref(),
            Some(seed),
            "{section}.{group}.{field} vs base.lua {table}.{key}"
        );
    }
    // base.lua leaves these to Hyprland, so the seed is Hyprland's documented default.
    for (table, key) in [
        ("dwindle", "smart_split"),
        ("dwindle", "force_split"),
        ("master", "mfact"),
        ("master", "new_status"),
        ("master", "orientation"),
        ("scrolling", "column_width"),
    ] {
        assert_eq!(
            lua_value(base, table, key),
            None,
            "base.lua now sets {table}.{key}"
        );
    }
}

async fn rig() -> (Rig, LookState) {
    let mut state = State::default();
    state
        .set_section("look", json!({"gaps_in": 4, "blur": true}))
        .unwrap();
    state
        .set_section(
            "layouts",
            json!({"default": "dwindle", "workspaces": {"2": "scrolling"},
                   "master": {"mfact": 0.6}}),
        )
        .unwrap();
    let r = fake_uqd::rig(state).await;
    let page = LookState::new(r.client.clone());
    (r, page)
}

fn look(field: &str, value: Value) -> Op {
    Op::SetLook {
        field: field.into(),
        value,
    }
}

fn option(group: &str, field: &str, value: Value) -> Op {
    Op::SetLayoutOption {
        group: group.into(),
        field: field.into(),
        value,
    }
}

#[tokio::test]
async fn set_look_keeps_other_fields() {
    let (r, page) = rig().await;
    let out = run(&page, look("gaps_out", json!(12))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["look"]["gaps_out"], 12);
    assert_eq!(out.state["look"]["gaps_in"], 4);
    assert_eq!(out.state["look"]["blur"], true);
    assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");
    assert_eq!(r.state.lock().unwrap().settings.look.gaps_out, Some(12));

    // "Use theme": null clears one override and leaves the rest.
    let out = run(&page, look("gaps_in", Value::Null)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["look"]["gaps_in"], Value::Null);
    assert_eq!(out.state["look"]["gaps_out"], 12);

    let out = run(&page, look("animation_speed", json!(1.5))).await;
    assert_eq!(out.state["look"]["animation_speed"], 1.5);
    let out = run(&page, look("rounding", json!(99))).await;
    assert!(out.error.unwrap().contains("rounding"));
    assert_eq!(out.state["look"]["gaps_out"], 12);
}

#[tokio::test]
async fn reduce_motion_is_a_setting() {
    let (r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["reduce_motion"], false);
    let out = run(&page, Op::SetReduceMotion { on: true }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["reduce_motion"], true);
    assert!(r.state.lock().unwrap().settings.ui.reduce_motion);
    assert_eq!(out.state["look"]["gaps_in"], 4, "the look is untouched");
    let out = run(&page, Op::SetReduceMotion { on: false }).await;
    assert_eq!(out.state["reduce_motion"], false);
    assert!(!r.state.lock().unwrap().settings.ui.reduce_motion);
}

#[tokio::test]
async fn set_option_in_group() {
    let (_r, page) = rig().await;
    let out = run(&page, option("dwindle", "force_split", json!("left"))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["layouts"]["dwindle"]["force_split"], "left");
    assert_eq!(
        out.state["layouts"]["master"]["mfact"], 0.6,
        "other groups stay"
    );
    assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");

    let out = run(&page, option("", "default", json!("master"))).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["layouts"]["default"], "master");
    assert_eq!(out.state["layouts"]["dwindle"]["force_split"], "left");

    let out = run(&page, option("master", "mfact", Value::Null)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["layouts"]["master"]["mfact"], Value::Null);

    let out = run(&page, option("scrolling", "column_width", json!(0.05))).await;
    assert!(out.error.unwrap().contains("column_width"));
}

#[tokio::test]
async fn workspace_add_and_remove() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::SetWorkspace {
            selector: "5".into(),
            layout: Some("master".into()),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["layouts"]["workspaces"]["5"], "master");
    assert_eq!(out.state["layouts"]["workspaces"]["2"], "scrolling");
    assert_eq!(r.state.lock().unwrap().layouts.len(), 2);

    let out = run(
        &page,
        Op::SetWorkspace {
            selector: "2".into(),
            layout: None,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["layouts"]["workspaces"], json!({"5": "master"}));
    assert_eq!(out.state["layouts"]["default"], "dwindle", "options stay");

    let out = run(
        &page,
        Op::SetWorkspace {
            selector: "6".into(),
            layout: Some("tiled".into()),
        },
    )
    .await;
    assert!(out.error.is_some());
    assert_eq!(out.state["layouts"]["workspaces"], json!({"5": "master"}));
}

#[tokio::test]
async fn rejected_verify_restores_state() {
    let (r, page) = rig().await;
    let before = run(&page, Op::Refresh).await.state;
    *r.verify_error.lock().unwrap() = Some("bad option".into());
    let out = run(&page, look("gaps_in", json!(9))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Hyprland rejected the new config: bad option")
    );
    assert_eq!(out.state, before);

    *r.verify_error.lock().unwrap() = Some("bad layout".into());
    let out = run(
        &page,
        Op::SetWorkspace {
            selector: "7".into(),
            layout: Some("master".into()),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Hyprland rejected the new config: bad layout")
    );
    assert_eq!(out.state, before);
}

#[tokio::test]
async fn look_snapshot_includes_sources() {
    let (r, page) = rig().await;
    r.explain.set("look.rounding", json!({"source": "runtime"}));
    let path = "/home/u/.config/hypr/user.lua";
    r.explain.set(
        "layouts.dwindle.smart_split",
        json!({"source": "user_lua", "detail": {"line": 3, "path": path}}),
    );
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = &out.state["sources"];
    let plain = |source: &str| json!({"source": source, "detail": null});
    // Keyed by setting id, with only `source` and `detail`.
    assert_eq!(s["gaps-inner"], plain("uq"));
    assert_eq!(s["gaps-outer"], plain("theme"));
    assert_eq!(s["blur"], plain("uq"));
    assert_eq!(s["blur-size"], plain("default"));
    assert_eq!(s["rounding"], plain("runtime"));
    assert_eq!(s["default-layout"], plain("uq"));
    // A group row shows its most telling key: user.lua over a default.
    assert_eq!(
        s["dwindle-options"],
        json!({"source": "user_lua", "detail": {"line": 3, "path": path}})
    );
    // and a value uQ sets over the defaults beside it.
    assert_eq!(s["master-options"], plain("uq"));
    assert_eq!(s["scrolling-options"], plain("default"));
    let ids: Vec<&str> = uq_core::settings::provenance::OPTIONS
        .iter()
        .filter(|r| ["look", "layouts"].contains(&r.domain()))
        .map(|r| r.setting_id)
        .collect();
    for id in &ids {
        assert!(s.get(id).is_some(), "{id} in {s}");
    }
    assert!(s.get("ui-font").is_none(), "only this page's domains: {s}");

    // A write reads the sources back too.
    let out = run(&page, look("gaps_out", json!(12))).await;
    assert_eq!(out.state["sources"]["gaps-outer"], plain("uq"));
}

#[tokio::test]
async fn look_snapshot_shows_profile_source() {
    let (r, page) = rig().await;
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    let toml = "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n\n[look]\ngaps_in = 3\n";
    proxy.set_profile(toml, false).await.unwrap();
    proxy.apply_profile("gaming").await.unwrap();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = &out.state["sources"];
    assert_eq!(
        s["gaps-inner"],
        json!({"source": "profile", "detail": {"name": "Gaming"}})
    );
    assert_eq!(
        s["gaps-outer"]["source"], "theme",
        "keys it didn't set are unchanged"
    );

    // A hand change takes the key back from the profile.
    let out = run(&page, look("gaps_in", json!(9))).await;
    assert_eq!(out.state["sources"]["gaps-inner"]["source"], "uq");
}

#[tokio::test]
async fn explain_failure_gives_empty_sources() {
    let (r, page) = rig().await;
    r.explain.fail();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None, "the page still works without sources");
    assert_eq!(out.state["sources"], json!({}));
    assert_eq!(out.state["look"]["gaps_in"], 4);
}

#[tokio::test]
async fn reapply_reruns_the_domain_and_reads_back() {
    let (r, page) = rig().await;
    let out = reapply(&page, &r.client, "layouts").await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["look"]["gaps_in"], 4);
    assert_eq!(*r.explain.reapplied.lock().unwrap(), vec!["layouts"]);
    let out = reapply(&page, &r.client, "nope").await;
    assert!(out.error.is_some(), "{out:?}");
    assert_eq!(out.state["look"]["gaps_in"], 4);
}

#[tokio::test]
async fn unknown_field_refused() {
    let (_r, page) = rig().await;
    let before = run(&page, Op::Refresh).await.state;
    for op in [
        look("nope", json!(1)),
        option("", "dwindle", json!(1)),
        option("dwindle", "mfact", json!(0.5)),
        option("tiling", "x", json!(1)),
    ] {
        let out = run(&page, op).await;
        assert!(out.error.as_deref().unwrap().contains("unknown"), "{out:?}");
        assert_eq!(out.state, before);
    }
    let out = run(&page, look("nope", json!(1))).await;
    assert_eq!(out.error.as_deref(), Some("unknown field \"nope\""));
}

/// Every field the page writes keeps its value: a fresh page object on the same uqd reads
/// back what each `Op` wrote, so no control resets itself.
#[tokio::test]
async fn every_field_round_trips() {
    let (r, page) = rig().await;
    let mut writes = vec![
        (look("gaps_in", json!(7)), "/look/gaps_in", json!(7)),
        (look("gaps_out", json!(13)), "/look/gaps_out", json!(13)),
        (
            look("border_width", json!(3)),
            "/look/border_width",
            json!(3),
        ),
        (look("rounding", json!(9)), "/look/rounding", json!(9)),
        (look("blur", json!(false)), "/look/blur", json!(false)),
        (look("blur_size", json!(5)), "/look/blur_size", json!(5)),
        (look("blur_passes", json!(2)), "/look/blur_passes", json!(2)),
        (look("shadow", json!(false)), "/look/shadow", json!(false)),
        (
            look("animations", json!(false)),
            "/look/animations",
            json!(false),
        ),
        (
            look("animation_speed", json!(1.5)),
            "/look/animation_speed",
            json!(1.5),
        ),
        (
            option("", "default", json!("master")),
            "/layouts/default",
            json!("master"),
        ),
    ];
    for (group, field, value) in [
        ("dwindle", "preserve_split", json!(false)),
        ("dwindle", "smart_split", json!(true)),
        ("dwindle", "force_split", json!("left")),
        ("master", "mfact", json!(0.7)),
        ("master", "new_status", json!("master")),
        ("master", "orientation", json!("right")),
        ("scrolling", "column_width", json!(0.7)),
        ("scrolling", "fullscreen_on_one_column", json!(false)),
    ] {
        writes.push((
            option(group, field, value.clone()),
            Box::leak(format!("/layouts/{group}/{field}").into_boxed_str()),
            value,
        ));
    }
    writes.push((
        Op::SetWorkspace {
            selector: "5".into(),
            layout: Some("master".into()),
        },
        "/layouts/workspaces/5",
        json!("master"),
    ));
    // Every writable field is covered.
    let covered = writes.len() - 2; // the default layout and the workspace rule
    assert_eq!(
        covered,
        LOOK_FIELDS.len() + GROUPS.iter().map(|(_, f)| f.len()).sum::<usize>(),
        "a field was added without a round trip"
    );

    let before = run(&page, Op::Refresh).await.state;
    for (op, pointer, want) in &writes {
        assert_ne!(
            before.pointer(pointer),
            Some(want),
            "{pointer} starts at the value"
        );
        let out = run(&page, op.clone()).await;
        assert_eq!(out.error, None, "{pointer}");
    }
    let fresh = LookState::new(r.client.clone());
    let state = run(&fresh, Op::Refresh).await.state;
    for (_, pointer, want) in &writes {
        assert_eq!(state.pointer(pointer), Some(want), "{pointer} reads back");
    }
}
