//! Verifying candidates, startup and a missing compositor.

use super::*;

#[tokio::test]
async fn verify_main_lives_in_hypr_dir_with_relative_requires() {
    let mut r = rig();
    r.desktop
        .set_layout("1", "master", Origin::Settings)
        .await
        .unwrap();
    let (path, src) = r.comp.verified.lock().unwrap()[0].clone();
    assert_eq!(path.parent(), Some(r.desktop.paths().hypr_dir.as_path()));
    assert!(
        src.contains(r#"require("./uq/generated.lua.candidate")"#),
        "{src}"
    );
    assert!(src.contains(r#"require("./uq/user.lua")"#), "{src}");
}

#[tokio::test]
async fn failed_verify_keeps_last_good_and_reports() {
    let mut r = rig();
    r.desktop
        .set_theme("forest", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    let before = fs::read(&p.generated_lua).unwrap();
    let tokens_before = fs::read(p.theme_dir.join("tokens.json")).unwrap();
    drain(&mut r.events);
    *r.comp.verify_error.lock().unwrap() = Some("user.lua:3: syntax error".into());

    let err = r
        .desktop
        .set_theme("velvet", Variant::Light, Origin::Settings)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, DesktopError::ConfigInvalid(m) if m == "user.lua:3: syntax error"),
        "{err:?}"
    );
    assert_eq!(fs::read(&p.generated_lua).unwrap(), before);
    assert_eq!(
        fs::read(p.theme_dir.join("tokens.json")).unwrap(),
        tokens_before
    );
    assert_eq!(State::load(&p.state_file).theme, "forest");
    assert_eq!(r.desktop.get_theme().0, "forest");
    assert_eq!(
        drain(&mut r.events),
        vec![Event::ConfigError("user.lua:3: syntax error".into())]
    );
    let leftovers: Vec<_> = fs::read_dir(p.generated_lua.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let mut leftovers = leftovers;
    leftovers.sort();
    assert_eq!(leftovers, vec!["generated.lua", "user.lua"]);
}

#[tokio::test]
async fn verify_sees_candidate_and_user_files() {
    let mut r = rig();
    r.desktop
        .set_layout("1", "master", Origin::Settings)
        .await
        .unwrap();
    let verify = r
        .comp
        .calls()
        .into_iter()
        .find(|c| c.starts_with("verify "))
        .unwrap();
    let main = PathBuf::from(verify.trim_start_matches("verify "));
    assert!(!main.exists(), "temporary main config left behind");
}

#[tokio::test]
async fn compositor_down_still_persists() {
    let mut r = rig_with(false, |_| {});
    r.desktop
        .set_layout("web", "dwindle", Origin::Settings)
        .await
        .unwrap();
    r.desktop
        .set_theme("matcha", Variant::Light, Origin::Settings)
        .await
        .unwrap();
    let p = r.desktop.paths().clone();
    let s = State::load(&p.state_file);
    assert_eq!(
        (s.theme.as_str(), s.layouts.get("name:web")),
        ("matcha", Some(&Layout::Dwindle))
    );
    assert!(p.theme_dir.join("tokens.json").exists());
}

#[tokio::test]
async fn startup_creates_user_lua_and_renders() {
    let mut r = rig();
    r.desktop.startup().await;
    let p = r.desktop.paths().clone();
    assert!(p.user_lua.exists());
    assert!(p.generated_lua.exists());
    assert!(
        fs::read_to_string(&p.generated_lua)
            .unwrap()
            .contains("Graphite (dark)")
    );
}

#[tokio::test]
async fn startup_always_leaves_the_capture_submap() {
    let mut r = rig();
    r.desktop.startup().await;
    let calls = r.comp.calls();
    let at = calls
        .iter()
        .position(|c| c == "eval hl.dispatch(hl.dsp.submap(\"reset\"))");
    assert_eq!(at, Some(0), "{calls:?}");
}

#[tokio::test]
async fn startup_never_overwrites_user_lua() {
    let mut r = rig_with(true, |p| {
        fs::create_dir_all(p.user_lua.parent().unwrap()).unwrap();
        fs::write(&p.user_lua, "hl.config({ general = { gaps_in = 0 } })\n").unwrap();
    });
    r.desktop.startup().await;
    assert_eq!(
        fs::read_to_string(&r.desktop.paths().user_lua).unwrap(),
        "hl.config({ general = { gaps_in = 0 } })\n"
    );
}

#[tokio::test]
async fn startup_saves_new_version_point_once() {
    let mut r = rig();
    r.desktop.startup().await;
    r.desktop.startup().await;
    let points: Vec<_> = r
        .desktop
        .restore
        .list()
        .into_iter()
        .filter(|p| p.kind == PointKind::Auto(AutoReason::NewVersion))
        .collect();
    assert_eq!(points.len(), 1, "{points:?}");
    assert_eq!(points[0].name, env!("CARGO_PKG_VERSION"));
    assert!(journal_of(&r).is_empty());
}
