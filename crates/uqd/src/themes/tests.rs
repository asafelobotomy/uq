use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use uq_core::paths::Paths;

struct Rig {
    dir: tempfile::TempDir,
    desktop: Desktop,
    comp: Arc<FakeCompositor>,
    events: broadcast::Receiver<Event>,
}

fn paths(dir: &Path) -> Paths {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir);
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    paths
}

fn desktop_on(dir: &Path, comp: Arc<FakeCompositor>) -> Desktop {
    Desktop::new(paths(dir), comp, Arc::new(FakeSystem::default())).unwrap()
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let comp = Arc::new(FakeCompositor::running());
    let mut desktop = desktop_on(dir.path(), comp.clone());
    desktop.startup().await;
    let events = desktop.subscribe();
    Rig {
        dir,
        desktop,
        comp,
        events,
    }
}

impl Rig {
    fn root(&self) -> PathBuf {
        self.desktop.paths().user_themes_dir.clone()
    }

    fn events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.events.try_recv().ok()).collect()
    }

    fn themes_changed(&mut self) -> usize {
        self.events()
            .iter()
            .filter(|e| matches!(e, Event::ThemesChanged))
            .count()
    }

    async fn import(
        &mut self,
        text: &str,
        replace: bool,
        look: bool,
    ) -> Result<String, DesktopError> {
        self.desktop
            .import_theme(text, replace, look, Origin::Settings)
            .await
    }

    fn user_ids(&mut self) -> Vec<String> {
        self.desktop.theme_info()["themes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["user"] == true)
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    }
}

/// A theme file for slug `slug` named `name`, made from gruvbox.
fn file(d: &Desktop, slug: &str, name: &str) -> String {
    d.export_theme("gruvbox", false)
        .unwrap()
        .replacen("id = \"gruvbox\"", &format!("id = \"{slug}\""), 1)
        .replacen("name = \"Gruvbox\"", &format!("name = \"{name}\""), 1)
}

fn with_look(text: &str) -> String {
    format!("{text}\n[look]\ngaps_in = 3\n")
}

fn write_theme(root: &Path, id: &str) {
    let d = tempfile::tempdir().unwrap();
    let desk = desktop_on(d.path(), Arc::new(FakeCompositor::running()));
    let t = uqtheme::parse(&file(&desk, id.trim_start_matches("user-"), "Hand")).unwrap();
    atomic_replace_dir(&root.join(id), &uqtheme::install_files(&t)).unwrap();
}

fn read(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn mtime(p: &Path) -> std::time::SystemTime {
    std::fs::metadata(p).unwrap().modified().unwrap()
}

#[tokio::test]
async fn startup_loads_user_themes_and_keeps_active() {
    let dir = tempfile::tempdir().unwrap();
    let root = paths(dir.path()).user_themes_dir;
    write_theme(&root, "user-ocean");
    std::fs::create_dir_all(dir.path().join("x")).ok();
    let state_file = paths(dir.path()).state_file;
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    std::fs::write(&state_file, "theme = \"user-ocean\"\nvariant = \"light\"\n").unwrap();
    let d = desktop_on(dir.path(), Arc::new(FakeCompositor::running()));
    assert_eq!(d.get_theme(), ("user-ocean".to_string(), Variant::Light));
}

#[tokio::test]
async fn startup_survives_broken_user_themes() {
    let dir = tempfile::tempdir().unwrap();
    let root = paths(dir.path()).user_themes_dir;
    std::fs::create_dir_all(root.join("notuser")).unwrap();
    std::fs::create_dir_all(root.join("user-onlydark")).unwrap();
    std::fs::write(root.join("user-onlydark/dark.toml"), "x").unwrap();
    std::fs::create_dir_all(root.join("user-junk")).unwrap();
    std::fs::write(root.join("user-junk/dark.toml"), "not a theme").unwrap();
    std::fs::write(root.join("user-junk/light.toml"), "not a theme").unwrap();
    std::os::unix::fs::symlink("/tmp", root.join("user-link")).unwrap();
    write_theme(&root, "user-fine");
    let mut d = desktop_on(dir.path(), Arc::new(FakeCompositor::running()));
    let info = d.theme_info();
    assert_eq!(info["skipped"].as_array().unwrap().len(), 4, "{info}");
    assert!(
        info["themes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "user-fine")
    );
}

#[tokio::test]
async fn import_saves_point_then_installs() {
    let mut r = rig().await;
    let before = r.desktop.state().clone();
    let n = r.desktop.restore_points().len();
    let text = file(&r.desktop, "ocean", "Ocean");
    assert_eq!(r.import(&text, false, false).await.unwrap(), "user-ocean");
    let points = r.desktop.restore_points();
    assert_eq!(points.len(), n + 1, "{points:?}");
    let point = points
        .iter()
        .find(|p| p.kind == PointKind::Auto(AutoReason::BeforeImport))
        .unwrap();
    assert_eq!(r.desktop.restore.load(&point.id).unwrap(), before);
    let dark = std::fs::read_to_string(r.root().join("user-ocean/dark.toml")).unwrap();
    let light = std::fs::read_to_string(r.root().join("user-ocean/light.toml")).unwrap();
    let got = uq_core::theme::user::scan(&r.root());
    assert_eq!(got.themes.len(), 1);
    assert!(dark.contains("Ocean") && light.contains("Ocean"));
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
    assert_eq!(r.themes_changed(), 1);
    assert_ne!(r.desktop.get_theme().0, "user-ocean");
}

#[tokio::test]
async fn refused_import_changes_nothing() {
    let mut r = rig().await;
    let n = r.desktop.restore_points().len();
    let good = file(&r.desktop, "ocean", "Ocean");
    let bad = [
        "not a theme".to_string(),
        good.replacen("format = 1", "format = 2", 1),
        good.replacen("name = \"Ocean\"", "name = \"X\\u2028Y\"", 1),
        good.replacen("id = \"ocean\"", "id = \"../x\"", 1),
        format!("{good}\nextra = 1\n"),
        " ".repeat(70_000),
    ];
    for text in bad {
        assert!(matches!(
            r.import(&text, false, false).await,
            Err(DesktopError::InvalidArgument(_))
        ));
    }
    assert_eq!(r.desktop.restore_points().len(), n);
    assert!(!r.root().exists());
    assert_eq!(r.themes_changed(), 0);
}

#[tokio::test]
async fn import_conflict_then_replace() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    match r.import(&text, false, false).await {
        Err(DesktopError::Conflict(m)) => assert!(m.ends_with("Replace it?"), "{m}"),
        other => panic!("{other:?}"),
    }
    r.import(&text, true, false).await.unwrap();
    assert_eq!(r.user_ids(), ["user-ocean"]);
}

#[tokio::test]
async fn replace_refuses_folder_with_extra_files() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    let notes = r.root().join("user-ocean/notes.txt");
    std::fs::write(&notes, "mine").unwrap();
    match r.import(&text, true, false).await {
        Err(DesktopError::InvalidArgument(m)) => assert!(m.contains("other files"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "mine");
}

#[tokio::test]
async fn import_sees_folder_removed_by_hand() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    std::fs::remove_dir_all(r.root().join("user-ocean")).unwrap();
    r.import(&text, false, false).await.unwrap();
}

#[tokio::test]
async fn replacing_active_theme_rerenders_without_history() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let tokens = r.desktop.paths().theme_dir.join("tokens.json");
    let old = read(&tokens).unwrap();
    let len = r.desktop.history(1000).len();
    let changed = text.replacen("radius = 8", "radius = 9", 1);
    let changed = if changed == text {
        // Whatever the fixture's radius is, move it.
        let t = uqtheme::parse(&text).unwrap();
        text.replacen(
            &format!("radius = {}", t.dark.ui.radius),
            &format!("radius = {}", t.dark.ui.radius + 1),
            1,
        )
    } else {
        changed
    };
    assert_ne!(changed, text);
    r.import(&changed, true, false).await.unwrap();
    assert_ne!(read(&tokens).unwrap(), old);
    assert_eq!(r.desktop.history(1000).len(), len);
}

#[tokio::test]
async fn import_with_look_is_one_undoable_entry() {
    let mut r = rig().await;
    r.desktop
        .set_section("look", r#"{"gaps_in":5}"#, Origin::Settings)
        .await
        .unwrap();
    let n = r.desktop.history(1000).len();
    let text = with_look(&file(&r.desktop, "ocean", "Ocean"));
    r.import(&text, false, true).await.unwrap();
    let h = r.desktop.history(1000);
    assert_eq!(h.len(), n + 1, "{h:?}");
    let last = h.first().unwrap(); // newest first
    assert_eq!(
        (last.domain.as_str(), last.origin.as_str()),
        ("look", "settings")
    );
    assert_eq!(r.desktop.state().section("look").unwrap()["gaps_in"], 3);
    let id = last.id.clone();
    r.desktop.undo(&id, false).await.unwrap();
    assert_eq!(r.desktop.state().section("look").unwrap()["gaps_in"], 5);
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
}

#[tokio::test]
async fn import_look_rejected_by_verify_keeps_theme() {
    let mut r = rig().await;
    *r.comp.verify_error.lock().unwrap() = Some("nope".into());
    let text = with_look(&file(&r.desktop, "ocean", "Ocean"));
    let err = r.import(&text, false, true).await.unwrap_err();
    assert!(
        err.to_string().starts_with("The theme was added, but"),
        "{err}"
    );
    assert!(r.desktop.theme_ids().contains(&"user-ocean".to_string()));
    assert_eq!(r.themes_changed(), 1);
}

#[tokio::test]
async fn import_refuses_linked_root() {
    let mut r = rig().await;
    let target = r.dir.path().join("elsewhere");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(r.root().parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, r.root()).unwrap();
    let text = file(&r.desktop, "ocean", "Ocean");
    let err = r.import(&text, false, false).await.unwrap_err();
    assert!(err.to_string().contains("is a link"), "{err}");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}

#[tokio::test]
async fn import_refuses_in_dev_session() {
    let mut r = rig().await;
    r.desktop.dev_session = true;
    let text = file(&r.desktop, "ocean", "Ocean");
    let err = r.import(&text, false, false).await.unwrap_err();
    assert!(err.to_string().contains("test session"), "{err}");
    assert!(r.desktop.export_theme("gruvbox", false).is_ok());
    assert!(!r.root().exists());
}

#[tokio::test]
async fn import_caps_at_fifty() {
    let mut r = rig().await;
    for n in 0..MAX_USER_THEMES {
        let text = file(&r.desktop, &format!("t{n}"), &format!("T {n}"));
        r.import(&text, false, false).await.unwrap();
    }
    let text = file(&r.desktop, "one-more", "One more");
    assert!(matches!(
        r.import(&text, false, false).await,
        Err(DesktopError::InvalidArgument(_))
    ));
    // A replace doesn't count.
    let again = file(&r.desktop, "t3", "T 3");
    r.import(&again, true, false).await.unwrap();
    assert_eq!(r.user_ids().len(), MAX_USER_THEMES);
}

#[tokio::test]
async fn export_matches_registry_and_never_holds_overrides() {
    let mut r = rig().await;
    r.desktop
        .set_section("appearance", r#"{"ui_font":"Comic"}"#, Origin::Settings)
        .await
        .unwrap();
    r.desktop
        .set_section("look", r#"{"gaps_in":4}"#, Origin::Settings)
        .await
        .unwrap();
    let plain = uqtheme::parse(&r.desktop.export_theme("gruvbox", false).unwrap()).unwrap();
    let stored = r.desktop.themes.get("gruvbox", Variant::Dark).unwrap();
    assert_eq!(plain.dark.fonts, stored.fonts);
    assert!(plain.look.is_none());
    let full = uqtheme::parse(&r.desktop.export_theme("gruvbox", true).unwrap()).unwrap();
    let look = full.look.unwrap();
    assert_eq!(look["gaps_in"], 4);
    assert!(look.values().all(|v| !v.is_null()));
    assert!(matches!(
        r.desktop.export_theme("nope", false),
        Err(DesktopError::UnknownTheme(_))
    ));
}

#[tokio::test]
async fn delete_active_switches_then_deletes() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    let out = r
        .desktop
        .delete_theme("user-ocean", Origin::Uqctl)
        .await
        .unwrap();
    assert_eq!(out, json!({"switched": true, "profiles": []}));
    assert!(!r.root().join("user-ocean").exists());
    assert_eq!(r.desktop.get_theme().0, DEFAULT_THEME);
    let h = r.desktop.history(1000);
    assert!(
        h.iter().any(|e| e.domain == "theme" && e.origin == "uqctl"),
        "{h:?}"
    );
    assert!(!r.desktop.theme_ids().contains(&"user-ocean".to_string()));
}

#[tokio::test]
async fn delete_reports_profiles_using_it() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_profile(
            "format = 1\nid = \"gaming\"\nname = \"Gaming\"\n[appearance]\ntheme = \"user-ocean\"\n",
            false,
        )
        .unwrap();
    let out = r
        .desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert_eq!(out, json!({"switched": false, "profiles": ["Gaming"]}));
}

#[tokio::test]
async fn delete_refuses_system_and_unknown() {
    let mut r = rig().await;
    assert!(matches!(
        r.desktop.delete_theme("gruvbox", Origin::Settings).await,
        Err(DesktopError::InvalidArgument(_))
    ));
    assert!(matches!(
        r.desktop.delete_theme("user-nope", Origin::Settings).await,
        Err(DesktopError::UnknownTheme(_))
    ));
    r.desktop.dev_session = true;
    assert!(matches!(
        r.desktop.delete_theme("user-nope", Origin::Settings).await,
        Err(DesktopError::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn delete_never_follows_links() {
    let mut r = rig().await;
    let target = r.dir.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    write_theme(&target.parent().unwrap().join("t"), "user-x");
    std::fs::rename(target.parent().unwrap().join("t/user-x"), &target).unwrap();
    std::fs::create_dir_all(r.root()).unwrap();
    std::os::unix::fs::symlink(&target, r.root().join("user-x")).unwrap();
    assert!(
        r.desktop
            .delete_theme("user-x", Origin::Settings)
            .await
            .is_err()
    );
    assert!(target.join("dark.toml").exists() && target.join("light.toml").exists());
    // Swapped for a link after it was loaded: still refused, target untouched.
    r.desktop
        .import_theme(&file(&r.desktop, "y", "Y"), false, false, Origin::Settings)
        .await
        .unwrap();
    std::fs::remove_dir_all(r.root().join("user-y")).unwrap();
    std::os::unix::fs::symlink(&target, r.root().join("user-y")).unwrap();
    assert!(
        r.desktop
            .delete_theme("user-y", Origin::Settings)
            .await
            .is_err()
    );
    assert!(target.join("dark.toml").exists());
}

#[tokio::test]
async fn delete_is_not_recursive() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    let notes = r.root().join("user-ocean/notes.txt");
    std::fs::write(&notes, "mine").unwrap();
    r.desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert!(notes.exists());
    assert!(r.user_ids().is_empty());
}

#[tokio::test]
async fn active_theme_kept_when_folder_removed() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop
        .set_theme("user-ocean", Variant::Dark, Origin::Settings)
        .await
        .unwrap();
    std::fs::remove_dir_all(r.root().join("user-ocean")).unwrap();
    assert!(r.desktop.refresh_themes());
    let info = r.desktop.theme_info();
    let t = info["themes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "user-ocean")
        .unwrap();
    assert_eq!(t["missing"], true);
    r.desktop
        .set_section("look", r#"{"gaps_in":6}"#, Origin::Settings)
        .await
        .unwrap();
}

#[tokio::test]
async fn refresh_without_change_reads_nothing() {
    let mut r = rig().await;
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.events();
    assert!(!r.desktop.refresh_themes());
    assert_eq!(r.themes_changed(), 0);
    write_theme(&r.root(), "user-hand");
    assert!(r.desktop.refresh_themes());
    assert_eq!(r.themes_changed(), 1);
    assert!(r.user_ids().contains(&"user-hand".to_string()));
}

#[tokio::test]
async fn user_lua_untouched() {
    let mut r = rig().await;
    let lua = r.desktop.paths().user_lua.clone();
    std::fs::create_dir_all(lua.parent().unwrap()).unwrap();
    std::fs::write(&lua, "-- mine\n").unwrap();
    let (bytes, time) = (read(&lua), mtime(&lua));
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    r.desktop.export_theme("user-ocean", true).unwrap();
    r.desktop
        .delete_theme("user-ocean", Origin::Settings)
        .await
        .unwrap();
    assert_eq!((read(&lua), mtime(&lua)), (bytes, time));
}

#[tokio::test]
async fn no_state_key_from_theme_store() {
    let mut r = rig().await;
    r.desktop
        .set_section("look", r#"{"gaps_in":6}"#, Origin::Settings)
        .await
        .unwrap();
    let state_file = r.desktop.paths().state_file.clone();
    let before = read(&state_file);
    let text = file(&r.desktop, "ocean", "Ocean");
    r.import(&text, false, false).await.unwrap();
    assert_eq!(read(&state_file), before);
}
