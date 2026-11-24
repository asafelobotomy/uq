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
fn failed_modules_include_files_that_did_not_load() {
    // Hyprland 0.57's report for a file that is there but didn't load (cut short).
    let half = vec![
        "require(\"/usr/share/uq/hypr/base.lua\"): error loading module '/usr/share/uq/hypr/base.lua' from file '/usr/share/uq/hypr/base.lua':".to_string(),
        "\t/usr/share/uq/hypr/base.lua:812: 'end' expected (to close 'function' at line 790) near <eof>".to_string(),
    ];
    assert_eq!(failed_modules(&half), vec!["/usr/share/uq/hypr/base.lua"]);
    let mixed = vec![
        "hyprland.lua:2: module './uq/generated.lua' not found:".to_string(),
        "require(\"./uq/user.lua\"): user.lua:3: attempt to call a nil value".to_string(),
        "require(\"./uq/generated.lua\"): again".to_string(),
    ];
    assert_eq!(
        failed_modules(&mixed),
        vec!["./uq/generated.lua", "./uq/user.lua"]
    );
    assert!(failed_modules(&["user.lua:2: unexpected symbol near 'x'".into()]).is_empty());
    let hypr = Path::new("/h");
    assert!(failed_files(&["require(\"socket\"): nope".into()], hypr).is_empty());
}

fn desktop() -> (tempfile::TempDir, Desktop, Arc<FakeCompositor>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Paths::under(dir.path());
    paths.themes_dir = repo.join("themes");
    paths.templates_dir = repo.join("templates");
    let comp = Arc::new(FakeCompositor::running());
    let mut d = Desktop::new(paths, comp.clone(), Arc::new(FakeSystem::default())).unwrap();
    d.recover.busy = dir.path().join("db.lck");
    (dir, d, comp)
}

fn reloads(comp: &FakeCompositor) -> usize {
    comp.calls().iter().filter(|c| *c == "reload").count()
}

fn set_errors(comp: &FakeCompositor, errors: Vec<String>) {
    *comp.config_errors.lock().unwrap() = errors;
}

fn missing(p: &Path) -> Vec<String> {
    vec![format!(
        "hyprland.lua:1: module '{}' not found:",
        p.display()
    )]
}

fn did_not_load(p: &Path) -> Vec<String> {
    let p = p.display();
    vec![format!(
        "require(\"{p}\"): error loading module '{p}' from file '{p}':"
    )]
}

/// Long enough for the files to be seen twice unchanged.
const SETTLE: Duration = Duration::from_secs(3);

#[tokio::test(start_paused = true)]
async fn a_file_back_after_a_failed_reload_gets_one_more_reload() {
    let (dir, mut d, comp) = desktop();
    let base = dir.path().join("base.lua");
    std::fs::write(&base, "").unwrap();
    set_errors(&comp, missing(&base));
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 1);
    // The reload failed the same way with the same file: no reload loop.
    d.config_reloaded().await;
    tokio::time::sleep(WATCH_FOR).await;
    assert_eq!(reloads(&comp), 1);
}

#[tokio::test(start_paused = true)]
async fn a_half_written_file_gets_a_reload_once_per_version() {
    let (dir, mut d, comp) = desktop();
    let base = dir.path().join("base.lua");
    std::fs::write(&base, "hl.bind(").unwrap();
    set_errors(&comp, did_not_load(&base));
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 1, "reloaded once the file settled");
    d.config_reloaded().await;
    tokio::time::sleep(WATCH_FOR).await;
    assert_eq!(reloads(&comp), 1, "the same file again: left alone");
    std::fs::write(&base, "-- a different version\n").unwrap();
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 2, "a new version gets its own reload");
}

#[tokio::test(start_paused = true)]
async fn no_reload_while_a_package_transaction_runs() {
    let (dir, mut d, comp) = desktop();
    let base = dir.path().join("base.lua");
    std::fs::write(&base, "").unwrap();
    std::fs::write(&d.recover.busy, "").unwrap();
    set_errors(&comp, did_not_load(&base));
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE * 3).await;
    assert_eq!(reloads(&comp), 0, "pacman is still running");
    std::fs::remove_file(&d.recover.busy).unwrap();
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 1, "done: reloaded");
}

#[tokio::test(start_paused = true)]
async fn a_clean_reload_or_a_real_error_is_left_alone() {
    let (dir, mut d, comp) = desktop();
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 0, "no errors");
    set_errors(&comp, missing(&dir.path().join("never-there.lua")));
    d.config_reloaded().await;
    tokio::time::sleep(WATCH_FOR + SETTLE).await;
    assert_eq!(reloads(&comp), 0, "the file really is missing");
    set_errors(&comp, vec!["user.lua:2: unexpected symbol near 'x'".into()]);
    d.config_reloaded().await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        reloads(&comp),
        0,
        "a syntax error outside a require is the user's to fix"
    );
}

#[tokio::test(start_paused = true)]
async fn a_file_that_is_missing_for_seconds_still_gets_its_reload() {
    let (dir, mut d, comp) = desktop();
    let base = dir.path().join("base.lua");
    set_errors(&comp, missing(&base));
    d.config_reloaded().await;
    assert_eq!(reloads(&comp), 0, "the file is not back yet");
    tokio::time::sleep(WATCH_EVERY * 3).await;
    assert_eq!(reloads(&comp), 0, "still missing");
    std::fs::write(&base, "").unwrap();
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reloads(&comp), 1, "back: reloaded once");
    tokio::time::sleep(WATCH_FOR).await;
    assert_eq!(reloads(&comp), 1, "and not again");
}

#[tokio::test(start_paused = true)]
async fn waiting_for_a_file_gives_up_and_a_second_wait_does_not_double_up() {
    let (dir, mut d, comp) = desktop();
    set_errors(&comp, missing(&dir.path().join("never.lua")));
    d.config_reloaded().await;
    d.config_reloaded().await;
    assert!(d.recover.watching.load(std::sync::atomic::Ordering::SeqCst));
    tokio::time::sleep(WATCH_FOR + WATCH_EVERY * 2).await;
    assert_eq!(reloads(&comp), 0);
    assert!(
        !d.recover.watching.load(std::sync::atomic::Ordering::SeqCst),
        "gave up"
    );
}
