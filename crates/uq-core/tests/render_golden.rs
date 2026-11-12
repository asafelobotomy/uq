use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uq_core::atomic::{atomic_replace_dir, atomic_write};
use uq_core::render::{RenderedFiles, render};
use uq_core::theme::{ThemeRegistry, Variant};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> ThemeRegistry {
    ThemeRegistry::load_dir(&repo().join("themes")).unwrap()
}

const EXPECTED_FILES: [&str; 9] = [
    "gtk-3.0/gtk.css",
    "gtk-4.0/gtk.css",
    "kde/uq.colors",
    "qt6ct/colors/uq.conf",
    "shell.env",
    "terminal/alacritty.toml",
    "terminal/foot.ini",
    "terminal/kitty.conf",
    "tokens.json",
];

#[test]
fn golden_all_variants() {
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let bless = std::env::var_os("UQ_BLESS").is_some();
    let mut mismatches = Vec::new();
    for t in registry().iter() {
        let out = render(t, &repo().join("templates"), None).unwrap();
        let names: Vec<_> = out
            .0
            .keys()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, EXPECTED_FILES, "{} {}", t.id, t.variant);
        let dir = golden.join(format!("{}-{}", t.id, t.variant));
        for (rel, body) in &out.0 {
            let path = dir.join(rel);
            if bless {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, body).unwrap();
            } else if fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
                mismatches.push(path.display().to_string());
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "golden mismatch (UQ_BLESS=1 to update):\n{}",
        mismatches.join("\n")
    );
}

/// The greeter's pre-rendered `tokens.json` files (`uqctl render-themes`, run at package build)
/// are byte-for-byte what uqd renders for that variant with default settings, for every variant.
#[test]
fn prerendered_tokens_equal_uqd_render() {
    let reg = registry();
    let out = tempfile::tempdir().unwrap();
    let written =
        uq_core::render::prerender_tokens(&reg, &repo().join("templates"), out.path()).unwrap();
    assert_eq!(written.len(), reg.iter().count());
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for t in reg.iter() {
        let name = format!("{}-{}", t.id, t.variant);
        let pre = fs::read_to_string(out.path().join(&name).join("tokens.json")).unwrap();
        // uqd's render_candidate: the theme with the (default) appearance applied.
        let theme = t.with_appearance(&uq_core::settings::Appearance::default());
        let uqd = render(&theme, &repo().join("templates"), None).unwrap();
        assert_eq!(pre, uqd.0[Path::new("tokens.json")], "{name}");
        let gold = fs::read_to_string(golden.join(&name).join("tokens.json")).unwrap();
        assert_eq!(pre, gold, "{name}");
    }
    // Nothing else is written, and the themes directory still loads with it inside.
    let dirs = fs::read_dir(out.path()).unwrap().count();
    assert_eq!(dirs, reg.iter().count());
}

#[test]
fn tokens_json_round_trips() {
    let reg = registry();
    let t = reg.get("graphite", Variant::Dark).unwrap();
    let out = render(t, &repo().join("templates"), None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out.0[Path::new("tokens.json")]).unwrap();
    assert_eq!(v["colors"]["accent"], t.colors.accent.hex());
    assert_eq!(v["variant"], "dark");
    assert_eq!(v["id"], "graphite");
    assert_eq!(v["terminal"]["ansi"].as_array().unwrap().len(), 16);
}

#[test]
fn foot_uses_bare_hex() {
    let reg = registry();
    let t = reg.get("gruvbox", Variant::Dark).unwrap();
    let out = render(t, &repo().join("templates"), None).unwrap();
    assert!(out.0[Path::new("terminal/foot.ini")].contains("background=282828"));
}

#[test]
fn missing_template_is_an_error() {
    let reg = registry();
    let t = reg.get("gruvbox", Variant::Dark).unwrap();
    let empty = tempfile::tempdir().unwrap();
    assert!(render(t, empty.path(), None).is_err());
}

fn files(pairs: &[(&str, &str)]) -> RenderedFiles {
    RenderedFiles(
        pairs
            .iter()
            .map(|(k, v)| (PathBuf::from(k), v.to_string()))
            .collect(),
    )
}

#[test]
fn atomic_replace_swaps_whole_dir() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("theme");
    atomic_replace_dir(&target, &files(&[("a.txt", "one"), ("sub/b.txt", "two")])).unwrap();
    atomic_replace_dir(&target, &files(&[("a.txt", "three")])).unwrap();
    assert_eq!(fs::read_to_string(target.join("a.txt")).unwrap(), "three");
    assert!(!target.join("sub/b.txt").exists(), "stale file survived");
    let leftovers: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(leftovers, vec!["theme"]);
}

#[test]
fn atomic_replace_leaves_target_on_failure() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("p");
    let target = parent.join("theme");
    atomic_replace_dir(&target, &files(&[("a.txt", "original")])).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    let res = atomic_replace_dir(&target, &files(&[("a.txt", "new")]));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(res.is_err());
    assert_eq!(
        fs::read_to_string(target.join("a.txt")).unwrap(),
        "original"
    );
}

#[test]
fn atomic_write_replaces_file() {
    let root = tempfile::tempdir().unwrap();
    let p = root.path().join("x/state.toml");
    atomic_write(&p, "a").unwrap();
    atomic_write(&p, "b").unwrap();
    assert_eq!(fs::read_to_string(&p).unwrap(), "b");
    assert_eq!(fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn concurrent_atomic_writes_never_share_a_temp_file() {
    let root = tempfile::tempdir().unwrap();
    let p = root.path().join("state.toml");
    let bodies: Vec<String> = (0..8)
        .map(|i| format!("writer {i}\n").repeat(4096))
        .collect();
    std::thread::scope(|s| {
        for body in &bodies {
            let p = &p;
            s.spawn(move || {
                for _ in 0..20 {
                    atomic_write(p, body).unwrap();
                }
            });
        }
    });
    // The file is exactly one writer's whole body, and no temp file is left behind.
    let got = fs::read_to_string(&p).unwrap();
    assert!(bodies.contains(&got), "a mixed or torn write");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn atomic_write_replaces_a_symlink_not_its_target() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("dotfiles.toml");
    fs::write(&real, "kept").unwrap();
    let link = root.path().join("state.toml");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    atomic_write(&link, "new").unwrap();
    assert!(
        !fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&link).unwrap(), "new");
    assert_eq!(fs::read_to_string(&real).unwrap(), "kept");
}
