//! uQ's base config plus every generated variant must load.
//!
//! - `lua_mock_runs_every_variant` runs the files under a stock Lua interpreter with a mock `hl`
//!   table: catches syntax and runtime errors without Hyprland installed.
//! - `all_variants_pass_hyprland_verify` (ignored by default) runs `Hyprland --verify-config`;
//!   CI runs it with `--ignored` in an Arch container that has Hyprland.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uq_core::lua::{Layout, emit_generated};
use uq_core::paths::Paths;
use uq_core::stub::stub_contents;
use uq_core::theme::ThemeRegistry;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn layouts() -> BTreeMap<String, Layout> {
    BTreeMap::from([
        ("1".into(), Layout::Master),
        ("2".into(), Layout::Scrolling),
        ("web".into(), Layout::Dwindle),
    ])
}

/// Lay out a uQ config tree for one theme variant; returns (paths, main config file).
fn tree(root: &Path, generated: &str) -> (Paths, PathBuf) {
    let p = Paths::under(root);
    std::fs::create_dir_all(p.base_lua.parent().unwrap()).unwrap();
    std::fs::copy(repo().join("config/hypr/base.lua"), &p.base_lua).unwrap();
    std::fs::create_dir_all(p.generated_lua.parent().unwrap()).unwrap();
    std::fs::write(&p.generated_lua, generated).unwrap();
    std::fs::write(&p.user_lua, "").unwrap();
    std::fs::write(&p.main_lua, stub_contents(&p)).unwrap();
    let main = p.main_lua.clone();
    (p, main)
}

const MOCK: &str = r#"
local calls = {}
local function proxy(name)
  return setmetatable({}, {
    __index = function(_, k) return proxy(name .. "." .. k) end,
    __call = function(_, ...) calls[#calls + 1] = name; return proxy(name .. "()") end,
  })
end
hl = proxy("hl")
for i = 1, #arg do dofile(arg[i]) end
local seen = {}
for _, c in ipairs(calls) do seen[c] = true end
for _, want in ipairs({"hl.config", "hl.bind", "hl.on", "hl.workspace_rule"}) do
  if not seen[want] then error("never called " .. want) end
end
print("ok " .. #calls)
"#;

#[test]
fn lua_mock_runs_every_variant() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, MOCK).unwrap();
    for t in reg.iter() {
        let root = dir.path().join(format!("{}-{}", t.id, t.variant));
        let (p, _) = tree(&root, &emit_generated(t, &layouts()));
        let out = Command::new(&lua)
            .arg(&mock)
            .args([&p.base_lua, &p.generated_lua, &p.user_lua])
            .output()
            .unwrap();
        assert!(
            out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("ok "),
            "{} {}: {}{}",
            t.id,
            t.variant,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn all_variants_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for t in reg.iter() {
        let root = dir.path().join(format!("{}-{}", t.id, t.variant));
        let (_, main) = tree(&root, &emit_generated(t, &layouts()));
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
        assert!(out.status.success(), "{} {}: {text}", t.id, t.variant);
    }
}

fn which(bin: &str) -> Result<PathBuf, ()> {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(bin))
                .find(|p| p.is_file())
        })
        .ok_or(())
}
