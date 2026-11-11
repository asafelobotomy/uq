//! The Apps and scaling page against the fake uqd.

use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::{Outcome, run};
use uq_core::compat::hypr::{PLATFORM_ENV, SwitchKey};
use uq_core::compat::valid_scale;
use uq_core::rules::WindowInfo;
use uq_core::startup_explain::{BrowserFile, OverrideState};
use uq_core::state::State;
use uq_core::test_bus::{connect, private_bus};

fn app_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("code.desktop"),
        "[Desktop Entry]\nType=Application\nName=Visual Studio Code\nIcon=vscode\n\
         Exec=/usr/bin/code --unity-launch %F\nStartupWMClass=Code\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("chromium.desktop"),
        "[Desktop Entry]\nType=Application\nName=Chromium\nIcon=chromium\nExec=chromium %U\n",
    )
    .unwrap();
    d
}

fn win(class: &str, title: &str, xwayland: bool) -> WindowInfo {
    WindowInfo {
        class: class.into(),
        title: title.into(),
        initial_class: class.into(),
        initial_title: title.into(),
        xwayland,
        workspace_id: 1,
        workspace_name: "1".into(),
        ..Default::default()
    }
}

async fn rig_with(st: State) -> (Rig, CompatState, tempfile::TempDir) {
    let r = fake_uqd::rig(st).await;
    let dir = app_dir();
    let page = CompatState::with_dirs(r.client.clone(), vec![dir.path().to_path_buf()]);
    (r, page, dir)
}

async fn rig() -> (Rig, CompatState, tempfile::TempDir) {
    rig_with(State::default()).await
}

fn section(r: &Rig) -> Value {
    r.state.lock().unwrap().section("compat").unwrap()
}

fn switch<'a>(state: &'a Value, key: &str) -> &'a Value {
    state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("no switch {key} in {state}"))
}

fn save(fix: Value) -> Op {
    Op::SaveFix { fix }
}

fn code_fix() -> Value {
    json!({"id": "code", "platform": "x11", "scale": 1.5,
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"]})
}

#[tokio::test]
async fn switch_round_trips_and_journals() {
    let (r, page, _d) = rig().await;
    for key in SwitchKey::ALL {
        let out = run(
            &page,
            Op::SetSwitch {
                key: key.key().into(),
                on: true,
            },
        )
        .await;
        assert_eq!(out.error, None, "{}", key.key());
        let row = switch(&out.state, key.key());
        assert_eq!(row["on"], true);
        assert_eq!(row["label"], key.label());
        assert_eq!(row["description"], key.description());
        assert_eq!(section(&r)[key.key()], true);
    }
    assert_eq!(r.history.origins(), ["settings"; 5]);
    let out = run(
        &page,
        Op::SetSwitch {
            key: "qt_wayland".into(),
            on: false,
        },
    )
    .await;
    assert_eq!(switch(&out.state, "qt_wayland")["on"], false);
    assert_eq!(section(&r)["qt_wayland"], false);
    assert_eq!(r.history.origins().len(), 6);
    let out = run(
        &page,
        Op::SetSwitch {
            key: "nope".into(),
            on: true,
        },
    )
    .await;
    assert!(out.error.is_some());
    assert_eq!(r.history.origins().len(), 6);
}

#[tokio::test]
async fn blurry_lists_only_xwayland_and_names_the_app() {
    let (r, page, _d) = rig().await;
    r.windows.set(vec![
        win("Code", "main.rs - Code", true),
        win("Code", "lib.rs - Code", true),
        win("firefox", "Mozilla Firefox", false),
        win("mystery-app", "Mystery", true),
    ]);
    let out = run(&page, Op::RefreshWindows).await;
    assert_eq!(out.error, None);
    let rows = out.state["blurry"]["windows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0]["class"], "Code");
    assert_eq!(rows[0]["title"], "main.rs - Code");
    assert_eq!(rows[0]["app_id"], "code.desktop");
    assert_eq!(rows[0]["app_name"], "Visual Studio Code");
    assert_eq!(rows[0]["has_fix"], false);
    assert_eq!(rows[1]["class"], "mystery-app");
    assert_eq!(rows[1]["app_id"], Value::Null);
    assert_eq!(rows[1]["app_name"], Value::Null);
    assert_eq!(rows[1]["has_fix"], false);

    run(&page, save(code_fix())).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["windows"][0]["has_fix"], true);
}

#[tokio::test]
async fn blurry_heading_states() {
    let (r, page, _d) = rig().await;
    // The fake's two outputs are scaled 1.6 and 1.5.
    let out = run(&page, Op::Refresh).await;
    let blurry = &out.state["blurry"];
    assert_eq!(blurry["scaled"], true);
    assert_eq!(blurry["sharp_on"], false);
    assert_eq!(blurry["heading"], BLURRY);

    let on = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: true,
    };
    let out = run(&page, on).await;
    assert_eq!(out.state["blurry"]["sharp_on"], true);
    assert_eq!(out.state["blurry"]["heading"], SHARP_ON);

    let off = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: false,
    };
    run(&page, off).await;
    for o in r.displays.live_outputs.lock().unwrap().iter_mut() {
        o.scale = 1.0;
    }
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["scaled"], false);
    assert_eq!(out.state["blurry"]["heading"], NOT_SCALED);
    assert_eq!(
        NOT_SCALED,
        "Your screens aren't scaled, so nothing is blurry."
    );
    assert_eq!(BLURRY, "These older apps may look blurry.");
    assert_eq!(SHARP_ON, "Older apps open now");

    // A disabled output's scale doesn't count.
    {
        let mut outputs = r.displays.live_outputs.lock().unwrap();
        outputs[0].scale = 2.0;
        outputs[0].disabled = true;
    }
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["blurry"]["scaled"], false);
}

fn entry(id: &str, wm: Option<&str>, exec: Option<&str>) -> AppEntry {
    AppEntry {
        id: id.into(),
        name: id.into(),
        icon: String::new(),
        terminal_emulator: false,
        wm_class: wm.map(String::from),
        exec_name: exec.map(String::from),
    }
}

#[test]
fn app_for_class_order() {
    let id = |apps: &[AppEntry], class: &str| app_for_class(class, apps).map(|a| a.id.clone());
    // 1. StartupWMClass, ignoring ASCII case.
    let apps = [entry("a.desktop", Some("Thing"), None)];
    assert_eq!(id(&apps, "thing").as_deref(), Some("a.desktop"));
    // 2. The id without `.desktop`.
    let apps = [entry("Foo.desktop", None, None)];
    assert_eq!(id(&apps, "foo").as_deref(), Some("Foo.desktop"));
    // 3. The id's last dot part.
    let apps = [entry("org.gnome.Nautilus.desktop", None, None)];
    assert_eq!(
        id(&apps, "nautilus").as_deref(),
        Some("org.gnome.Nautilus.desktop")
    );
    assert_eq!(
        id(&apps, "org.gnome.nautilus").as_deref(),
        Some("org.gnome.Nautilus.desktop")
    );
    // 4. The program's file name, exactly.
    let apps = [entry("x.desktop", None, Some("bar"))];
    assert_eq!(id(&apps, "bar").as_deref(), Some("x.desktop"));
    assert_eq!(id(&apps, "Bar"), None);
    assert_eq!(id(&apps, "unknown"), None);

    // An earlier rule wins over a later one, whatever the list order.
    let apps = [
        entry("code.desktop", None, None),
        entry("q.desktop", Some("code"), None),
    ];
    assert_eq!(id(&apps, "Code").as_deref(), Some("q.desktop"));
    let apps = [
        entry("org.x.Foo.desktop", None, None),
        entry("foo.desktop", None, None),
    ];
    assert_eq!(id(&apps, "foo").as_deref(), Some("foo.desktop"));
    let apps = [
        entry("t.desktop", None, Some("foo")),
        entry("org.u.Foo.desktop", None, None),
    ];
    assert_eq!(id(&apps, "foo").as_deref(), Some("org.u.Foo.desktop"));
}

#[tokio::test]
async fn save_fix_round_trips() {
    let (r, page, _d) = rig().await;
    let out = run(&page, save(code_fix())).await;
    assert_eq!(out.error, None);
    assert_eq!(
        section(&r)["apps"],
        json!([{"id": "code.desktop", "platform": "x11", "scale": 1.5,
            "env": [{"name": "FOO", "value": "1"}], "args": ["--x"], "elsewhere": false}])
    );
    let fix = &out.state["fixes"][0];
    assert_eq!(fix["id"], "code.desktop");
    assert_eq!(fix["name"], "Visual Studio Code");
    assert_eq!(fix["icon"], "vscode");
    assert_eq!(fix["platform"], "x11");
    assert_eq!(fix["scale"], 1.5);
    assert_eq!(fix["env"], json!([{"name": "FOO", "value": "1"}]));
    assert_eq!(fix["args"], json!(["--x"]));
    assert_eq!(fix["elsewhere"], false);
    assert_eq!(fix["override"], "off");
    assert_eq!(fix["message"], Value::Null);
    assert_eq!(fix["origin"]["source"], "uq");

    // A save of the same app replaces its fix.
    let out = run(
        &page,
        save(json!({"id": "code.desktop", "platform": "wayland"})),
    )
    .await;
    assert_eq!(out.error, None);
    let fixes = out.state["fixes"].as_array().unwrap();
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0]["platform"], "wayland");
    assert_eq!(fixes[0]["scale"], Value::Null);

    // What uqd's file sync found reaches the row.
    r.files
        .lock()
        .unwrap()
        .overrides
        .insert("code.desktop".into(), OverrideState::OwnFile);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["fixes"][0]["override"], "own_file");
    assert_eq!(
        out.state["fixes"][0]["message"],
        "This app's launcher file isn't uQ's, so uQ left it alone."
    );

    let out = run(
        &page,
        Op::RemoveFix {
            id: "code.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["fixes"], json!([]));
    assert_eq!(section(&r)["apps"], json!([]));
    assert_eq!(r.history.origins(), ["settings"; 3]);

    // An empty fix is refused plainly.
    let out = run(&page, save(json!({"id": "code.desktop"}))).await;
    assert_eq!(
        out.error.as_deref(),
        Some("This fix doesn't change anything.")
    );
}

#[tokio::test]
async fn fix_with_blocked_env_shows_message() {
    let (r, page, _d) = rig().await;
    let fix = json!({"id": "code.desktop",
        "env": [{"name": "WAYLAND_DISPLAY", "value": "x"}]});
    let out = run(&page, save(fix)).await;
    let error = out.error.expect("refused");
    assert_eq!(
        error,
        "uQ needs this one to run your session, so it can't be changed here"
    );
    assert!(
        !error.contains("compat.apps") && !error.contains('['),
        "{error}"
    );
    assert!(r.history.origins().is_empty());
    assert_eq!(out.state["fixes"], json!([]));
}

#[tokio::test]
async fn elsewhere_refusal_snaps_back_with_reason() {
    let (r, page, _d) = rig().await;
    run(&page, save(code_fix())).await;
    let own = "You already have your own copy of this app's file, so uQ won't replace it.";
    *r.refuse_next.lock().unwrap() = Some(own.into());
    let on = || Op::SetElsewhere {
        id: "code".into(),
        on: true,
    };
    let out = run(&page, on()).await;
    assert_eq!(out.error.as_deref(), Some(own));
    assert_eq!(out.state["fixes"][0]["elsewhere"], false);
    assert_eq!(r.history.origins().len(), 1);

    // uqd's key is dropped from its refusal (F21).
    *r.refuse_next.lock().unwrap() =
        Some("invalid value for compat.apps[0].elsewhere: Not available in a test session".into());
    let out = run(&page, on()).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Not available in a test session")
    );

    let out = run(&page, on()).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["fixes"][0]["elsewhere"], true);

    let out = run(
        &page,
        Op::SetElsewhere {
            id: "chromium.desktop".into(),
            on: true,
        },
    )
    .await;
    assert!(out.error.is_some());
}

#[tokio::test]
async fn save_fix_asks_to_reopen_open_app() {
    let (r, page, _d) = rig().await;
    r.windows.set(vec![
        win("Code", "main.rs - Code", true),
        win("firefox", "Mozilla Firefox", false),
    ]);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["reopen"],
        json!({"seq": 0, "names": [], "more": 0})
    );
    let out = run(&page, save(code_fix())).await;
    assert_eq!(
        out.state["reopen"],
        json!({"seq": 1, "names": ["main.rs - Code"], "more": 0})
    );
    // No open window: no note.
    let out = run(
        &page,
        save(json!({"id": "chromium", "platform": "wayland"})),
    )
    .await;
    assert_eq!(out.state["reopen"]["seq"], 1);
    let out = run(
        &page,
        Op::RemoveFix {
            id: "code.desktop".into(),
        },
    )
    .await;
    assert_eq!(out.state["reopen"]["seq"], 2);
    // A refused save asks nothing.
    let out = run(&page, save(json!({"id": "code"}))).await;
    assert!(out.error.is_some());
    assert_eq!(out.state["reopen"]["seq"], 2);
}

#[tokio::test]
async fn fields_come_from_the_table() {
    let f = fields();
    let switches = f["switches"].as_array().unwrap();
    assert_eq!(switches.len(), SwitchKey::ALL.len());
    for (row, key) in switches.iter().zip(SwitchKey::ALL) {
        assert_eq!(row["key"], key.key());
        assert_eq!(row["label"], key.label());
        assert_eq!(row["description"], key.description());
    }
    let platforms: Vec<&str> = f["platforms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    let mut expected = vec!["auto"];
    expected.extend(PLATFORM_ENV.iter().map(|(p, _)| *p));
    assert_eq!(platforms, expected);
    let scales: Vec<f64> = f["scales"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_f64().unwrap())
        .collect();
    assert_eq!(scales.len(), 9);
    assert_eq!(scales.first(), Some(&1.0));
    assert_eq!(scales.last(), Some(&3.0));
    assert!(scales.iter().all(|s| valid_scale(*s)));

    let (_r, page, _d) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["fields"], f);
}

#[test]
fn preview_names_env_and_args() {
    let p = preview(
        &json!({"id": "chromium", "platform": "wayland", "scale": 1.5,
        "env": [{"name": "FOO", "value": "1"}], "args": ["--x"]}),
    );
    assert_eq!(
        p,
        json!({
            "env": ["GDK_BACKEND", "QT_QPA_PLATFORM", "SDL_VIDEODRIVER",
                "ELECTRON_OZONE_PLATFORM_HINT", "GDK_SCALE", "QT_SCALE_FACTOR", "FOO"],
            "args": ["--ozone-platform=wayland", "--force-device-scale-factor=1.5", "--x"],
            "problem": null,
        })
    );
    let p = preview(&json!({"id": "code.desktop", "args": ["--y"]}));
    assert_eq!(p, json!({"env": [], "args": ["--y"], "problem": null}));
    // The user's own entry for a name the platform sets is listed once.
    let p = preview(&json!({"id": "code", "platform": "x11",
        "env": [{"name": "GDK_BACKEND", "value": "wayland"}]}));
    assert_eq!(p["env"].as_array().unwrap().len(), 4);

    let p = preview(&json!({"id": "code", "env": [{"name": "bad name", "value": "1"}]}));
    let problem = p["problem"].as_str().expect("a problem");
    assert!(
        !problem.contains("compat") && !problem.contains('['),
        "{problem}"
    );
    assert_eq!(p["env"], json!([]));
    assert_eq!(p["args"], json!([]));
    let p = preview(&json!({"id": "code"}));
    assert_eq!(p["problem"], "This fix doesn't change anything.");
}

#[tokio::test]
async fn browsers_from_explain_detail() {
    let (r, page, _d) = rig().await;
    r.files.lock().unwrap().browsers = vec![
        ("Chromium".into(), BrowserFile::Ours),
        ("Brave".into(), BrowserFile::OwnFile),
        (
            "Vivaldi".into(),
            BrowserFile::Error("Read-only file system".into()),
        ),
    ];
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        out.state["browsers"],
        json!([
            {"name": "Chromium", "file": "ours", "message": null},
            {"name": "Brave", "file": "own_file",
                "message": "Brave has its own flags file, so uQ left it alone."},
            {"name": "Vivaldi", "file": "error", "message": "Read-only file system"},
        ])
    );
}

#[tokio::test]
async fn sharp_x11_off_reads_default() {
    let (_r, page, _d) = rig().await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(
        switch(&out.state, "sharp_x11")["origin"]["source"],
        "default"
    );
    assert_eq!(
        switch(&out.state, "electron_wayland")["origin"]["source"],
        "uq"
    );
    let on = Op::SetSwitch {
        key: "sharp_x11".into(),
        on: true,
    };
    let out = run(&page, on).await;
    assert_eq!(switch(&out.state, "sharp_x11")["origin"]["source"], "uq");
    assert_eq!(out.state["section"], Value::Null);
}

#[tokio::test]
async fn uqd_down_gives_empty_state_without_an_error() {
    let bus = private_bus();
    let page = CompatState::new(connect(&bus).await);
    assert_eq!(
        run(&page, Op::Refresh).await,
        Outcome {
            state: json!({}),
            error: None
        }
    );
    assert_eq!(run(&page, Op::RefreshWindows).await.error, None);
    assert_eq!(run(&page, save(code_fix())).await.error, None);
}
