//! Rules from apps (`integration_rules`, Phase 4 spec §3.3) in uqd.

use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::PathBuf;
use std::sync::Arc;
use uq_core::history::Origin;
use uq_core::paths::Paths;

const ROMFORGE: &str = r#"
[[window_rule]]
id = "romforge.emulators"
name = "Emulators open full screen on the game workspace"
class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
workspace = "name:game"
fullscreen = true
"#;

struct Rig {
    _dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    Rig {
        _dir: dir,
        desktop,
        comp,
    }
}

fn install(r: &Rig, name: &str, text: &str) -> PathBuf {
    let dir = r.desktop.paths().integrations_dir.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("rules.toml");
    std::fs::write(&file, text).unwrap();
    file
}

async fn turn_on(r: &mut Rig, ids: &[&str]) -> Result<(), DesktopError> {
    let v = json!({ "enabled": ids });
    r.desktop
        .set_section("integration_rules", &v.to_string(), Origin::Settings)
        .await
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn enabled(r: &Rig) -> Value {
    serde_json::from_str::<Value>(&r.desktop.get_section("integration_rules").unwrap()).unwrap()
        ["enabled"]
        .clone()
}

#[tokio::test]
async fn off_by_default_and_listed() {
    let r = rig();
    install(&r, "romforge", ROMFORGE);
    install(
        &r,
        "broken",
        "[[window_rule]]\nid = \"broken.x\"\nclass = [\"(\"]\nfloat = true\n",
    );
    assert_eq!(enabled(&r), json!([]));
    let list = r.desktop.list_integration_rules();
    assert_eq!(list["rules"].as_array().unwrap().len(), 1, "{list}");
    let rule = &list["rules"][0];
    assert_eq!(rule["id"], "romforge.emulators");
    assert_eq!(rule["integration"], "romforge");
    assert_eq!(rule["enabled"], false);
    assert_eq!(list["problems"][0]["integration"], "broken");
    assert!(
        list["problems"][0]["error"]
            .as_str()
            .unwrap()
            .contains("pattern"),
        "{list}"
    );
    assert_eq!(list["missing"], json!([]));
}

#[tokio::test]
async fn turning_on_renders_verifies_and_journals() {
    let mut r = rig();
    install(&r, "romforge", ROMFORGE);
    turn_on(&mut r, &["romforge.emulators"]).await.unwrap();
    let calls = r.comp.calls();
    let verify = calls.iter().position(|c| c.starts_with("verify")).unwrap();
    let reload = calls.iter().position(|c| c == "reload").unwrap();
    assert!(verify < reload, "{calls:?}");
    let g = generated(&r);
    assert!(g.contains("\n-- Rules from apps\n"), "{g}");
    assert!(
        g.contains(
            r#"hl.window_rule({ name = "uq:app:romforge.emulators", enabled = true, match = { class = "^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$" }, fullscreen = true, workspace = "name:game" })"#
        ),
        "{g}"
    );
    assert!(
        !g.contains("Emulators open"),
        "the name never reaches the file"
    );
    assert_eq!(enabled(&r), json!(["romforge.emulators"]));
    assert_eq!(
        r.desktop.list_integration_rules()["rules"][0]["enabled"],
        true
    );
    let h = r.desktop.history(10);
    assert_eq!(h[0].domain, "integration_rules");
    assert_eq!(h[0].origin, "settings");
    assert_eq!(
        h[0].summary,
        "Rule from an app turned on: romforge.emulators"
    );
    // Undo turns it off again, and the rule leaves the config.
    let id = h[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert_eq!(enabled(&r), json!([]));
    assert!(!generated(&r).contains("Rules from apps"));
}

#[tokio::test]
async fn invalid_or_missing_rules_are_refused_and_stay_off() {
    let mut r = rig();
    install(
        &r,
        "romforge",
        &ROMFORGE.replace("name:game", "name:no/slash"),
    );
    let err = turn_on(&mut r, &["romforge.emulators"]).await.unwrap_err();
    let DesktopError::InvalidSetting(msg) = &err else {
        panic!("{err:?}");
    };
    assert!(
        msg.contains("from romforge can't be used") && msg.contains("workspace"),
        "{msg}"
    );
    let err = turn_on(&mut r, &["hauler.nothing"]).await.unwrap_err();
    assert!(err.to_string().contains("isn't installed"), "{err}");
    let err = turn_on(&mut r, &["not an id"]).await.unwrap_err();
    assert!(matches!(err, DesktopError::InvalidSetting(_)), "{err:?}");
    assert_eq!(enabled(&r), json!([]));
    assert!(r.desktop.history(10).is_empty());
    assert!(!generated(&r).contains("Rules from apps"));
}

#[tokio::test]
async fn hyprland_refusal_keeps_it_off() {
    let mut r = rig();
    install(&r, "romforge", ROMFORGE);
    let before = generated(&r);
    *r.comp.verify_error.lock().unwrap() = Some("generated.lua:80: bad rule".into());
    let err = turn_on(&mut r, &["romforge.emulators"]).await.unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(enabled(&r), json!([]));
    assert_eq!(generated(&r), before);
    assert_eq!(
        r.desktop.list_integration_rules()["rules"][0]["enabled"],
        false
    );
}

#[tokio::test]
async fn a_removed_file_drops_the_rule_and_keeps_the_id() {
    let mut r = rig();
    let file = install(&r, "romforge", ROMFORGE);
    turn_on(&mut r, &["romforge.emulators"]).await.unwrap();
    assert!(generated(&r).contains("uq:app:romforge.emulators"));
    std::fs::remove_file(&file).unwrap();
    // The next render (any section) leaves it out; the id stays on for a reinstall.
    r.desktop.reapply("look").await.unwrap();
    assert!(!generated(&r).contains("romforge"), "{}", generated(&r));
    assert_eq!(enabled(&r), json!(["romforge.emulators"]));
    let list = r.desktop.list_integration_rules();
    assert_eq!(list["missing"], json!(["romforge.emulators"]));
    let v = r.desktop.explain("integration_rules").await.unwrap();
    assert_eq!(v["romforge.emulators"]["detail"]["installed"], false, "{v}");
    // A write that resends the missing id is accepted (it was already on).
    turn_on(&mut r, &["romforge.emulators"]).await.unwrap();
    // Back again: rendered with no new write.
    install(&r, "romforge", ROMFORGE);
    r.desktop.reapply("look").await.unwrap();
    assert!(generated(&r).contains("uq:app:romforge.emulators"));
    // A file that turned bad is dropped the same way.
    install(
        &r,
        "romforge",
        "[[window_rule]]\nid = \"romforge.emulators\"\nclass = []\n",
    );
    r.desktop.reapply("look").await.unwrap();
    assert!(!generated(&r).contains("romforge"));
    let v = r.desktop.explain("integration_rules").await.unwrap();
    assert_eq!(v["romforge.emulators"]["source"], "uq", "{v}");
}
