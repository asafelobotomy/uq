//! The greeter's Hyprland config.

use super::*;

/// uq-greeter's fixed Hyprland config (greeter/hyprland.lua), behind the wrapper that
/// packaging/uq-greeter-session writes (the fixed file, then the localectl keyboard layout).
pub(crate) fn greeter_tree(dir: &Path) -> PathBuf {
    let wrapper = dir.join("hyprland.lua");
    let fixed = repo().join("greeter/hyprland.lua").canonicalize().unwrap();
    std::fs::write(
        &wrapper,
        format!(
            "require(\"{}\")\nhl.config({{ input = {{ kb_layout = \"de,us\", kb_variant = \"nodeadkeys,\" }} }})\n",
            fixed.display()
        ),
    )
    .unwrap();
    wrapper
}

#[test]
pub(crate) fn greeter_config_runs_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, GREETER_MOCK).unwrap();
    let out = Command::new(&lua)
        .arg(&mock)
        .arg(repo().join("greeter/hyprland.lua"))
        .output()
        .unwrap();
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("ok"),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
pub(crate) fn greeter_config_passes_hyprland_verify() {
    let dir = tempfile::tempdir().unwrap();
    let main = greeter_tree(dir.path());
    let out = Command::new("Hyprland")
        .arg("--verify-config")
        .arg("-c")
        .arg(&main)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
}
