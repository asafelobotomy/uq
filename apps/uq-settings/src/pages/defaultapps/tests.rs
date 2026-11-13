use super::*;
use crate::pages::fake_uqd;
use crate::pages::runner::run;
use std::path::Path;
use uq_core::mimeapps::MAX_FILE;
use uq_core::state::State;

struct T {
    dir: tempfile::TempDir,
    _uqd: Option<fake_uqd::Rig>,
    page: DefaultAppsState,
}

fn app(dir: &Path, id: &str, name: &str, extra: &str) {
    std::fs::write(
        dir.join(id),
        format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\n{extra}"),
    )
    .unwrap();
}

async fn setup(with_uqd: bool) -> T {
    let dir = tempfile::tempdir().unwrap();
    let apps = dir.path().join("apps");
    std::fs::create_dir_all(&apps).unwrap();
    app(
        &apps,
        "firefox.desktop",
        "Firefox",
        "MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;\n",
    );
    app(
        &apps,
        "chromium.desktop",
        "Chromium",
        "MimeType=x-scheme-handler/http;\n",
    );
    app(
        &apps,
        "feh.desktop",
        "feh",
        "MimeType=image/png;image/jpeg;\nNoDisplay=true\n",
    );
    app(
        &apps,
        "hidden.desktop",
        "Hidden",
        "MimeType=image/png;\nHidden=true\n",
    );
    app(
        &apps,
        "kitty.desktop",
        "Kitty",
        "Categories=System;TerminalEmulator;\n",
    );
    app(
        &apps,
        "foot.desktop",
        "Foot",
        "Categories=TerminalEmulator;\n",
    );
    std::fs::write(
        dir.path().join("system.list"),
        "[Default Applications]\nimage/png=feh.desktop;\n",
    )
    .unwrap();
    let uqd = if with_uqd {
        Some(fake_uqd::rig(State::default()).await)
    } else {
        None
    };
    let page = DefaultAppsState::with(
        uqd.as_ref().map(|r| r.client.clone()),
        vec![apps],
        dir.path().join("config/mimeapps.list"),
        vec![dir.path().join("system.list")],
    );
    T {
        dir,
        _uqd: uqd,
        page,
    }
}

fn kind<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == id)
        .unwrap()
}

fn set(kind: &str, app: &str) -> Op {
    Op::SetDefault {
        kind: kind.into(),
        app: app.into(),
    }
}

#[tokio::test]
async fn offers_the_apps_that_open_each_kind() {
    let t = setup(true).await;
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let browser = kind(&out.state, "browser");
    let names: Vec<_> = browser["apps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Chromium", "Firefox"]);
    assert!(browser["current"].is_null());
    let images = kind(&out.state, "images");
    assert_eq!(images["current"], "feh.desktop");
    assert_eq!(images["source"], "system");
    assert_eq!(
        images["apps"].as_array().unwrap().len(),
        1,
        "hidden apps aren't offered"
    );
    let terminals: Vec<_> = out.state["terminal"]["apps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(terminals, ["foot.desktop", "kitty.desktop"]);
    assert_eq!(out.state["terminal"]["available"], true);
}

#[tokio::test]
async fn setting_a_default_keeps_every_other_line() {
    let t = setup(true).await;
    let file = t.dir.path().join("config/mimeapps.list");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let before = "# mine\n[Added Associations]\ntext/html=chromium.desktop;\n\n[Default Applications]\napplication/pdf=zathura.desktop;\nx-scheme-handler/http=chromium.desktop;\n";
    std::fs::write(&file, before).unwrap();
    let out = run(&t.page, set("browser", "firefox.desktop")).await;
    assert_eq!(out.error, None);
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(
        after.starts_with("# mine\n[Added Associations]\ntext/html=chromium.desktop;\n"),
        "{after}"
    );
    assert!(after.contains("application/pdf=zathura.desktop;"));
    for m in KINDS[0].mimes {
        assert_eq!(
            mimeapps::default_for(&after, m).as_deref(),
            Some("firefox.desktop"),
            "{m}"
        );
    }
    assert_eq!(kind(&out.state, "browser")["current"], "firefox.desktop");
    assert_eq!(kind(&out.state, "browser")["source"], "you");
    assert_eq!(kind(&out.state, "pdf")["current"], "zathura.desktop");

    // Re-opened, the page reads the same.
    let again = DefaultAppsState::with(None, t.page.app_dirs.clone(), file.clone(), vec![]);
    let out = run(&again, Op::Refresh).await;
    assert_eq!(kind(&out.state, "browser")["current"], "firefox.desktop");
}

#[tokio::test]
async fn a_symlinked_list_is_written_at_its_target() {
    let t = setup(true).await;
    let real = t.dir.path().join("dotfiles/mimeapps.list");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, "[Default Applications]\n").unwrap();
    let file = t.dir.path().join("config/mimeapps.list");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &file).unwrap();
    let out = run(&t.page, set("images", "feh.desktop")).await;
    assert_eq!(out.error, None);
    assert!(
        std::fs::symlink_metadata(&file)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        std::fs::read_to_string(&real)
            .unwrap()
            .contains("image/png=feh.desktop;")
    );
}

#[tokio::test]
async fn refuses_wrong_apps_and_huge_files() {
    let t = setup(true).await;
    let out = run(&t.page, set("images", "firefox.desktop")).await;
    assert!(out.error.unwrap().contains("can't open these"));
    let out = run(&t.page, set("nonsense", "firefox.desktop")).await;
    assert!(out.error.is_some());
    let file = t.dir.path().join("config/mimeapps.list");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let big = format!(
        "[Default Applications]\n{}",
        "#".repeat(MAX_FILE as usize + 10)
    );
    std::fs::write(&file, &big).unwrap();
    let out = run(&t.page, set("browser", "firefox.desktop")).await;
    assert!(out.error.unwrap().contains("too large"));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        big,
        "left untouched"
    );
}

#[tokio::test]
async fn terminal_through_uqd_and_without_it() {
    let t = setup(true).await;
    let out = run(
        &t.page,
        Op::SetTerminal {
            app: Some("foot.desktop".into()),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["terminal"]["current"], "foot.desktop");
    let out = run(&t.page, Op::SetTerminal { app: None }).await;
    assert_eq!(out.error, None);
    assert!(out.state["terminal"]["current"].is_null());
    let out = run(
        &t.page,
        Op::SetTerminal {
            app: Some("../x".into()),
        },
    )
    .await;
    assert!(out.error.is_some(), "uqd refuses a bad id");

    let t = setup(false).await;
    let out = run(&t.page, Op::Refresh).await;
    assert_eq!(out.state["terminal"]["available"], false);
    let out = run(
        &t.page,
        Op::SetTerminal {
            app: Some("foot.desktop".into()),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("uQ's settings service isn't running.")
    );
}
