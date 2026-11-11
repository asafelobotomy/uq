use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use uq_core::paths::Paths;

const FIXTURE: &str = include_str!("../../../uq-core/tests/fixtures/monitors-all.json");
const SCALE: &str = r#"{"outputs":{"DP-1":{"scale":1.5}}}"#;
const DELL: &str = r#"{"make":"Dell Inc.","model":"DELL U2723QE","serial":"ABC1234"}"#;
const BOE: &str = r#"{"make":"BOE","model":"0x0BCA","serial":""}"#;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    *comp.monitors_json.lock().unwrap() = FIXTURE.into();
    let kv = dir.path().join("kvantum");
    std::fs::create_dir_all(kv.join("KvGnome")).unwrap();
    std::fs::write(kv.join("KvGnome/KvGnome.kvconfig"), "").unwrap();
    let mut desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_kvantum_dir(kv);
    desktop.startup().await;
    comp.calls.lock().unwrap().clear();
    let events = desktop.subscribe();
    Rig {
        _dir: dir,
        desktop,
        comp,
        events,
    }
}

fn drain(rx: &mut broadcast::Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn stored(r: &Rig) -> serde_json::Value {
    serde_json::from_str(&r.desktop.get_section("monitors").unwrap()).unwrap()
}

async fn set(r: &mut Rig, json: &str) -> Result<(), DesktopError> {
    r.desktop
        .set_section("monitors", json, Origin::Settings)
        .await
}

fn refused(res: Result<(), DesktopError>) -> String {
    match res {
        Err(DesktopError::InvalidSetting(m)) => m,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn set_section_monitors_layout_still_refused() {
    let mut r = rig().await;
    let before = std::fs::read(&r.desktop.paths().state_file).unwrap();
    let msg = refused(set(&mut r, SCALE).await);
    assert_eq!(msg, "displays change through ApplyMonitors");
    assert_eq!(
        std::fs::read(&r.desktop.paths().state_file).unwrap(),
        before
    );
    assert!(r.desktop.history(100).is_empty());
}

#[tokio::test]
async fn set_section_monitors_extras_accepted() {
    let mut r = rig().await;
    let cases = [
        (
            format!(r#"{{"outputs":{{}},"primary":{DELL}}}"#),
            "Main display",
        ),
        (
            r#"{"outputs":{},"bar":{"show":"main","on":[]}}"#.to_string(),
            "Bar shown on the main display only",
        ),
        (
            r#"{"outputs":{},"workspaces":{"strategy":"interleave","count":10,"manual":[],"move_back":true}}"#
                .to_string(),
            "Workspaces across screens",
        ),
    ];
    // Each write keeps what the earlier ones set, as the page sends the whole section.
    let mut section = serde_json::json!({"outputs": {}});
    for (i, (json, summary)) in cases.iter().enumerate() {
        let patch: serde_json::Value = serde_json::from_str(json).unwrap();
        for (k, v) in patch.as_object().unwrap() {
            section[k] = v.clone();
        }
        set(&mut r, &section.to_string()).await.unwrap();
        let h = r.desktop.history(100);
        assert_eq!(h.len(), i + 1, "one entry each");
        assert_eq!(h[0].domain, "monitors");
        assert_eq!(h[0].origin, "settings");
        assert!(h[0].summary.contains(summary), "{}", h[0].summary);
    }
    // An arrangement rename.
    r.desktop
        .remember_monitors("Desk", Origin::Settings)
        .await
        .unwrap();
    let mut section = stored(&r);
    section["profiles"][0]["name"] = "Home".into();
    set(&mut r, &section.to_string()).await.unwrap();
    let h = r.desktop.history(1);
    assert!(
        h[0].summary.contains("renamed to “Home”"),
        "{}",
        h[0].summary
    );
    assert_eq!(stored(&r)["profiles"][0]["name"], "Home");
    assert!(drain(&mut r.events).contains(&Event::MonitorsChanged));
}

#[tokio::test]
async fn planner_change_rerenders() {
    let mut r = rig().await;
    let planner = r#"{"outputs":{},"workspaces":{"strategy":"sequential","count":10,"manual":[],"move_back":true}}"#;
    set(&mut r, planner).await.unwrap();
    let calls = r.comp.calls();
    assert!(calls.contains(&"reload".to_string()), "{calls:?}");
    let generated = std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap();
    assert!(generated.contains(r#"workspace = "1""#), "{generated}");
    assert!(generated.contains(r#"monitor = "DP-1""#), "{generated}");
    // A non-rendering extra changes nothing live.
    r.comp.calls.lock().unwrap().clear();
    let bar = r#"{"outputs":{},"workspaces":{"strategy":"sequential","count":10,"manual":[],"move_back":true},"bar":{"show":"main","on":[]}}"#;
    set(&mut r, bar).await.unwrap();
    assert!(!r.comp.calls().contains(&"reload".to_string()));
    // Refused while a display change is waiting.
    r.desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    let msg = refused(set(&mut r, planner).await);
    assert_eq!(msg, "keep or revert the display change first");
}

#[tokio::test]
async fn apply_keeps_stored_extras() {
    let mut r = rig().await;
    set(&mut r, &format!(r#"{{"outputs":{{}},"primary":{BOE}}}"#))
        .await
        .unwrap();
    // The page's Apply sends its whole draft; only the layout counts.
    let json = format!(r#"{{"outputs":{{"DP-1":{{"scale":1.5}}}},"primary":{DELL}}}"#);
    let (token, _) = r
        .desktop
        .apply_monitors(&json, Instant::now())
        .await
        .unwrap();
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    let s = stored(&r);
    assert_eq!(s["outputs"]["DP-1"]["scale"], 1.5);
    assert_eq!(s["primary"]["make"], "BOE", "{s}");
}

#[tokio::test]
async fn remember_is_journalled() {
    let mut r = rig().await;
    let id = r
        .desktop
        .remember_monitors("  Desk ", Origin::Settings)
        .await
        .unwrap();
    assert!(uq_core::settings::valid_profile_id(&id));
    let h = r.desktop.history(10);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].summary, "Display arrangement “Desk” saved");
    assert_eq!(h[0].origin, "settings");
    let s = stored(&r);
    assert_eq!(s["profiles"][0]["name"], "Desk");
    assert_eq!(s["profiles"][0]["outputs"].as_array().unwrap().len(), 2);
    // The same screens again: updated, same id, still one arrangement.
    let again = r
        .desktop
        .remember_monitors("", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(again, id);
    assert_eq!(stored(&r)["profiles"].as_array().unwrap().len(), 1);
    let h = r.desktop.history(10);
    assert_eq!(h.len(), 1, "an identical save adds no entry");
    // A new name on the same set renames and says so.
    r.desktop
        .remember_monitors("Home", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(stored(&r)["profiles"][0]["id"], id.as_str());
    assert!(r.desktop.history(1)[0].summary.contains("Home"));
}

#[tokio::test]
async fn remember_refused_while_waiting() {
    let mut r = rig().await;
    r.desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    let err = r.desktop.remember_monitors("Desk", Origin::Settings).await;
    assert_eq!(
        err,
        Err(DesktopError::InvalidSetting(
            "keep or revert the display change first".into()
        ))
    );
}

#[tokio::test]
async fn remember_refuses_past_32() {
    let mut r = rig().await;
    r.desktop.state.settings.monitors.profiles = (0..32)
        .map(|i| uq_core::settings::MonitorProfile {
            id: format!("m-{i:04x}"),
            name: format!("Set {i}"),
            outputs: vec![uq_core::settings::SavedOutput {
                monitor: uq_core::settings::MonitorId {
                    make: format!("Maker {i}"),
                    model: "X".into(),
                    serial: String::new(),
                },
                hint: String::new(),
                config: Default::default(),
            }],
            workspaces: None,
        })
        .collect();
    let err = r
        .desktop
        .remember_monitors("One more", Origin::Settings)
        .await;
    assert!(
        matches!(&err, Err(DesktopError::InvalidSetting(m)) if m.contains("32")),
        "{err:?}"
    );
}

#[tokio::test]
async fn remember_needs_a_running_desktop() {
    let mut r = rig().await;
    *r.comp.running.lock().unwrap() = false;
    assert!(
        r.desktop
            .remember_monitors("Desk", Origin::Settings)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn list_outputs_has_roles_and_arrangement() {
    let mut r = rig().await;
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["roles"]["primary"], "");
    assert_eq!(v["roles"]["main"], "eDP-1");
    assert!(v["roles"]["bar"].is_null());
    assert!(v["arrangement"].is_null());
    assert!(v["pending"].is_null());
    assert_eq!(v["live"].as_array().unwrap().len(), 2);

    set(
        &mut r,
        &format!(r#"{{"outputs":{{}},"primary":{DELL},"bar":{{"show":"main","on":[]}}}}"#),
    )
    .await
    .unwrap();
    r.desktop
        .remember_monitors("Desk", Origin::Settings)
        .await
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["roles"]["primary"], "DP-1");
    assert_eq!(v["roles"]["main"], "DP-1");
    assert_eq!(v["roles"]["bar"], serde_json::json!(["DP-1"]));
    let a = &v["arrangement"];
    assert_eq!(a["name"], "Desk");
    assert_eq!(a["applied"], true, "{a}");
    assert_eq!(a["failed"], false);

    // The stored layout moves away from the saved one: not applied any more.
    let (token, _) = r
        .desktop
        .apply_monitors(SCALE, Instant::now())
        .await
        .unwrap();
    r.desktop
        .confirm_monitors(&token, Origin::Settings)
        .await
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&r.desktop.list_outputs(Instant::now()).await).unwrap();
    assert_eq!(v["arrangement"]["applied"], false, "{v}");
}
