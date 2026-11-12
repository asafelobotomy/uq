use super::*;
use crate::compositor::fake::FakeCompositor;
use crate::system::FakeSystem;
use std::sync::Arc;
use uq_core::paths::Paths;

/// What Hyprland 0.57 reported when `base.lua` was missing for a moment (cut short).
fn base_missing() -> Vec<String> {
    vec![
        "/home/u/.config/hypr/hyprland.lua:1: module '/usr/share/uq/hypr/base.lua' not found:\n\
         no field package.preload['/usr/share/uq/hypr/base.lua']\n\
         no file '/usr/share/lua/5.5//usr/share/uq/hypr/base/lua.lua'"
            .into(),
    ]
}

#[test]
fn reads_the_missing_module_names() {
    assert_eq!(
        missing_modules(&base_missing()),
        vec!["/usr/share/uq/hypr/base.lua"]
    );
    let two = vec![
        "a.lua:1: module './uq/generated.lua' not found:".to_string(),
        "x module 'b.lua' not found and module 'c.lua' not found".to_string(),
    ];
    assert_eq!(
        missing_modules(&two),
        vec!["./uq/generated.lua", "b.lua", "c.lua"]
    );
    assert!(missing_modules(&["syntax error near 'end'".into()]).is_empty());
    assert!(missing_modules(&["module '' not found".into()]).is_empty());
    assert!(missing_modules(&["module 'unterminated".into()]).is_empty());
    let long = format!("module '{}.lua' not found", "a".repeat(2000));
    assert!(missing_modules(&[long]).is_empty());
}

#[test]
fn module_paths_are_absolute_or_under_the_config_dir() {
    let hypr = Path::new("/home/u/.config/hypr");
    assert_eq!(
        module_path("/usr/share/uq/hypr/base.lua", hypr),
        Some(PathBuf::from("/usr/share/uq/hypr/base.lua"))
    );
    assert_eq!(
        module_path("./uq/generated.lua", hypr),
        Some(PathBuf::from("/home/u/.config/hypr/./uq/generated.lua"))
    );
    assert_eq!(module_path("socket", hypr), None);
}

#[test]
fn worth_reloading_only_when_every_missing_file_is_back() {
    let d = tempfile::tempdir().unwrap();
    let here = d.path().join("base.lua");
    let errs = |name: &str| vec![format!("x:1: module '{name}' not found:")];
    let name = here.to_str().unwrap();
    assert!(!worth_reloading(&errs(name), d.path()), "still missing");
    std::fs::write(&here, "").unwrap();
    assert!(worth_reloading(&errs(name), d.path()));
    let both = vec![format!(
        "module '{name}' not found; module 'gone.lua' not found"
    )];
    assert!(!worth_reloading(&both, d.path()), "one is still missing");
    assert!(!worth_reloading(&[], d.path()));
    assert!(!worth_reloading(&["syntax error".into()], d.path()));
}

fn desktop() -> (tempfile::TempDir, Desktop, Arc<FakeCompositor>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let d = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    (dir, d, comp)
}

fn reloads(comp: &FakeCompositor) -> usize {
    comp.calls().iter().filter(|c| *c == "reload").count()
}

#[tokio::test]
async fn a_file_back_after_a_failed_reload_gets_one_more_reload() {
    let (dir, mut d, comp) = desktop();
    let base = dir.path().join("base.lua");
    std::fs::write(&base, "").unwrap();
    *comp.config_errors.lock().unwrap() = vec![format!(
        "hyprland.lua:1: module '{}' not found:",
        base.display()
    )];
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 1);
    // The reload failed the same way: not again within RETRY_GAP, so no reload loop.
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 1);
}

#[tokio::test]
async fn a_clean_reload_or_a_real_error_is_left_alone() {
    let (dir, mut d, comp) = desktop();
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 0, "no errors");
    *comp.config_errors.lock().unwrap() = vec![format!(
        "hyprland.lua:3: module '{}' not found:",
        dir.path().join("never-there.lua").display()
    )];
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 0, "the file really is missing");
    *comp.config_errors.lock().unwrap() = vec!["user.lua:2: unexpected symbol near 'x'".into()];
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 0, "a syntax error is the user's to fix");
}
