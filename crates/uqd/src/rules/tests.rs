use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::service::Event;
use crate::system::FakeSystem;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::history::Origin;
use uq_core::paths::Paths;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    let events = desktop.subscribe();
    Rig {
        _dir: dir,
        desktop,
        comp,
        events,
    }
}

const PAVU: &str = r#"{"window":[{"name":"Pavucontrol floats","enabled":true,
  "match":{"class":"^pavucontrol$"},"effects":{"float":true}}],"layer":[],"workspace":[]}"#;

fn section(r: &Rig) -> Value {
    serde_json::from_str(&r.desktop.get_section("rules").unwrap()).unwrap()
}

async fn write(r: &mut Rig, json: &str) -> Result<(), DesktopError> {
    r.desktop.set_section("rules", json, Origin::Settings).await
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn state_toml(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().state_file).unwrap_or_default()
}

fn first_id(r: &Rig) -> String {
    section(r)["window"][0]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn set_rules_section_renders_verifies_and_journals() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let calls = r.comp.calls();
    let verify = calls.iter().position(|c| c.starts_with("verify")).unwrap();
    let reload = calls.iter().position(|c| c == "reload").unwrap();
    assert!(verify < reload, "{calls:?}");
    let g = generated(&r);
    assert!(g.contains("hl.window_rule"), "{g}");
    assert!(g.contains(&format!("uq:{}", first_id(&r))), "{g}");
    let h = r.desktop.history(10);
    assert_eq!(h[0].domain, "rules");
    assert_eq!(h[0].origin, "settings");
    assert_eq!(h[0].summary, "Window rule “Pavucontrol floats” added");
}

#[tokio::test]
async fn ids_are_assigned_by_uqd() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let id = first_id(&r);
    assert!(id.starts_with("r-") && (6..=10).contains(&id.len()), "{id}");
}

#[tokio::test]
async fn resending_get_section_is_accepted() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let again = r.desktop.get_section("rules").unwrap();
    write(&mut r, &again).await.unwrap();
    assert_eq!(section(&r)["window"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn rejected_rules_keep_previous_config() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let id = first_id(&r);
    let (toml, gen_, sec) = (state_toml(&r), generated(&r), section(&r));
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:9: bad rule".into());
    let mut two: Value = sec.clone();
    two["window"].as_array_mut().unwrap().push(json!({
        "name": "Second", "enabled": true,
        "match": {"class": "^two$"}, "effects": {"float": true}}));
    let err = write(&mut r, &two.to_string()).await.unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(state_toml(&r), toml);
    assert_eq!(generated(&r), gen_);
    assert_eq!(section(&r), sec);
    let v = r.desktop.explain("rules").await.unwrap();
    assert_eq!(v[format!("window.{id}")]["source"], "error", "{v}");
    assert_eq!(v["rules"]["source"], "error", "{v}");
    assert_eq!(v["rules"]["detail"]["message"], "user.lua:9: bad rule");
}

#[tokio::test]
async fn rejected_first_rule_still_explained() {
    let mut r = rig();
    *r.comp.verify_error.lock().unwrap() = Some("nope".into());
    assert!(write(&mut r, PAVU).await.is_err());
    let v = r.desktop.explain("rules").await.unwrap();
    assert_eq!(v["rules"]["source"], "error", "{v}");
}

#[tokio::test]
async fn undo_restores_the_rule_list() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let entry = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&entry, false).await.unwrap();
    let s = section(&r);
    assert!(s["window"].as_array().unwrap().is_empty(), "{s}");
}

#[tokio::test]
async fn restore_point_brings_rules_back() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let point = r.desktop.save_restore_point("with rule").unwrap();
    write(&mut r, r#"{"window":[],"layer":[],"workspace":[]}"#)
        .await
        .unwrap();
    assert!(section(&r)["window"].as_array().unwrap().is_empty());
    r.desktop.restore(&point).await.unwrap();
    assert_eq!(section(&r)["window"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn explain_flags_user_lua_override() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let id = first_id(&r);
    let p = r.desktop.paths().user_lua.clone();
    std::fs::write(
        &p,
        format!("hl.window_rule({{ name = \"uq:{id}\", enabled = false }})\n"),
    )
    .unwrap();
    let v = r.desktop.explain("rules").await.unwrap();
    let row = &v[format!("window.{id}")];
    assert_eq!(row["source"], "user_lua", "{v}");
    assert_eq!(row["detail"]["line"], 1);
    assert_eq!(row["detail"]["path"], p.to_string_lossy().as_ref());
}

#[tokio::test]
async fn explain_workspace_rule_in_user_lua() {
    let mut r = rig();
    write(
        &mut r,
        r#"{"window":[],"layer":[],"workspace":[{"workspace":"3","persistent":true}]}"#,
    )
    .await
    .unwrap();
    let p = r.desktop.paths().user_lua.clone();
    std::fs::write(
        &p,
        "\nhl.workspace_rule({ workspace = \"3\", gaps_in = 0 })\n",
    )
    .unwrap();
    let v = r.desktop.explain("rules").await.unwrap();
    assert_eq!(v["workspace.3"]["source"], "user_lua", "{v}");
    assert_eq!(v["workspace.3"]["detail"]["line"], 2);
}

#[tokio::test]
async fn layout_and_workspace_rule_writes_stay_apart() {
    let mut r = rig();
    write(
        &mut r,
        r#"{"window":[],"layer":[],"workspace":[{"workspace":"3","persistent":true}]}"#,
    )
    .await
    .unwrap();
    r.desktop
        .set_layout("3", "master", Origin::Settings)
        .await
        .unwrap();
    let s = section(&r);
    assert_eq!(s["workspace"].as_array().unwrap().len(), 1, "{s}");
    assert_eq!(s["workspace"][0]["persistent"], true);
    let layouts: Value = serde_json::from_str(&r.desktop.get_section("layouts").unwrap()).unwrap();
    // A rules write leaves layouts alone.
    write(&mut r, PAVU).await.unwrap();
    let after: Value = serde_json::from_str(&r.desktop.get_section("layouts").unwrap()).unwrap();
    assert_eq!(layouts, after);
    // One merged call for workspace 3.
    write(
        &mut r,
        r#"{"window":[],"layer":[],"workspace":[{"workspace":"3","persistent":true}]}"#,
    )
    .await
    .unwrap();
    let g = generated(&r);
    assert_eq!(g.matches("workspace = \"3\"").count(), 1, "{g}");
    assert!(g.contains("master"), "{g}");
    assert!(g.contains("persistent"), "{g}");
}

#[tokio::test]
async fn rules_not_in_settings_changed() {
    let mut r = rig();
    write(&mut r, PAVU).await.unwrap();
    let mut seen = false;
    while let Ok(e) = r.events.try_recv() {
        if let Event::SettingsChanged(s) = e {
            seen = true;
            let v = serde_json::to_value(&*s).unwrap();
            assert!(v.get("rules").is_none(), "{v}");
        }
    }
    assert!(seen, "SettingsChanged was sent");
}

#[tokio::test]
async fn invalid_pattern_is_refused_before_render() {
    let mut r = rig();
    let bad = r#"{"window":[{"name":"x","enabled":true,"match":{"class":"("},
        "effects":{"float":true}}],"layer":[],"workspace":[]}"#;
    let err = write(&mut r, bad).await.unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
    assert!(
        r.comp.calls().iter().all(|c| !c.starts_with("verify")),
        "{:?}",
        r.comp.calls()
    );
}

#[tokio::test]
async fn list_windows_maps_clients() {
    let r = rig();
    *r.comp.clients_json.lock().unwrap() = json!([{
        "address": "0xdeadbeef", "pid": 42, "mapped": true,
        "class": "firefox", "title": "Docs", "initialClass": "firefox",
        "initialTitle": "Mozilla", "xwayland": false, "floating": true,
        "fullscreen": 0, "pinned": false,
        "workspace": {"id": 2, "name": "2"}, "tags": ["web"]
    }])
    .to_string();
    let w = r.desktop.list_windows().await.unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].class, "firefox");
    assert!(w[0].floating);
    assert_eq!(w[0].workspace_id, 2);
    assert_eq!(w[0].tags, vec!["web".to_string()]);
    let text = serde_json::to_string(&w).unwrap();
    assert!(!text.contains("deadbeef") && !text.contains("42"), "{text}");
}

#[tokio::test]
async fn list_windows_empty_when_hyprland_down() {
    let r = rig();
    *r.comp.running.lock().unwrap() = false;
    assert!(r.desktop.list_windows().await.unwrap().is_empty());
}
