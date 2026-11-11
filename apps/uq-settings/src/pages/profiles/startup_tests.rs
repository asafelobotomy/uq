//! The Profiles page with startup apps, environment variables and app fixes (D1).

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::history::Entry;
use uq_core::state::State;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

const CODE: &str = "[Desktop Entry]\nType=Application\nName=Code Editor\nExec=code %F\n";

/// FOO=1 and BAR=2; Qt apps on Wayland; a fix for Code (X11, 150 %, with its own options); the
/// applet off; Syncthing added with a delay.
fn state() -> State {
    let mut st = State::default();
    st.set_section(
        "env",
        json!({"vars": [{"name": "FOO", "value": "1"}, {"name": "BAR", "value": "2"}]}),
    )
    .unwrap();
    st.set_section(
        "compat",
        json!({"qt_wayland": true, "apps": [{"id": "code.desktop", "platform": "x11",
            "scale": 1.5, "args": ["--x"], "elsewhere": false}]}),
    )
    .unwrap();
    st.set_section(
        "autostart",
        json!({"files": [{"id": "nm-applet.desktop", "enabled": false}],
            "added": [{"id": "a-3f9c", "name": "Syncthing", "argv": ["syncthing"],
                "enabled": true, "delay": 10}]}),
    )
    .unwrap();
    st
}

fn found(id: &str, name: &str, enabled: bool) -> Value {
    json!({"id": id, "name": name, "icon": null, "source": "system", "enabled": enabled,
        "can_toggle": true, "delay": 0, "status": if enabled { "running" } else { "off" }})
}

async fn rig() -> (Rig, ProfilesState, tempfile::TempDir) {
    let r = fake_uqd::rig(state()).await;
    r.autostart.set(vec![
        found("nm-applet.desktop", "Network", true),
        found("blueman.desktop", "Bluetooth", true),
    ]);
    let apps = tempfile::tempdir().unwrap();
    std::fs::write(apps.path().join("code.desktop"), CODE).unwrap();
    let page = ProfilesState::with_dirs(r.client.clone(), vec![apps.path().to_path_buf()]);
    (r, page, apps)
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

fn startup_of(list: &Value) -> Vec<(String, String)> {
    list.as_array()
        .unwrap()
        .iter()
        .filter(|k| startup::DOMAINS.iter().any(|d| k["domain"] == *d))
        .map(|k| {
            (
                k["label"].as_str().unwrap().to_string(),
                k["text"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn startup_keys_offered_in_add_settings() {
    let (_r, page, _apps) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let q = |n: &str| format!("\u{201c}{n}\u{201d}");
    let mut want: Vec<(String, String)> = vec![
        ("Environment variable FOO".into(), "1".into()),
        ("Environment variable BAR".into(), "2".into()),
    ];
    for (label, on) in [
        ("Sharp X11 apps", "Off"),
        ("Run Electron apps on Wayland", "Off"),
        ("Run Chromium-based browsers on Wayland", "Off"),
        ("Qt apps prefer Wayland", "On"),
        ("Steam UI scale", "Off"),
    ] {
        want.push((label.into(), on.into()));
    }
    want.extend([
        (format!("Run on for {}", q("Code Editor")), "X11".into()),
        (format!("Size for {}", q("Code Editor")), "150 %".into()),
        (format!("Start {} at login", q("Network")), "Off".into()),
        (format!("Start {} at login", q("Syncthing")), "On".into()),
        (format!("Start delay for {}", q("Syncthing")), "10 s".into()),
        (format!("Start {} at login", q("Bluetooth")), "On".into()),
    ]);
    assert_eq!(startup_of(&out.state["available"]), want);
    let text = out.state["available"].to_string();
    for word in ["argv", "args", "elsewhere", "syncthing\""] {
        assert!(!text.contains(word), "{word}: {text}");
    }
    let pages = out.state["pages"].as_array().unwrap();
    assert!(pages.contains(&json!({"id": "startup", "name": "Startup"})));
    assert!(pages.contains(&json!({"id": "apps-scaling", "name": "Apps and scaling"})));
}

#[tokio::test]
async fn create_from_startup_and_apps_pages() {
    let (r, page, _apps) = rig().await;
    let out = run(
        &page,
        Op::CreateFromPages {
            name: "Login".into(),
            icon: String::new(),
            pages: s(&["startup", "apps-scaling"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "login").unwrap();
    assert_eq!(p.values["env"], json!({"FOO": "1", "BAR": "2"}));
    assert_eq!(p.values["compat"]["qt_wayland"], true);
    assert_eq!(
        p.values["compat"]["apps"],
        json!({"code.desktop": {"platform": "x11", "scale": 1.5}})
    );
    assert_eq!(
        p.values["autostart"],
        json!({"nm-applet.desktop": {"enabled": false},
            "a-3f9c": {"enabled": true, "delay": 10},
            "blueman.desktop": {"enabled": true}})
    );
}

#[tokio::test]
async fn create_from_history_takes_startup_keys() {
    let (r, page, _apps) = rig().await;
    let st = state();
    let env = st.section("env").unwrap();
    let mut env_before = env.clone();
    env_before["vars"][0]["value"] = json!("0");
    env_before["vars"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "GONE", "value": "x"}));
    r.history
        .seed(Entry::new(200, "settings", "env", "", env_before, env));
    let compat = st.section("compat").unwrap();
    let mut compat_before = compat.clone();
    compat_before["apps"][0]["args"] = json!(["--y"]);
    r.history.seed(Entry::new(
        210,
        "settings",
        "compat",
        "",
        compat_before,
        compat,
    ));
    let out = run(
        &page,
        Op::CreateFromHistory {
            name: "Evening".into(),
            icon: String::new(),
            since: 100,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let p = stored(&r, "evening").unwrap();
    // FOO changed (its value now), GONE removed since (the profile removes it).
    assert_eq!(p.values["env"], json!({"FOO": "1", "GONE": false}));
    assert!(!p.values.contains_key("compat"), "{:?}", p.values);
    assert_eq!(
        out.state["notice"]["text"],
        "1 of those changes can't go in a profile"
    );
}

const AWAY: &str = "format = 1\nid = \"away\"\nname = \"Away\"\n\
    [autostart.\"nm-applet.desktop\"]\nenabled = true\n\
    [autostart.\"gone.desktop\"]\nenabled = false\n";

#[tokio::test]
async fn missing_startup_key_is_labelled_and_removable() {
    let (r, page, _apps) = rig().await;
    seed(&r, AWAY);
    let out = run(&page, Op::Refresh).await;
    let labels: Vec<String> = startup_of(&out.state["profiles"][0]["keys"])
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    assert_eq!(
        labels,
        [
            "A startup app that isn't on this computer",
            "Start \u{201c}Network\u{201d} at login",
        ]
    );
    let out = run(&page, Op::Apply { id: "away".into() }).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["report"]["notices"],
        json!([
            "1 startup app in this profile isn't on this computer, so it was left out.",
            "Startup apps change next time you log in."
        ])
    );
    assert_eq!(
        r.state.lock().unwrap().autostart.files[0].enabled,
        Some(true)
    );
    let out = run(
        &page,
        Op::RemoveKey {
            id: "away".into(),
            domain: "autostart".into(),
            path: s(&["gone.desktop", "enabled"]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        stored(&r, "away").unwrap().values["autostart"],
        json!({"nm-applet.desktop": {"enabled": true}})
    );
    let out = run(&page, Op::Off).await;
    assert_eq!(out.error, None);
    assert_eq!(
        r.state.lock().unwrap().autostart.files[0].enabled,
        Some(false)
    );
}

const DEV: &str = "format = 1\nid = \"dev\"\nname = \"Dev\"\n[env]\nTOKEN = \"s3cret\"\n";

#[tokio::test]
async fn export_with_env_warns_about_values() {
    let dir = tempfile::tempdir().unwrap();
    let (r, page, _apps) = rig().await;
    seed(&r, DEV);
    seed(
        &r,
        "format = 1\nid = \"plain\"\nname = \"Plain\"\n[look]\ngaps_in = 1\n",
    );
    let file = dir.path().join("plain.toml");
    let out = run(
        &page,
        Op::Export {
            id: "plain".into(),
            path: file,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "");
    let file = dir.path().join("dev.toml");
    let out = run(
        &page,
        Op::Export {
            id: "dev".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["notice"]["text"],
        "This profile includes your variables' values."
    );
    assert!(std::fs::read_to_string(&file).unwrap().contains("s3cret"));
}

#[tokio::test]
async fn import_and_export_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("all.toml");
    let (r, page, _apps) = rig().await;
    let all = "format = 1\nid = \"all\"\nname = \"All\"\n[env]\nA = \"1\"\nB = false\n\
               [compat]\nsharp_x11 = true\n[compat.apps.\"code.desktop\"]\nplatform = \"wayland\"\n\
               [autostart.\"a-3f9c\"]\ndelay = 30\n";
    let original = seed(&r, all);
    let out = run(
        &page,
        Op::Export {
            id: "all".into(),
            path: file.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    r.profiles.store.lock().unwrap().clear();
    let out = run(
        &page,
        Op::Import {
            path: file.clone(),
            replace: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(stored(&r, "all").unwrap(), original);
    for body in [
        "[autostart.\"a-1234\"]\nargv = [\"rm\"]\n",
        "[compat.apps.\"x.desktop\"]\nargs = [\"--x\"]\n",
        "[compat.apps.\"x.desktop\"]\nelsewhere = true\n",
        "[compat.apps.\"x.desktop\"]\nenv = []\n",
        "[env]\nXDG_SESSION_TYPE = \"x11\"\n",
    ] {
        let hostile = format!("format = 1\nid = \"bad\"\nname = \"Bad\"\n{body}");
        std::fs::write(&file, hostile).unwrap();
        let out = run(
            &page,
            Op::Import {
                path: file.clone(),
                replace: true,
            },
        )
        .await;
        let e = out.error.unwrap_or_default();
        assert!(
            !e.is_empty() && !e.contains(".desktop") && !e.contains('['),
            "{body}: {e}"
        );
    }
    assert!(stored(&r, "bad").is_none());
}
