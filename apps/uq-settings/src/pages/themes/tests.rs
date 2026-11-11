//! The theme row's logic against the fake uqd.

use super::editor::draft_colors;
use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};
use uq_core::theme::derive;
use uq_core::theme::{Theme, Variant};

async fn rig() -> (Rig, ThemesState, tempfile::TempDir) {
    let mut state = State {
        theme: "gruvbox".into(),
        ..State::default()
    };
    state
        .set_section("look", json!({"gaps_in": 8, "blur": true}))
        .unwrap();
    let r = fake_uqd::rig(state).await;
    let page = ThemesState::new(r.client.clone());
    (r, page, tempfile::tempdir().unwrap())
}

/// A `.uqtheme` for slug `slug` named `name`, made from gruvbox.
fn text(r: &Rig, slug: &str, name: &str) -> String {
    r.themes
        .text_of("gruvbox")
        .unwrap()
        .replacen("id = \"gruvbox\"", &format!("id = \"{slug}\""), 1)
        .replacen("name = \"Gruvbox\"", &format!("name = \"{name}\""), 1)
}

fn file(dir: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn import(path: &Path, replace: bool, with_look: bool) -> Op {
    Op::Import {
        path: path.to_path_buf(),
        replace,
        with_look,
    }
}

use std::path::Path;

fn row<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["themes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap_or_else(|| panic!("no theme {id} in {}", state["themes"]))
}

fn ids(state: &Value) -> Vec<String> {
    state["themes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

/// The editor's own colours as a draft with `name`.
fn draft(state: &Value, name: &str) -> Value {
    json!({
        "name": name,
        "author": "",
        "dark": state["editor"]["colors"]["dark"],
        "light": state["editor"]["colors"]["light"],
    })
}

#[tokio::test]
async fn snapshot_lists_names_and_marks_user_and_active() {
    let (r, page, _d) = rig().await;
    r.themes.add(&text(&r, "ocean", "Ocean"));
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = &out.state;
    assert_eq!(ids(s), ["dracula", "gruvbox", "user-ocean"]);
    assert_eq!(row(s, "gruvbox")["name"], "Gruvbox");
    assert_eq!(row(s, "gruvbox")["active"], true);
    assert_eq!(row(s, "gruvbox")["user"], false);
    assert_eq!(row(s, "user-ocean")["name"], "Ocean");
    assert_eq!(row(s, "user-ocean")["user"], true);
    assert_eq!(row(s, "user-ocean")["active"], false);
    assert_eq!(row(s, "user-ocean")["missing"], false);
    assert_eq!(
        (&s["theme"], &s["variant"]),
        (&json!("gruvbox"), &json!("dark"))
    );
    assert_eq!(s["skipped"], json!([]));
    assert_eq!(s["rootIsLink"], false);
    assert_eq!(s["editor"], Value::Null);
    assert_eq!(s["conflict"], Value::Null);
}

#[tokio::test]
async fn import_then_use_it_now() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.error, None);
    let imported = &out.state["imported"];
    assert_eq!(
        (&imported["id"], &imported["name"]),
        (&json!("user-ocean"), &json!("Ocean"))
    );
    assert_eq!(imported["unusedLook"], false);
    assert_eq!(
        out.state["theme"], "gruvbox",
        "an import never switches themes"
    );
    let out = run(
        &page,
        Op::Use {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["theme"], "user-ocean");
    assert_eq!(out.state["variant"], "dark");
    assert_eq!(out.state["imported"], Value::Null);
    assert_eq!(
        r.history.origins().last().map(String::as_str),
        Some("settings")
    );
}

#[tokio::test]
async fn import_says_when_the_look_was_left_out() {
    let (r, page, d) = rig().await;
    let with_look = format!("{}\n[look]\ngaps_in = 3\n", text(&r, "ocean", "Ocean"));
    let path = file(&d, "ocean.uqtheme", &with_look);
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.state["imported"]["unusedLook"], true);
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["gaps_in"],
        8
    );
    let out = run(&page, import(&path, true, true)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["imported"]["unusedLook"], false);
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["gaps_in"],
        3
    );
    assert_eq!(
        r.history.origins().last().map(String::as_str),
        Some("settings")
    );
}

#[tokio::test]
async fn import_conflict_then_replace() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(&page, import(&path, false, false)).await;
    assert_eq!(out.error, None, "a conflict is a question, not an error");
    let c = &out.state["conflict"];
    let message = c["message"].as_str().unwrap();
    assert!(message.contains("Ocean"), "{message}");
    assert!(!message.contains("Replace it?"), "{message}");
    assert_eq!(c["path"], path.display().to_string());
    assert_eq!(out.state["imported"], Value::Null);

    let out = run(&page, import(&path, true, false)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert_eq!(out.state["imported"]["id"], "user-ocean");
    assert_eq!(ids(&out.state), ["dracula", "gruvbox", "user-ocean"]);
}

#[tokio::test]
async fn import_refusals_are_plain() {
    let (r, page, d) = rig().await;
    let big = file(&d, "big.uqtheme", &"#".repeat(70_000));
    let out = run(&page, import(&big, false, false)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("The file is too big to be a theme.")
    );
    assert!(r.themes.calls().is_empty(), "refused before uqd was asked");

    let hard = text(&r, "ocean", "Ocean").replacen("text = \"#ebdbb2\"", "text = \"#282828\"", 1);
    let path = file(&d, "hard.uqtheme", &hard);
    let out = run(&page, import(&path, false, false)).await;
    let error = out.error.unwrap();
    assert!(error.contains("hard to read"), "{error}");
    assert!(
        !error.contains("toml") && !error.contains("TOML") && !error.contains('{'),
        "{error}"
    );
    assert_eq!(r.themes.calls(), ["ImportTheme"]);
    assert!(!ids(&out.state).contains(&"user-ocean".to_string()));

    let out = run(
        &page,
        import(&d.path().join("missing.uqtheme"), false, false),
    )
    .await;
    assert!(out.error.unwrap().starts_with("Can't read the file"));

    *r.themes.refuse_next.lock().unwrap() = Some("Nope.".into());
    let ok = file(&d, "ok.uqtheme", &text(&r, "ocean", "Ocean"));
    let out = run(&page, import(&ok, false, false)).await;
    assert_eq!(out.error.as_deref(), Some("Nope."));
}

#[tokio::test]
async fn export_writes_the_file_and_can_include_look() {
    let (r, page, d) = rig().await;
    let path = d.path().join("out/gruvbox.uqtheme");
    let out = run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: path.clone(),
            with_look: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let parsed = uqtheme::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(parsed.name, "Gruvbox");
    assert_eq!(parsed.look, None);

    let with_look = d.path().join("look.uqtheme");
    run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: with_look.clone(),
            with_look: true,
        },
    )
    .await;
    let parsed = uqtheme::parse(&std::fs::read_to_string(&with_look).unwrap()).unwrap();
    assert_eq!(parsed.look.unwrap()["gaps_in"], 8);

    let out = run(
        &page,
        Op::Export {
            id: "nope".into(),
            path: d.path().join("x.uqtheme"),
            with_look: false,
        },
    )
    .await;
    assert!(out.error.is_some());
    assert!(!d.path().join("x.uqtheme").exists());

    // A folder can't be written as a file.
    let out = run(
        &page,
        Op::Export {
            id: "gruvbox".into(),
            path: d.path().to_path_buf(),
            with_look: false,
        },
    )
    .await;
    assert!(out.error.unwrap().starts_with("Can't save the file: "));
    drop(r);
}

#[tokio::test]
async fn delete_active_reports_switch_and_profiles() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    run(
        &page,
        Op::Use {
            id: "user-ocean".into(),
        },
    )
    .await;
    let gaming = uq_core::profile::Profile::parse(
        "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[appearance]\ntheme = \"user-ocean\"\n",
    )
    .unwrap();
    r.profiles
        .store
        .lock()
        .unwrap()
        .insert(gaming.id.clone(), gaming);

    let out = run(
        &page,
        Op::Delete {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let deleted = &out.state["deleted"];
    assert_eq!(deleted["switched"], true);
    assert_eq!(deleted["profiles"], json!(["Gaming"]));
    assert_eq!(deleted["id"], "user-ocean");
    assert_eq!(out.state["theme"], "gruvbox");
    assert_eq!(ids(&out.state), ["dracula", "gruvbox"]);

    let out = run(
        &page,
        Op::Delete {
            id: "gruvbox".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Themes that come with uQ can't be deleted.")
    );
    assert_eq!(out.state["deleted"], Value::Null);
}

#[tokio::test]
async fn delete_inactive_does_not_switch() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    run(&page, import(&path, false, false)).await;
    let out = run(
        &page,
        Op::Delete {
            id: "user-ocean".into(),
        },
    )
    .await;
    assert_eq!(out.state["deleted"]["switched"], false);
    assert_eq!(out.state["deleted"]["profiles"], json!([]));
    assert_eq!(out.state["theme"], "gruvbox");
}

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

#[tokio::test]
async fn every_op_round_trips() {
    let (r, page, d) = rig().await;
    let path = file(&d, "ocean.uqtheme", &text(&r, "ocean", "Ocean"));
    let export = d.path().join("e.uqtheme");
    let open = Op::OpenEditor {
        from: "gruvbox".into(),
        edit: false,
    };
    let state = run(&page, open.clone()).await.state;
    let ops = vec![
        import(&path, false, false),
        Op::Use {
            id: "user-ocean".into(),
        },
        Op::Export {
            id: "user-ocean".into(),
            path: export,
            with_look: false,
        },
        Op::Save {
            draft: draft(&state, "Second"),
        },
        Op::Delete {
            id: "user-ocean".into(),
        },
        Op::Delete {
            id: "user-second".into(),
        },
    ];
    for op in ops {
        let label = format!("{op:?}");
        let out = run(&page, op).await;
        if label.starts_with("Save") {
            assert_eq!(out.error, None, "{label}");
        }
        let fresh = ThemesState::new(r.client.clone());
        let again = run(&fresh, Op::Refresh).await.state;
        for key in ["themes", "theme", "variant", "skipped"] {
            assert_eq!(again[key], out.state[key], "{label}: {key} reads back");
        }
    }
    let end = run(&page, Op::Refresh).await.state;
    assert_eq!(ids(&end), ["dracula", "gruvbox"]);
    assert_eq!(end["theme"], "gruvbox");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = ThemesState::new(connect(&bus).await);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out,
        Outcome {
            state: json!({}),
            error: None
        }
    );
}

#[tokio::test]
async fn variants_keep_their_own_kind() {
    // The editor's two variants are the base's dark and light ones, never swapped.
    let (r, page, _d) = rig().await;
    let out = run(
        &page,
        Op::OpenEditor {
            from: "dracula".into(),
            edit: false,
        },
    )
    .await;
    let dracula = r.themes.system_theme("dracula").unwrap();
    assert_eq!(dracula.dark.variant, Variant::Dark);
    assert_eq!(
        out.state["editor"]["colors"]["light"]["bg"],
        dracula.light.colors.bg.hex()
    );
}

/// What the QML tests' fixture holds.
const FIXTURE: &str = include_str!("../../../../../tests/settings-qml/fixtures/themes.js");

fn fixture(name: &str) -> Value {
    let lead = format!("var {name} = ");
    let start = FIXTURE.find(&lead).expect("the fixture sets it") + lead.len();
    let rest = &FIXTURE[start..];
    let end = rest
        .find("\n;")
        .expect("the fixture ends it with a `;` line");
    serde_json::from_str(&rest[..end]).expect("the fixture is JSON")
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
