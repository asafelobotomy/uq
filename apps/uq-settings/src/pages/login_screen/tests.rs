use super::*;
use crate::pages::fake_system::{FakeHelper, SysRig};
use crate::pages::fake_uqd;
use crate::pages::runner::run;
use serde_json::json;
use uq_core::state::State;

struct T {
    _rig: SysRig,
    helper: FakeHelper,
    uqd: Option<fake_uqd::Rig>,
    page: LoginScreenState,
}

async fn setup(with_helper: bool, with_uqd: bool) -> T {
    let mut rig = SysRig::new();
    let helper = if with_helper {
        FakeHelper::start(&mut rig, &[("alice", 1000)]).await
    } else {
        FakeHelper::files_only()
    };
    let system = rig.system();
    let uqd = if with_uqd {
        let mut state = State {
            theme: "dracula".into(),
            ..Default::default()
        };
        state
            .set_section(
                "wallpaper",
                json!({"outputs": {"*": {"image": "/nonexistent/pic.png", "fit": "fill", "interval": 300}}}),
            )
            .unwrap();
        state
            .set_section(
                "monitors",
                json!({"outputs": {"DP-1": {"mode": "2560x1440@144", "scale": 1.25, "description": "Dell U27"}}}),
            )
            .unwrap();
        Some(fake_uqd::rig(state).await)
    } else {
        None
    };
    let page = LoginScreenState::with(
        uqd.as_ref().map(|r| r.client.clone()),
        system,
        helper.root.path().to_path_buf(),
        "alice".into(),
    );
    T {
        _rig: rig,
        helper,
        uqd,
        page,
    }
}

fn appearance(theme: &str, variant: &str, hide: bool, motion: bool) -> Op {
    Op::SetAppearance {
        theme: theme.into(),
        variant: variant.into(),
        hide_users: hide,
        reduce_motion: motion,
    }
}

#[tokio::test]
async fn reads_the_defaults_and_the_shipped_variants() {
    let t = setup(true, true).await;
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["installed"], true);
    assert_eq!(s["helper"], true);
    assert_eq!(
        s["greeter"],
        json!({"theme": "graphite", "variant": "dark", "hide_users": false, "reduce_motion": false})
    );
    let ids: Vec<_> = s["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["dracula-dark", "graphite-dark", "graphite-light"]);
    assert_eq!(
        s["mine"],
        json!({"theme": "dracula", "variant": "dark", "available": true})
    );
    assert_eq!(s["wallpaper"]["mine"], "/nonexistent/pic.png");
    assert_eq!(s["wallpaper"]["set"], false);
    assert_eq!(s["auto_login"], json!({"user": "", "me": "alice"}));
}

#[tokio::test]
async fn every_setting_round_trips_through_the_helper() {
    let t = setup(true, true).await;
    let out = run(&t.page, appearance("graphite", "light", true, true)).await;
    assert_eq!(out.error, None);
    assert_eq!(
        out.state["greeter"],
        json!({"theme": "graphite", "variant": "light", "hide_users": true, "reduce_motion": true})
    );
    assert_eq!(out.state["notice"]["text"], SAVED);
    // Re-opening the page (a new model over the same files) reads the same values.
    let again = LoginScreenState::with(
        None,
        t._rig.system(),
        t.helper.root.path().to_path_buf(),
        "alice".into(),
    );
    let out = run(&again, Op::Refresh).await;
    assert_eq!(out.state["greeter"]["variant"], "light");
    assert_eq!(out.state["greeter"]["hide_users"], true);
    assert!(out.state["mine"].is_null(), "no uqd: no theme of mine");
}

#[tokio::test]
async fn same_as_mine_copies_the_current_theme() {
    let t = setup(true, true).await;
    let out = run(&t.page, appearance("", "", false, false)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["greeter"]["theme"], "dracula");
    assert_eq!(out.state["greeter"]["variant"], "dark");
    // A theme that isn't shipped pre-rendered is refused before the helper is asked.
    t.uqd.as_ref().unwrap().state.lock().unwrap().theme = "velvet".into();
    let out = run(&t.page, appearance("", "", false, false)).await;
    assert!(
        out.error
            .unwrap()
            .contains("isn't installed for the login screen")
    );
    assert_eq!(out.state["greeter"]["theme"], "dracula", "unchanged");
}

#[tokio::test]
async fn a_refused_prompt_leaves_everything_quietly() {
    let t = setup(true, true).await;
    t.helper.deny.set(true);
    let out = run(&t.page, appearance("graphite", "light", true, false)).await;
    assert_eq!(out.error, None, "a cancelled prompt is not an error");
    assert_eq!(out.state["greeter"]["variant"], "dark");
    assert_eq!(out.state["notice"]["text"], "", "no 'saved' notice");
    let out = run(&t.page, Op::SetAutoLogin { on: true }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["auto_login"]["user"], "");
}

#[tokio::test]
async fn auto_login_on_and_off() {
    let t = setup(true, false).await;
    let out = run(&t.page, Op::SetAutoLogin { on: true }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["auto_login"]["user"], "alice");
    let toml = t.helper.read(GREETD_TOML).unwrap();
    assert!(toml.contains("[initial_session]"), "{toml}");
    assert!(
        toml.contains("[default_session]"),
        "other tables kept: {toml}"
    );
    assert_eq!(auto_login_user(t.helper.root.path()), "alice");

    let out = run(&t.page, Op::SetAutoLogin { on: false }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["auto_login"]["user"], "");
    assert_eq!(out.state["notice"]["text"], "Auto-login is off.");
}

#[tokio::test]
async fn a_user_the_helper_refuses_shows_its_reason() {
    let mut t = setup(true, false).await;
    t.page.me = "ghost".into();
    let out = run(&t.page, Op::SetAutoLogin { on: true }).await;
    assert_eq!(out.error.as_deref(), Some("no such user"));
}

#[tokio::test]
async fn wallpaper_copy_pick_and_clear() {
    let t = setup(true, true).await;
    // The user's own picture is missing: a plain reason, nothing written.
    let out = run(&t.page, Op::WallpaperMine).await;
    assert!(
        out.error
            .unwrap()
            .starts_with("That picture couldn't be opened")
    );

    let dir = tempfile::tempdir().unwrap();
    let pic = dir.path().join("p.png");
    std::fs::write(&pic, b"\x89PNG fake").unwrap();
    let out = run(&t.page, Op::WallpaperFile { path: pic.clone() }).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["wallpaper"]["set"], true);
    assert_eq!(
        std::fs::read(t.helper.path(GREETER_WALLPAPER)).unwrap(),
        b"\x89PNG fake"
    );

    let out = run(&t.page, Op::ClearWallpaper).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["wallpaper"]["set"], false);

    // A directory and an oversized file are refused here, before the helper.
    let out = run(
        &t.page,
        Op::WallpaperFile {
            path: dir.path().to_path_buf(),
        },
    )
    .await;
    assert!(out.error.is_some());
    let big = dir.path().join("big.png");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(MAX_WALLPAPER + 1).unwrap();
    let out = run(&t.page, Op::WallpaperFile { path: big }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("That picture is larger than 20 MiB. Pick a smaller one.")
    );
}

#[tokio::test]
async fn my_display_layout_is_copied_without_names() {
    let t = setup(true, true).await;
    let out = run(&t.page, Op::UseMyDisplays).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["monitors"]["set"], true);
    let toml = t.helper.read(GREETER_MONITORS).unwrap();
    assert!(
        toml.contains("DP-1") && toml.contains("2560x1440@144"),
        "{toml}"
    );
    assert!(!toml.contains("Dell"), "descriptions are dropped");

    // "Use automatic layout" removes the copy.
    let out = run(&t.page, Op::AutomaticDisplays).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["monitors"]["set"], false);
    assert!(t.helper.read(GREETER_MONITORS).is_none());
}

#[tokio::test]
async fn without_uqd_the_copies_explain_themselves() {
    let t = setup(true, false).await;
    let out = run(&t.page, Op::UseMyDisplays).await;
    assert!(out.error.unwrap().contains("isn't running"));
    let out = run(&t.page, appearance("", "", false, false)).await;
    assert!(out.error.unwrap().contains("Pick a theme from the list"));
}

#[tokio::test]
async fn helper_missing_reads_as_unavailable() {
    let t = setup(false, false).await;
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.state["helper"], false);
    let out = run(&t.page, Op::SetAutoLogin { on: true }).await;
    assert_eq!(
        out.error.as_deref(),
        Some("uQ's system settings helper isn't available. Install the uq package.")
    );
}

#[test]
fn variants_skip_odd_directories() {
    let d = tempfile::tempdir().unwrap();
    for (name, tokens) in [
        ("graphite-dark", true),
        ("graphite-dim", true),
        ("Bad-dark", true),
        ("forest-light", false),
        ("nodash", true),
    ] {
        std::fs::create_dir_all(d.path().join(name)).unwrap();
        if tokens {
            std::fs::write(d.path().join(name).join("tokens.json"), "{}").unwrap();
        }
    }
    assert_eq!(
        rendered_variants(d.path()),
        [("graphite".to_string(), "dark".to_string())]
    );
}

#[test]
fn wallpaper_prefers_the_all_screens_entry() {
    let s = json!({"outputs": {"DP-1": {"image": "/a.png"}, "*": {"image": "/b.png"}}});
    assert_eq!(wallpaper_image(&s), Some(PathBuf::from("/b.png")));
    let s = json!({"outputs": {"*": {"folder": "/pics"}, "DP-1": {"image": "/a.png"}}});
    assert_eq!(wallpaper_image(&s), Some(PathBuf::from("/a.png")));
    assert_eq!(wallpaper_image(&json!({"outputs": {}})), None);
}
