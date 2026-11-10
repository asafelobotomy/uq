//! The Profiles page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::history::Entry;
use uq_core::profile::power_notice;
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};
use uq_core::theme::Variant;

const GAMING: &str =
    "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[look]\ngaps_in = 0\nblur = false\n";
const TURBO: &str = "format = 1\nid = \"turbo\"\nname = \"Turbo\"\n[look]\ngaps_in = 0\n[power]\nprofile = \"performance\"\n";

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Gaps 8, blur on, theme gruvbox (installed in the fake) in its light variant.
fn state() -> State {
    let mut st = State::default();
    st.set_section("look", json!({"gaps_in": 8, "blur": true}))
        .unwrap();
    st.theme = "gruvbox".into();
    st.variant = Variant::Light;
    st
}

async fn rig() -> (Rig, ProfilesState) {
    let r = fake_uqd::rig(state()).await;
    let page = ProfilesState::new(r.client.clone());
    (r, page)
}

fn seed(r: &Rig, text: &str) -> Profile {
    let p = Profile::parse(text).unwrap();
    r.profiles
        .store
        .lock()
        .unwrap()
        .insert(p.id.clone(), p.clone());
    p
}

fn stored(r: &Rig, id: &str) -> Option<Profile> {
    r.profiles.store.lock().unwrap().get(id).cloned()
}

fn gaps(r: &Rig) -> Option<i64> {
    r.state.lock().unwrap().section("look").unwrap()["gaps_in"].as_i64()
}

/// The `{domain}.{path}` of every key the snapshot shows for profile `i`.
fn shown_keys(state: &Value, i: usize) -> Vec<String> {
    state["profiles"][i]["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let path: Vec<&str> = k["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect();
            format!("{}.{}", k["domain"].as_str().unwrap(), path.join("."))
        })
        .collect()
}

#[tokio::test]
async fn create_from_pages_captures_current_values() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Focus Mode".into(),
            icon: "applications-office".into(),
            pages: s(&["look", "appearance"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "focus-mode").expect("saved under an id made from the name");
    assert_eq!(p.name, "Focus Mode");
    assert_eq!(p.icon.as_deref(), Some("applications-office"));
    assert_eq!(p.values["look"]["gaps_in"], 8);
    assert_eq!(p.values["look"]["blur"], true);
    assert_eq!(p.values["appearance"]["theme"], "gruvbox");
    assert_eq!(p.values["appearance"]["variant"], "light");
    assert!(!p.values.contains_key("shell"), "only the pages picked");

    let shown = &out.state["profiles"][0];
    assert_eq!(shown["id"], "focus-mode");
    assert_eq!(shown["active"], false);
    let gaps = shown["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["path"] == json!(["gaps_in"]))
        .unwrap();
    assert_eq!(gaps["label"], "Gaps between windows");
    assert_eq!(gaps["text"], "8");
    assert_eq!(out.state["active"], "");
    assert_eq!(out.state["notice"]["text"], "");

    // The same name again gets an id of its own.
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Focus Mode".into(),
            icon: String::new(),
            pages: s(&["look"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "focus-mode-2").unwrap();
    assert_eq!(p.icon, None);
}

#[tokio::test]
async fn create_from_history_takes_keys_changed_since() {
    let (r, page) = rig().await;
    r.history.seed(Entry::new(
        100,
        "settings",
        "look",
        "gaps",
        json!({"gaps_in": 5}),
        json!({"gaps_in": 8}),
    ));
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "blur",
        json!({}),
        json!({"blur": true}),
    ));
    r.history.seed(Entry::new(
        210,
        "shell",
        "settings",
        "idle",
        json!({"idle.dim": 60}),
        json!({"idle.dim": 120}),
    ));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: "weather-clear-night".into(),
            since: 150,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "evening").unwrap();
    assert_eq!(p.values.len(), 1, "{:?}", p.values);
    assert_eq!(p.values["look"], json!({"blur": true}));
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
    assert!(out.state["notice"]["seq"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn create_from_history_skips_keys_that_are_now_unset() {
    let (r, page) = rig().await;
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "rounding",
        json!({"rounding": 4}),
        json!({}),
    ));
    r.history.seed(Entry::new(
        200,
        "settings",
        "look",
        "blur",
        json!({}),
        json!({"blur": true}),
    ));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: String::new(),
            since: 0,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        stored(&r, "evening").unwrap().values["look"],
        json!({"blur": true})
    );
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
}

#[tokio::test]
async fn create_with_nothing_to_hold_is_an_error() {
    let (r, page) = rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Empty".into(),
            icon: String::new(),
            pages: s(&["displays", "history"]),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );

    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Empty".into(),
            icon: String::new(),
            since: 0,
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );
    assert!(r.profiles.store.lock().unwrap().is_empty());
    assert_eq!(out.state["profiles"], json!([]));
}

#[tokio::test]
async fn add_and_remove_keys() {
    let (r, page) = rig().await;
    seed(
        &r,
        "format = 1\nid = \"work\"\nname = \"Work\"\n[look]\ngaps_in = 2\n",
    );
    let out = run(
        &page,
        Op::AddKeys {
            id: "work".into(),
            keys: vec![
                ("look".into(), s(&["blur"])),
                ("appearance".into(), s(&["variant"])),
                // Unset now, so it can't go in.
                ("look".into(), s(&["rounding"])),
            ],
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "work").unwrap();
    assert_eq!(p.values["look"], json!({"gaps_in": 2, "blur": true}));
    assert_eq!(p.values["appearance"], json!({"variant": "light"}));
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
    assert_eq!(
        shown_keys(&out.state, 0),
        ["look.blur", "look.gaps_in", "appearance.variant"]
    );

    for (domain, path) in [("look", "gaps_in"), ("appearance", "variant")] {
        let out = run(
            &page,
            Op::RemoveKey {
                id: "work".into(),
                domain: domain.into(),
                path: s(&[path]),
            },
        )
        .await;
        assert_eq!(out.error, None);
    }
    let p = stored(&r, "work").unwrap();
    assert_eq!(p.values.len(), 1);
    assert_eq!(p.values["look"], json!({"blur": true}));

    let out = run(
        &page,
        Op::RemoveKey {
            id: "work".into(),
            domain: "look".into(),
            path: s(&["blur"]),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs at least one setting.")
    );
    assert_eq!(
        stored(&r, "work").unwrap().values["look"],
        json!({"blur": true})
    );
}

#[tokio::test]
async fn rename_keeps_the_keys() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    let out = run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: "  Play time ".into(),
            icon: "input-gaming".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "gaming").unwrap();
    assert_eq!(
        (p.name.as_str(), p.icon.as_deref()),
        ("Play time", Some("input-gaming"))
    );
    assert_eq!(p.values["look"], json!({"gaps_in": 0, "blur": false}));
    assert_eq!(out.state["profiles"][0]["name"], "Play time");

    let out = run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: " ".into(),
            icon: String::new(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("A profile needs a name of 1 to 100 characters.")
    );
}

#[tokio::test]
async fn apply_then_off_reports_kept_keys() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    let out = run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(gaps(&r), Some(0));
    assert_eq!(out.state["active"], "gaming");
    assert_eq!(out.state["profiles"][0]["active"], true);
    assert_eq!(out.state["report"]["kept"], json!([]));
    let applied_seq = out.state["report"]["seq"].as_u64().unwrap();
    assert!(applied_seq > 0);

    // Gaps changed by hand while the profile is on: kept and named by "Back to normal".
    let proxy = DesktopProxy::new(&r.client).await.unwrap();
    proxy
        .set_section("look", r#"{"gaps_in": 3, "blur": false}"#, "settings")
        .await
        .unwrap();
    let out = run(&page, Op::Off).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["report"]["kept"], json!(["Gaps between windows"]));
    assert_eq!(out.state["report"]["notices"], json!([]));
    assert!(out.state["report"]["seq"].as_u64().unwrap() > applied_seq);
    assert_eq!(out.state["active"], "");
    assert_eq!(gaps(&r), Some(3));
    assert_eq!(
        r.state.lock().unwrap().section("look").unwrap()["blur"],
        true,
        "the untouched key comes back"
    );
    let origins = r.history.origins();
    assert_eq!(origins.first().map(String::as_str), Some("profile:gaming"));
    assert_eq!(origins.last().map(String::as_str), Some("profile:gaming"));
}

#[tokio::test]
async fn apply_with_power_shows_notice() {
    let (r, page) = rig().await;
    seed(&r, TURBO);
    let out = run(&page, Op::Apply { id: "turbo".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["report"]["notices"],
        json!([power_notice(
            "power modes aren't available on this computer"
        )])
    );
    assert_eq!(out.state["profiles"][0]["power"], "Performance");
    assert_eq!(gaps(&r), Some(0), "the settings still apply");
}

#[tokio::test]
async fn export_then_import_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gaming.toml");
    let (r, page) = rig().await;
    let original = seed(&r, GAMING);
    let out = run(
        &page,
        Op::Export {
            id: "gaming".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(Profile::parse(&text).unwrap(), original);

    // Change the stored one, then bring the file back.
    run(
        &page,
        Op::Rename {
            id: "gaming".into(),
            name: "Changed".into(),
            icon: String::new(),
        },
    )
    .await;
    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None, "a conflict is asked about, not an error");
    let conflict = &out.state["conflict"];
    assert_eq!(conflict["path"], file.display().to_string());
    // The page asks the question; uqd's own "Replace it?" would ask it twice.
    assert_eq!(
        conflict["message"],
        "A profile called \u{201c}Changed\u{201d} already exists."
    );
    assert!(conflict["seq"].as_u64().unwrap() > 0);
    assert_eq!(stored(&r, "gaming").unwrap().name, "Changed");

    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert_eq!(stored(&r, "gaming").unwrap(), original);

    // A new id imports without asking.
    std::fs::write(&file, TURBO).unwrap();
    let out = run(
        &page,
        Op::Import {
            path: file,
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["conflict"], Value::Null);
    assert!(stored(&r, "turbo").is_some());
}

#[tokio::test]
async fn import_refuses_hostile_file() {
    let dir = tempfile::tempdir().unwrap();
    let (r, page) = rig().await;
    let cases: [(&str, Vec<u8>, &str); 4] = [
        (
            "big.toml",
            format!("{GAMING}# {}\n", "x".repeat(100 * 1024)).into_bytes(),
            "The file is too big to be a profile.",
        ),
        (
            "binary.toml",
            vec![0xff, 0xfe, 0x00, 0x01],
            "This file isn't a uQ profile.",
        ),
        (
            "escape.toml",
            b"format = 1\nid = \"../x\"\nname = \"X\"\n[look]\ngaps_in = 1\n".to_vec(),
            "The profile's id can only use a\u{2013}z, 0\u{2013}9 and dashes.",
        ),
        (
            "binds.toml",
            b"format = 1\nid = \"x\"\nname = \"X\"\n[keybinds]\nterminal = true\n".to_vec(),
            "This profile turns keyboard shortcuts on or off, which this version of uQ can't do yet.",
        ),
    ];
    for (name, bytes, message) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let out = run(
            &page,
            Op::Import {
                path,
                replace: true,
            },
        )
        .await;
        assert_eq!(out.error.as_deref(), Some(message), "{name}");
        assert_eq!(out.state["conflict"], Value::Null);
    }
    assert!(r.profiles.store.lock().unwrap().is_empty());
}

#[tokio::test]
async fn delete_active_is_refused_with_message() {
    let (r, page) = rig().await;
    seed(&r, GAMING);
    seed(&r, TURBO);
    run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    let out = run(
        &page,
        Op::Delete {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Turn this profile off before deleting it.")
    );
    assert!(stored(&r, "gaming").is_some());

    let out = run(&page, Op::Delete { id: "turbo".into() }).await;
    assert_eq!(out.error, None);
    assert!(stored(&r, "turbo").is_none());
    assert_eq!(out.state["profiles"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn snapshot_lists_available_keys_pages_and_icons() {
    let (_r, page) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let st = &out.state;
    assert_eq!(st["icons"], json!(ICONS));
    assert_eq!(
        st["pages"],
        json!([
            {"id": "appearance", "name": "Appearance"},
            {"id": "look", "name": "Look and layouts"},
            {"id": "wallpaper", "name": "Wallpaper"},
            {"id": "shell", "name": "Shell"},
        ])
    );
    let available = st["available"].as_array().unwrap();
    let find = |domain: &str, path: Value| {
        available
            .iter()
            .find(|k| k["domain"] == domain && k["path"] == path)
            .cloned()
    };
    assert_eq!(
        find("look", json!(["gaps_in"])),
        Some(
            json!({"domain": "look", "path": ["gaps_in"], "label": "Gaps between windows", "text": "8"})
        )
    );
    assert_eq!(
        find("appearance", json!(["theme"])).unwrap()["text"],
        "gruvbox"
    );
    assert_eq!(
        find("appearance", json!(["variant"])).unwrap()["text"],
        "light"
    );
    assert_eq!(find("look", json!(["blur"])).unwrap()["text"], "On");
    assert!(
        find("look", json!(["rounding"])).is_none(),
        "unset keys aren't offered"
    );
    assert!(available.iter().all(|k| k["domain"] != "power"));
    assert_eq!(st["conflict"], Value::Null);
    assert_eq!(st["report"], json!({"seq": 0, "kept": [], "notices": []}));
}

#[tokio::test]
async fn profiles_survive_reopening_the_model() {
    let (r, page) = rig().await;
    let every_kind = "format = 1\nid = \"all\"\nname = \"All kinds\"\nicon = \"media-playback-start\"\n\
        [look]\ngaps_in = 4\nblur = false\nanimation_speed = 1.5\n\
        [shell]\nclock_24h = false\n[shell.islands]\ntime = false\n\
        [appearance]\ntheme = \"dracula\"\nvariant = \"dark\"\nui_font = \"Inter\"\n\
        [power]\nprofile = \"power-saver\"\n";
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("all.toml");
    std::fs::write(&file, every_kind).unwrap();
    let out = run(
        &page,
        Op::Import {
            path: file,
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let out = run(
        &page,
        Op::AddKeys {
            id: "all".into(),
            keys: vec![("shell".into(), s(&["providers"]))],
        },
    )
    .await;
    assert_eq!(out.error, None);
    let before = out.state["profiles"].clone();

    let reopened = ProfilesState::new(r.client.clone());
    let out = run(&reopened, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["profiles"], before);
    let mut expected = Profile::parse(every_kind).unwrap();
    expected.with_key(uq_core::profile::Key {
        domain: "shell".into(),
        path: s(&["providers"]),
        value: json!([]),
    });
    assert_eq!(stored(&r, "all").unwrap(), expected);
    assert_eq!(
        shown_keys(&out.state, 0),
        [
            "look.animation_speed",
            "look.blur",
            "look.gaps_in",
            "shell.clock_24h",
            "shell.islands.time",
            "shell.providers",
            "appearance.theme",
            "appearance.ui_font",
            "appearance.variant",
            "power.profile",
        ]
    );
    assert_eq!(out.state["profiles"][0]["power"], "Power saver");
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = ProfilesState::new(connect(&bus).await);
    let out = run(&page, Op::Refresh).await;
    assert_eq!((out.state, out.error), (json!({}), None));
    let out = run(
        &page,
        Op::Apply {
            id: "gaming".into(),
        },
    )
    .await;
    assert_eq!((out.state, out.error), (json!({}), None));
}
