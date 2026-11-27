//! The theme editor.

use super::*;

#[tokio::test]
async fn editor_new_from_current_seeds_seven_colours() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let e = &out.state["editor"];
    assert_eq!(e["name"], "Gruvbox copy");
    assert_eq!(e["id"], "");
    assert_eq!(e["edit"], false);
    assert_eq!(e["base"], "gruvbox");
    let gruvbox = r.themes.system_theme("gruvbox").unwrap();
    let want = |t: &Theme| serde_json::to_value(derive::base_of(t)).unwrap();
    assert_eq!(e["colors"]["dark"], want(&gruvbox.dark));
    assert_eq!(e["colors"]["light"], want(&gruvbox.light));
    let keys: Vec<&str> = e["colors"]["dark"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 7);
    assert_eq!(e["colors"]["dark"]["bg"], gruvbox.dark.colors.bg.hex());

    let out = run(&page, Op::CloseEditor).await;
    assert_eq!(out.state["editor"], Value::Null);
    assert_eq!(page.preview(&json!({}))["ok"], false);
}

#[tokio::test]
async fn editor_edit_only_for_user_themes() {
    let (_r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: true,
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Only your own themes can be edited.")
    );
    assert_eq!(out.state["editor"], Value::Null);
}

#[tokio::test]
async fn editor_preview_blocks_failing_variant() {
    let (_r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    let mut d = draft(&out.state, "Mine");
    let good = page.preview(&d);
    assert_eq!(good["ok"], true, "{good}");
    assert_eq!(good["nameProblem"], "");
    assert_eq!(good["dark"]["problems"], json!([]));
    assert_eq!(good["dark"]["tokens"]["bg"], d["dark"]["bg"]);
    assert!(
        good["dark"]["tokens"]["surface_alt"]
            .as_str()
            .unwrap()
            .starts_with('#')
    );

    d["dark"]["text"] = d["dark"]["bg"].clone();
    let bad = page.preview(&d);
    assert_eq!(bad["ok"], false);
    let problems = bad["dark"]["problems"].as_array().unwrap();
    assert!(
        problems
            .iter()
            .any(|p| p.as_str().unwrap().contains("Text on the background")),
        "{problems:?}"
    );
    assert_eq!(bad["light"]["problems"], json!([]));

    d["dark"]["text"] = json!("#ebdbb2");
    d["light"]["accent"] = json!("red");
    let unread = page.preview(&d);
    assert_eq!(unread["ok"], false);
    assert_eq!(
        unread["light"]["problems"],
        json!(["Accent: Use a colour like #1e2a3b"])
    );

    d["light"]["accent"] = json!("#abc");
    assert_eq!(page.preview(&d)["light"]["tokens"]["accent"], "#aabbcc");

    let nameless = page.preview(&draft(&out.state, "  "));
    assert_eq!(nameless["ok"], false);
    assert!(nameless["nameProblem"].as_str().unwrap().contains("name"));
}

#[tokio::test]
async fn editor_save_new_gets_free_slug() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    let mut draft = draft(&out.state, "Ocean");
    draft["dark"]["accent"] = json!("#3388ff");
    let out = run(&page, Op::Save { draft }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        ids(&out.state),
        ["dracula", "gruvbox", "user-ocean", "user-ocean-2"]
    );
    assert_eq!(out.state["imported"]["id"], "user-ocean-2");
    assert_eq!(out.state["imported"]["name"], "Ocean");
    assert_eq!(out.state["editor"], Value::Null, "a saved editor closes");
    let saved = r.themes.user.lock().unwrap();
    assert_eq!(saved["user-ocean-2"].dark.colors.accent.hex(), "#3388ff");
    assert_eq!(
        derive::base_of(&saved["user-ocean-2"].light),
        derive::base_of(&r.themes.system_theme("gruvbox").unwrap().light)
    );
}

#[tokio::test]
async fn editor_edit_replaces_same_id() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "user-ocean".into(),
            edit: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["editor"]["name"], "Ocean");
    assert_eq!(out.state["editor"]["id"], "user-ocean");
    let mut draft = draft(&out.state, "Ocean");
    draft["author"] = json!("Sam");
    draft["dark"]["accent"] = json!("#ff8800");
    let out = run(&page, Op::Save { draft }).await;
    assert_eq!(out.error, None);
    assert_eq!(ids(&out.state), ["dracula", "gruvbox", "user-ocean"]);
    assert_eq!(out.state["imported"], Value::Null);
    assert!(!out.state["notice"]["text"].as_str().unwrap().is_empty());
    let saved = r.themes.user.lock().unwrap();
    assert_eq!(saved["user-ocean"].dark.colors.accent.hex(), "#ff8800");
    assert_eq!(saved["user-ocean"].author.as_deref(), Some("Sam"));
}

#[tokio::test]
async fn editor_refuses_hostile_name() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    for name in [
        "X\nshell /bin/sh -c evil",
        "X */ * { color: red } /*",
        "{{ 7*7 }}",
        "X\u{202e}",
        &"x".repeat(65),
    ] {
        let out = run(
            &page,
            Op::Save {
                draft: draft(&out.state, name),
            },
        )
        .await;
        let error = out.error.unwrap();
        assert!(error.contains("name"), "{name:?}: {error}");
        assert_ne!(out.state["editor"], Value::Null, "the editor stays open");
    }
    assert!(
        r.themes.calls().iter().all(|c| c != "ImportTheme"),
        "before any call"
    );
}

#[tokio::test]
async fn editor_save_refuses_failing_colours_and_keeps_editor() {
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    let mut d = draft(&out.state, "Mine");
    d["dark"]["text"] = d["dark"]["bg"].clone();
    let out = run(&page, Op::Save { draft: d.clone() }).await;
    assert!(out.error.unwrap().contains("Text on the background"));
    d["dark"]["text"] = json!("nope");
    let out = run(&page, Op::Save { draft: d }).await;
    assert_eq!(out.error.as_deref(), Some("Use a colour like #1e2a3b"));
    assert!(r.themes.calls().iter().all(|c| c != "ImportTheme"));
    let out = run(&page, Op::Save { draft: json!({}) }).await;
    assert!(out.error.is_some());
}

#[tokio::test]
async fn save_without_an_editor_is_refused() {
    let (_r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::Save {
            draft: json!({"name": "A"}),
        },
    )
    .await;
    assert_eq!(out.error.as_deref(), Some("There's no theme to save."));
}

#[test]
fn draft_colors_expand_short_hex_and_refuse_the_rest() {
    let side = |hex: &str| {
        json!({"bg": hex, "surface": "#112233", "text": "#fff", "accent": "#abcdef",
               "good": "#0f0", "warn": "#ff0", "bad": "#f00"})
    };
    let (dark, light) =
        draft_colors(&json!({"dark": side("#000"), "light": side("#ffffff")})).unwrap();
    assert_eq!(dark.bg.hex(), "#000000");
    assert_eq!(dark.text.hex(), "#ffffff");
    assert_eq!(light.bg.hex(), "#ffffff");
    assert_eq!(light.good.hex(), "#00ff00");
    for bad in ["red", "#12", "#12345", "1e2a3b", "#gggggg", "", "#1e2a3b4"] {
        let err = draft_colors(&json!({"dark": side(bad), "light": side("#fff")})).unwrap_err();
        assert_eq!(err, "Use a colour like #1e2a3b", "{bad:?}");
    }
    assert!(draft_colors(&json!({"dark": side("#000")})).is_err());
}

/// The theme editor's QML tests feed it a copy of a session and its preview; they must not
/// drift from what the page logic says for a new theme made from Gruvbox.
#[tokio::test]
async fn fixture_matches_the_editor_session_and_preview() {
    let (_r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    assert_eq!(
        fixture("editor"),
        out.state["editor"],
        "update tests/settings-qml/fixtures/themes.js"
    );
    assert_eq!(
        fixture("preview"),
        page.preview(&draft(&out.state, "Gruvbox copy")),
        "update tests/settings-qml/fixtures/themes.js"
    );
}

#[tokio::test]
#[ignore = "prints the fixture"]
async fn print_fixture() {
    let (_r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "gruvbox".into(),
            edit: false,
        },
    )
    .await;
    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap();
    println!("var editor = {}\n;", pretty(&out.state["editor"]));
    println!(
        "var preview = {}\n;",
        pretty(&page.preview(&draft(&out.state, "Gruvbox copy")))
    );
}
