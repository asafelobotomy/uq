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

const SHOT: &str = r#"{"overrides":[],"custom":[{"name":"Screenshot","combo":"SUPER + SHIFT + P",
  "action":{"kind":"command","command":"grim -g \"$(slurp)\" ~/shot.png"}}]}"#;

fn section(r: &Rig) -> Value {
    serde_json::from_str(&r.desktop.get_section("keybinds").unwrap()).unwrap()
}

async fn write(r: &mut Rig, json: &str) -> Result<(), DesktopError> {
    r.desktop
        .set_section("keybinds", json, Origin::Settings)
        .await
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn state_toml(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().state_file).unwrap_or_default()
}

#[tokio::test]
async fn set_section_renders_verifies_and_journals() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let calls = r.comp.calls();
    let verify = calls.iter().position(|c| c.starts_with("verify")).unwrap();
    let reload = calls.iter().position(|c| c == "reload").unwrap();
    assert!(verify < reload, "{calls:?}");
    // The file Hyprland verified is the one that was installed, and it has the bind.
    let verified = r.comp.verified.lock().unwrap().clone();
    let id = section(&r)["custom"][0]["id"].as_str().unwrap().to_string();
    let line = format!("description = \"uq:{id}\"");
    assert!(generated(&r).contains(&line), "{}", generated(&r));
    assert!(
        verified
            .iter()
            .any(|(_, text)| text.contains("generated.lua"))
    );
    let h = r.desktop.history(10);
    assert_eq!(h[0].domain, "keybinds");
    assert_eq!(h[0].origin, "settings");
    assert_eq!(h[0].summary, "Shortcut “Screenshot” added");
    assert!(id.starts_with("k-"));
    // A command's text is in the file only as a Lua string, never in the history.
    assert!(!h[0].summary.contains("grim"));
}

#[tokio::test]
async fn conflicting_write_is_refused_with_both_names() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let toml = state_toml(&r);
    let verifies = r
        .comp
        .calls()
        .iter()
        .filter(|c| c.starts_with("verify"))
        .count();
    let bad = r#"{"custom":[{"name":"Mine","combo":"WIN + space",
        "action":{"kind":"uq","name":"terminal"}}]}"#;
    let err = write(&mut r, bad).await.unwrap_err();
    let DesktopError::InvalidSetting(m) = &err else {
        panic!("{err:?}")
    };
    assert!(m.contains("Mine") && m.contains("Open Spotlight"), "{m}");
    assert!(m.contains("keybinds.custom[0].combo"), "{m}");
    let after = r
        .comp
        .calls()
        .iter()
        .filter(|c| c.starts_with("verify"))
        .count();
    assert_eq!(verifies, after, "a refused write is never verified");
    assert_eq!(state_toml(&r), toml);
}

#[tokio::test]
async fn rejected_binds_keep_previous_config() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let (toml, gen_, sec) = (state_toml(&r), generated(&r), section(&r));
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:9: bad bind".into());
    let mut two = sec.clone();
    two["overrides"] = json!([{"id": "uq.spotlight", "combo": "SUPER + K"}]);
    let err = write(&mut r, &two.to_string()).await.unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(state_toml(&r), toml);
    assert_eq!(generated(&r), gen_);
    assert_eq!(section(&r), sec);
    let v = r.desktop.explain("keybinds").await.unwrap();
    assert_eq!(v["keybinds"]["source"], "error", "{v}");
    assert_eq!(v["keybinds"]["detail"]["message"], "user.lua:9: bad bind");
    assert_eq!(v["binds.uq.close"]["source"], "error", "{v}");
}

#[tokio::test]
async fn undo_and_restore_bring_binds_back() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let entry = r.desktop.history(10)[0].id.clone();
    let point = r.desktop.save_restore_point("with shortcut").unwrap();
    write(&mut r, r#"{"overrides":[],"custom":[]}"#)
        .await
        .unwrap();
    assert!(section(&r)["custom"].as_array().unwrap().is_empty());
    r.desktop.restore(&point).await.unwrap();
    assert_eq!(section(&r)["custom"].as_array().unwrap().len(), 1);
    assert!(generated(&r).contains("uq:k-"));
    // Undoing the write that added it takes it out again.
    r.desktop.undo(&entry, true).await.unwrap();
    assert!(section(&r)["custom"].as_array().unwrap().is_empty());
    assert!(!generated(&r).contains("uq:k-"));
}

#[tokio::test]
async fn undo_that_would_conflict_is_refused() {
    let mut r = rig();
    // Spotlight moves off Super + Space ...
    write(
        &mut r,
        r#"{"overrides":[{"id":"uq.spotlight","combo":"SUPER + K"}],"custom":[]}"#,
    )
    .await
    .unwrap();
    let rebind = r.desktop.history(10)[0].id.clone();
    // ... and a shortcut of your own takes Super + Space.
    write(
        &mut r,
        r#"{"overrides":[{"id":"uq.spotlight","combo":"SUPER + K"}],
            "custom":[{"name":"Mine","combo":"SUPER + Space",
              "action":{"kind":"uq","name":"terminal"}}]}"#,
    )
    .await
    .unwrap();
    let before = (state_toml(&r), generated(&r));
    // Undoing the rebind would put Spotlight back on Super + Space, where Mine is.
    let err = r.desktop.undo(&rebind, true).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Mine") && msg.contains("Open Spotlight"),
        "{err:?}"
    );
    assert!(!matches!(err, DesktopError::Conflict(_)), "{err:?}");
    assert_eq!((state_toml(&r), generated(&r)), before);
}

#[tokio::test]
async fn explain_live_and_user_lua() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    // Hyprland has everything but the close bind; user.lua unbinds Super + Q on line 3.
    let kb = r.desktop.state.keybinds.clone();
    let live: Vec<Value> = kb
        .effective()
        .iter()
        .filter(|e| e.enabled && e.id != "uq.close")
        .filter_map(|e| {
            let c = e.combo?;
            let text = uq_core::keybinds::combo::canonical(c);
            let (mods, key) = text.rsplit_once(" + ").unwrap_or(("", &text));
            let mask: u32 = mods
                .split(" + ")
                .map(|m| match m {
                    "SUPER" => 64,
                    "SHIFT" => 1,
                    "CTRL" => 4,
                    "ALT" => 8,
                    _ => 0,
                })
                .sum();
            Some(json!({"modmask": mask, "key": key, "description": format!("uq:{}", e.id)}))
        })
        .collect();
    *r.comp.binds_json.lock().unwrap() = Value::Array(live).to_string();
    let p = r.desktop.paths().user_lua.clone();
    std::fs::write(&p, "-- mine\n\nhl.unbind(\"SUPER + Q\")\n").unwrap();
    let v = r.desktop.explain("keybinds").await.unwrap();
    assert!(r.comp.calls().iter().any(|c| c == "binds"));
    let row = &v["binds.uq.close"];
    assert_eq!(row["source"], "user_lua", "{v}");
    assert_eq!(row["detail"]["line"], 3);
    assert_eq!(row["detail"]["path"], p.to_string_lossy().as_ref());
    assert_eq!(v["binds.uq.spotlight"]["source"], "default", "{v}");
    let id = section(&r)["custom"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(v[format!("binds.{id}")]["source"], "uq", "{v}");
    // Hyprland down: every row reports from state.
    *r.comp.running.lock().unwrap() = false;
    let v = r.desktop.explain("keybinds").await.unwrap();
    assert_eq!(v["binds.uq.close"]["source"], "default", "{v}");
}

#[tokio::test]
async fn list_binds_includes_marked_user_lua_rows() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let p = r.desktop.paths().user_lua.clone();
    std::fs::write(&p, "hl.bind(\"SUPER + T\", d) -- uq:bind Terminal two\n").unwrap();
    let v: Value = serde_json::from_str(&r.desktop.list_binds().unwrap()).unwrap();
    assert_eq!(v["binds"].as_array().unwrap().len(), 56);
    assert_eq!(v["user_lua"][0]["line"], 1);
    assert_eq!(v["user_lua"][0]["combo"], "SUPER + T");
    assert_eq!(v["user_lua"][0]["text"], "Terminal two");
    assert_eq!(v["binds"][55]["name"], "Screenshot");
    // No command text in the listing.
    assert!(!v.to_string().contains("grim"));
}

#[tokio::test]
async fn keybinds_not_in_settings_changed() {
    let mut r = rig();
    write(&mut r, SHOT).await.unwrap();
    let (mut seen, mut section_seen) = (false, false);
    while let Ok(e) = r.events.try_recv() {
        match e {
            Event::SettingsChanged(s) => {
                seen = true;
                let v = serde_json::to_value(&*s).unwrap();
                assert!(v.get("keybinds").is_none(), "{v}");
            }
            Event::SectionChanged { domain, .. } if domain == "keybinds" => section_seen = true,
            _ => {}
        }
    }
    assert!(seen, "SettingsChanged was sent");
    assert!(section_seen, "SectionChanged(keybinds) was sent");
}
