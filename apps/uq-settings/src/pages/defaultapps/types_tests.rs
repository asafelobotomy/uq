//! File types, links, going back to the system's choice, uQ's own apps and the desktop's own list.

use super::*;
use crate::pages::runner::run;
use std::path::Path;
use uq_core::default_apps::{REELO, UQVIEW};
use uq_core::mimeapps::default_for;

fn app(dir: &Path, id: &str, name: &str, mimes: &str) {
    std::fs::write(
        dir.join(id),
        format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\nMimeType={mimes}\n"),
    )
    .unwrap();
}

struct T {
    dir: tempfile::TempDir,
    page: DefaultAppsState,
}

impl T {
    fn user(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("config/mimeapps.list")).unwrap_or_default()
    }
    fn hypr(&self) -> std::path::PathBuf {
        self.dir.path().join("config/hyprland-mimeapps.list")
    }
}

fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let apps = dir.path().join("apps");
    let mime = dir.path().join("mime");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::create_dir_all(mime.join("image")).unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    std::fs::write(mime.join("image/avif.xml"), "<comment>AVIF image</comment>").unwrap();
    app(&apps, UQVIEW, "uQview", "image/png;image/avif;");
    app(&apps, REELO, "reelo", "video/mp4;video/ogg;audio/mpeg;");
    app(&apps, "feh.desktop", "feh", "image/png;");
    app(&apps, "irc.desktop", "Chat", "x-scheme-handler/irc;");
    std::fs::write(
        dir.path().join("system.list"),
        "[Default Applications]\nimage/png=feh.desktop;\n",
    )
    .unwrap();
    let page = DefaultAppsState::with(
        None,
        vec![apps],
        dir.path().join("config/mimeapps.list"),
        vec![dir.path().join("system.list")],
    )
    .with_overrides(desktop_files(&dir.path().join("config"), "Hyprland"))
    .with_mime_dirs(vec![mime]);
    T { dir, page }
}

fn find<'a>(list: &'a Value, key: &str, val: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|v| v[key] == val)
        .unwrap_or_else(|| panic!("no {val}"))
}

#[tokio::test]
async fn lists_every_type_and_link_with_names() {
    let t = setup();
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let types = &out.state["types"];
    let avif = find(types, "mime", "image/avif");
    assert_eq!(avif["label"], "AVIF image");
    assert_eq!(avif["group"], "images");
    assert_eq!(avif["apps"], json!([UQVIEW]));
    let irc = find(types, "mime", "x-scheme-handler/irc");
    assert_eq!(irc["group"], "links");
    assert_eq!(irc["label"], "Chat rooms (IRC)");
    assert_eq!(out.state["apps"][UQVIEW]["native"], true);
    assert_eq!(out.state["apps"]["feh.desktop"]["native"], false);
    let images = find(&out.state["kinds"], "id", "images");
    assert_eq!(images["native"], UQVIEW);
    assert_eq!(out.state["native_left"], true);
}

#[tokio::test]
async fn one_type_set_and_reset() {
    let t = setup();
    let set = Op::SetType {
        mime: "image/avif".into(),
        app: UQVIEW.into(),
    };
    assert_eq!(run(&t.page, set).await.error, None);
    assert_eq!(
        default_for(&t.user(), "image/avif").as_deref(),
        Some(UQVIEW)
    );
    assert_eq!(default_for(&t.user(), "image/png"), None, "only that one");
    let wrong = Op::SetType {
        mime: "image/avif".into(),
        app: "feh.desktop".into(),
    };
    assert!(run(&t.page, wrong).await.error.is_some());
    let bad = Op::SetType {
        mime: "image/png\n[x]".into(),
        app: UQVIEW.into(),
    };
    assert!(run(&t.page, bad).await.error.is_some());
    let out = run(
        &t.page,
        Op::ResetType {
            mime: "image/avif".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(default_for(&t.user(), "image/avif"), None);
}

#[tokio::test]
async fn reset_goes_back_to_the_system_and_native_fills_all() {
    let t = setup();
    let out = run(&t.page, Op::UseNative).await;
    assert_eq!(out.error, None);
    let user = t.user();
    for (m, app) in [
        ("image/png", UQVIEW),
        ("image/avif", UQVIEW),
        ("video/ogg", REELO),
        ("audio/mpeg", REELO),
    ] {
        assert_eq!(default_for(&user, m).as_deref(), Some(app), "{m}");
    }
    assert_eq!(out.state["native_left"], false);
    let out = run(
        &t.page,
        Op::ResetDefault {
            kind: "images".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let images = find(&out.state["kinds"], "id", "images");
    assert_eq!(images["current"], "feh.desktop");
    assert_eq!(images["source"], "system");
    assert_eq!(
        default_for(&t.user(), "image/avif"),
        None,
        "the family is cleared too"
    );
}

#[tokio::test]
async fn a_choice_here_wins_over_the_desktops_own_list() {
    let t = setup();
    std::fs::write(
        t.hypr(),
        "[Default Applications]\nimage/png=feh.desktop;\ntext/plain=kate.desktop;\n",
    )
    .unwrap();
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.state["override_file"], "hyprland-mimeapps.list");
    assert_eq!(
        find(&out.state["kinds"], "id", "images")["source"],
        "override"
    );
    let set = Op::SetDefault {
        kind: "images".into(),
        app: UQVIEW.into(),
    };
    let out = run(&t.page, set).await;
    assert_eq!(out.error, None);
    let images = find(&out.state["kinds"], "id", "images");
    assert_eq!(images["current"], UQVIEW);
    assert_eq!(images["source"], "you");
    let hypr = std::fs::read_to_string(t.hypr()).unwrap();
    assert_eq!(default_for(&hypr, "image/png"), None);
    assert_eq!(
        default_for(&hypr, "text/plain").as_deref(),
        Some("kate.desktop")
    );
}

#[test]
fn desktop_lists_are_named_safely() {
    let d = Path::new("/c");
    assert_eq!(
        desktop_files(d, "Hyprland:uQ"),
        [d.join("hyprland-mimeapps.list"), d.join("uq-mimeapps.list")]
    );
    assert!(desktop_files(d, "../x").is_empty());
    assert!(desktop_files(d, "").is_empty());
}
