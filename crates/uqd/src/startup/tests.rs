use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::service::Event;
use crate::system::FakeSystem;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::history::Origin;
use uq_core::paths::Paths;
use uq_core::settings::OutputConfig;

const CODE: &str = "code.desktop";
const SOURCE: &str = "[Desktop Entry]\nType=Application\nName=Code\nExec=/usr/bin/code %F\n";
const ELSEWHERE: &str = r#"{"apps":[{"id":"code.desktop","platform":"wayland","elsewhere":true}]}"#;

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn rig_dev(dev: bool) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let desktop = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default()))
        .unwrap()
        .with_dev_session(dev);
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        events,
    }
}

fn rig() -> Rig {
    rig_dev(false)
}

fn root(r: &Rig) -> &Path {
    r.dir.path()
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

async fn write(r: &mut Rig, domain: &str, json: &str) -> Result<(), DesktopError> {
    r.desktop.set_section(domain, json, Origin::Settings).await
}

fn generated(r: &Rig) -> String {
    std::fs::read_to_string(&r.desktop.paths().generated_lua).unwrap_or_default()
}

fn env_file(r: &Rig) -> Option<String> {
    std::fs::read_to_string(files::environment_d_path(&r.desktop.paths().startup)).ok()
}

fn state_toml(r: &Rig) -> Option<String> {
    std::fs::read_to_string(&r.desktop.paths().state_file).ok()
}

fn override_path(r: &Rig) -> PathBuf {
    files::override_target(&r.desktop.paths().startup, CODE).unwrap()
}

fn put_source(r: &Rig) {
    put(&root(r).join("share-dirs/applications").join(CODE), SOURCE);
}

fn dp1(r: &Rig, scale: f64) {
    *r.comp.monitors_json.lock().unwrap() =
        format!(r#"[{{"name":"DP-1","x":0,"y":0,"scale":{scale}}}]"#);
}

async fn explain(r: &Rig, domain: &str) -> Value {
    r.desktop.explain(domain).await.unwrap()
}

#[tokio::test]
async fn env_section_renders_hl_env_and_journals() {
    let mut r = rig();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#)
        .await
        .unwrap();
    assert!(r.comp.calls().iter().any(|c| c.starts_with("verify")));
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("FOO", "bar", true)"#), "{g}");
    let h = r.desktop.history(10);
    assert_eq!(h[0].domain, "env");
    assert_eq!(h[0].origin, "settings");
    assert!(h[0].summary.contains("FOO"), "{}", h[0].summary);
    let f = env_file(&r).unwrap();
    assert!(f.contains("FOO='bar'"), "{f}");
}

#[tokio::test]
async fn env_dev_session_does_not_export() {
    let mut r = rig_dev(true);
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"bar"}]}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("FOO", "bar", false)"#), "{g}");
}

#[tokio::test]
async fn compat_section_renders_zero_scaling_with_main_scale() {
    let mut r = rig();
    dp1(&r, 1.5);
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains("force_zero_scaling = true"), "{g}");
    assert!(g.contains(r#"hl.env("GDK_SCALE", "1", true)"#), "{g}");
    assert!(
        g.contains(r#"hl.env("STEAM_FORCE_DESKTOPUI_SCALING", "1.5", true)"#),
        "{g}"
    );
    assert_eq!(r.desktop.main_scale, Some(1.5));
    let child = r.desktop.child_env();
    assert_eq!(child[0].0, "XCURSOR_THEME");
    assert!(child.contains(&("GDK_SCALE".to_string(), "1".to_string())));
    // Scale lines never reach environment.d (F18).
    assert!(env_file(&r).is_none_or(|f| !f.contains("GDK_SCALE")));
}

#[tokio::test]
async fn stored_monitor_scale_wins() {
    let mut r = rig();
    dp1(&r, 1.5);
    r.desktop.state.settings.monitors.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            scale: Some(2.0),
            ..OutputConfig::default()
        },
    );
    write(&mut r, "compat", r#"{"sharp_x11":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains(r#"hl.env("GDK_SCALE", "2", true)"#), "{g}");
}

#[tokio::test]
async fn compat_without_hyprland_leaves_scale_lines_out() {
    let mut r = rig();
    dp1(&r, 2.0);
    *r.comp.running.lock().unwrap() = false;
    write(&mut r, "compat", r#"{"sharp_x11":true,"steam_scale":true}"#)
        .await
        .unwrap();
    let g = generated(&r);
    assert!(g.contains("force_zero_scaling = true"), "{g}");
    assert!(
        !g.contains("GDK_SCALE") && !g.contains("STEAM_FORCE"),
        "{g}"
    );
    assert_eq!(r.desktop.main_scale, None);
}

async fn assert_refused(r: &mut Rig, message: &str) {
    let before = state_toml(r);
    let err = write(r, "compat", ELSEWHERE).await.unwrap_err();
    assert_eq!(err, DesktopError::InvalidSetting(message.into()));
    assert_eq!(state_toml(r), before);
    assert!(r.desktop.history(10).is_empty());
    assert!(r.desktop.state().compat.apps.is_empty());
}

#[tokio::test]
async fn compat_elsewhere_refused_for_own_file() {
    let mut r = rig();
    put_source(&r);
    put(&override_path(&r), SOURCE);
    assert_refused(&mut r, OWN_FILE).await;
    assert_eq!(std::fs::read_to_string(override_path(&r)).unwrap(), SOURCE);
}

#[tokio::test]
async fn compat_elsewhere_refused_without_source() {
    let mut r = rig();
    assert_refused(&mut r, NO_SOURCE).await;
}

#[tokio::test]
async fn compat_elsewhere_refused_in_dev_session() {
    let mut r = rig_dev(true);
    put_source(&r);
    assert_refused(&mut r, DEV_SESSION).await;
    assert!(!files::present(&override_path(&r)));
}

#[tokio::test]
async fn undo_compat_removes_override() {
    let mut r = rig();
    put_source(&r);
    write(&mut r, "compat", ELSEWHERE).await.unwrap();
    assert!(files::is_ours(&override_path(&r), MarkerKind::Override));
    let id = r.desktop.history(10)[0].id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert!(r.desktop.state().compat.apps.is_empty());
    assert!(!files::present(&override_path(&r)));
}

#[tokio::test]
async fn restore_point_brings_flags_back() {
    let mut r = rig();
    put(
        &root(&r).join("share-dirs/applications/chromium.desktop"),
        SOURCE,
    );
    let flags = r
        .desktop
        .paths()
        .startup
        .config_home
        .join("chromium-flags.conf");
    write(&mut r, "compat", r#"{"chromium_wayland":true}"#)
        .await
        .unwrap();
    assert!(files::is_ours(&flags, MarkerKind::Flags));
    let point = r.desktop.save_restore_point("with flags").unwrap();
    write(&mut r, "compat", r#"{"chromium_wayland":false}"#)
        .await
        .unwrap();
    assert!(!files::present(&flags));
    r.desktop.restore(&point).await.unwrap();
    assert!(files::is_ours(&flags, MarkerKind::Flags));
    let rows = explain(&r, "compat").await;
    assert_eq!(
        rows["chromium_wayland"]["detail"]["browsers"],
        json!([{"name": "Chromium", "file": "ours"}])
    );
}

#[tokio::test]
async fn autostart_section_writes_hidden_copy_and_never_renders() {
    let mut r = rig();
    let applet = "[Desktop Entry]\nType=Application\nName=Applet\nExec=nm-applet\n";
    put(&root(&r).join("xdg/autostart/nm-applet.desktop"), applet);
    write(
        &mut r,
        "autostart",
        r#"{"files":[{"id":"nm-applet.desktop","enabled":false,"delay":null}],"added":[]}"#,
    )
    .await
    .unwrap();
    let copy = r
        .desktop
        .paths()
        .startup
        .user_autostart()
        .join("nm-applet.desktop");
    let t = std::fs::read_to_string(&copy).unwrap();
    assert!(
        t.contains("Hidden=true") && t.contains("X-uQ-Autostart=true"),
        "{t}"
    );
    let calls = r.comp.calls();
    assert!(!calls.iter().any(|c| c.starts_with("verify")), "{calls:?}");
    assert_eq!(r.desktop.history(10)[0].domain, "autostart");
    let rows = explain(&r, "autostart").await;
    assert_eq!(rows["nm-applet.desktop"]["source"], "uq");
}

#[tokio::test]
async fn explain_env_user_lua_and_file_error() {
    let mut r = rig();
    put(
        &r.desktop.paths().user_lua,
        "-- mine\nhl.env(\"FOO\", \"mine\")\n",
    );
    let two = r#"{"vars":[{"name":"FOO","value":"a"},{"name":"BAR","value":"b"}]}"#;
    write(&mut r, "env", two).await.unwrap();
    let rows = explain(&r, "env").await;
    assert_eq!(rows["vars.FOO"]["source"], "user_lua");
    assert_eq!(rows["vars.FOO"]["detail"]["line"], 2);
    assert_eq!(rows["vars.BAR"]["source"], "uq");
    assert!(rows.get("env").is_none());

    // A folder where the file goes: the write can't land.
    let path = files::environment_d_path(&r.desktop.paths().startup);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"c"}]}"#)
        .await
        .unwrap();
    let rows = explain(&r, "env").await;
    assert_eq!(rows["env"]["source"], "error");
    assert_eq!(rows["vars.FOO"]["source"], "error");
}

#[tokio::test]
async fn explain_compat_sharp_x11_runtime() {
    let mut r = rig();
    r.comp
        .set_option(uq_core::compat::hypr::XWAYLAND_ZERO_SCALING, json!(false));
    write(&mut r, "compat", r#"{"sharp_x11":true}"#)
        .await
        .unwrap();
    let rows = explain(&r, "compat").await;
    assert_eq!(rows["sharp_x11"]["source"], "runtime");
    assert_eq!(rows["electron_wayland"]["source"], "uq");
}

#[tokio::test]
async fn explain_compat_sharp_x11_off_is_default() {
    let r = rig();
    let rows = explain(&r, "compat").await;
    assert_eq!(rows["sharp_x11"]["source"], "default");
    assert!(rows.get("qt_wayland").is_some());
}

#[tokio::test]
async fn rejected_env_keeps_previous_config() {
    let mut r = rig();
    write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"a"}]}"#)
        .await
        .unwrap();
    let before = generated(&r);
    *r.comp.verify_error.lock().unwrap() = Some("bad env".into());
    let err = write(&mut r, "env", r#"{"vars":[{"name":"FOO","value":"b"}]}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, DesktopError::ConfigInvalid(_)), "{err:?}");
    assert_eq!(generated(&r), before);
    assert!(env_file(&r).unwrap().contains("FOO='a'"));
    let rows = explain(&r, "env").await;
    assert_eq!(rows["env"]["source"], "error");
    assert_eq!(rows["vars.FOO"]["source"], "error");
}

#[tokio::test]
async fn env_not_in_settings_changed() {
    let mut r = rig();
    write(
        &mut r,
        "env",
        r#"{"vars":[{"name":"FOO_SECRET","value":"x"}]}"#,
    )
    .await
    .unwrap();
    let events: Vec<Event> = std::iter::from_fn(|| r.events.try_recv().ok()).collect();
    assert!(events.iter().any(
        |e| matches!(e, Event::SectionChanged { domain, json } if domain == "env" && json.contains("FOO_SECRET"))
    ));
    let settings: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::SettingsChanged(s) => Some(serde_json::to_string(s).unwrap()),
            _ => None,
        })
        .collect();
    assert!(!settings.is_empty());
    assert!(settings.iter().all(|s| !s.contains("FOO_SECRET")));
}

/// Every regular file and symlink under `dir`.
fn tree(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => out.extend(tree(&p)),
            _ => out.push(p),
        }
    }
    out
}

#[tokio::test]
async fn tests_never_touch_the_real_home() {
    let mut r = rig();
    let s = r.desktop.paths().startup.clone();
    let mut all = vec![&s.config_home, &s.data_home, &s.home];
    all.extend(&s.config_dirs);
    all.extend(&s.data_dirs);
    all.extend(&s.runtime_dir);
    for p in all {
        assert!(p.starts_with(root(&r)), "{p:?}");
    }
    let real =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/environment.d/60-uq.conf"));
    let real_before = real
        .as_ref()
        .map(|p| std::fs::metadata(p).ok().map(|m| m.modified().ok()));
    put(
        &root(&r).join("share-dirs/applications/chromium.desktop"),
        SOURCE,
    );
    put_source(&r);
    put(
        &root(&r).join("xdg/autostart/nm-applet.desktop"),
        "[Desktop Entry]\nType=Application\nName=A\nExec=a\n",
    );
    let mut st = r.desktop.state().clone();
    st.set_section("env", json!({"vars": [{"name": "FOO", "value": "1"}]}))
        .unwrap();
    st.set_section("compat", serde_json::from_str(ELSEWHERE).unwrap())
        .unwrap();
    st.compat.chromium_wayland = true;
    st.set_section(
        "autostart",
        json!({"files": [{"id": "nm-applet.desktop", "enabled": false}], "added": []}),
    )
    .unwrap();
    r.desktop.state = st;
    r.desktop.startup().await;
    // The full sync ran: every kind of file is there, inside the rig.
    let files = tree(root(&r));
    for want in [
        files::environment_d_path(&s),
        s.config_home.join("chromium-flags.conf"),
        override_path(&r),
        s.user_autostart().join("nm-applet.desktop"),
    ] {
        assert!(files.contains(&want), "{want:?} in {files:?}");
    }
    let real_after = real
        .as_ref()
        .map(|p| std::fs::metadata(p).ok().map(|m| m.modified().ok()));
    assert_eq!(real_before, real_after);
}
