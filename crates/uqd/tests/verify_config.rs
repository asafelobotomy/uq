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
use uq_core::settings::{AccelProfile, MouseSettings};
use uq_core::stub::stub_contents;
use uq_core::theme::ThemeRegistry;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every mouse key away from its default, so the emitted `input` block is fully exercised.
fn mouse() -> MouseSettings {
    MouseSettings {
        sensitivity: -0.25,
        accel_profile: AccelProfile::Flat,
        natural_scroll: true,
        left_handed: true,
        scroll_factor: 1.5,
    }
}

fn layouts() -> BTreeMap<String, Layout> {
    BTreeMap::from([
        ("1".into(), Layout::Master),
        ("2".into(), Layout::Scrolling),
        ("name:web".into(), Layout::Dwindle),
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
-- exec_cmd strings seen while a given hl.on("<event>", fn) callback runs.
local hook, in_hook = nil, {}
local function proxy(name)
  return setmetatable({}, {
    __index = function(_, k) return proxy(name .. "." .. k) end,
    __call = function(_, ...)
      calls[#calls + 1] = name
      local first = ({...})[1]
      for _, a in ipairs({...}) do
        if type(a) == "string" then
          calls[#calls + 1] = name .. ":" .. a
          if hook and name == "hl.exec_cmd" then in_hook[hook] = (in_hook[hook] or "") .. a .. "\n" end
        end
        if type(a) == "function" then
          local outer = hook
          if name == "hl.on" then hook = first end
          a()
          hook = outer
        end
      end
      return proxy(name .. "()")
    end,
  })
end
hl = proxy("hl")
for i = 1, #arg do dofile(arg[i]) end
local seen = {}
for _, c in ipairs(calls) do seen[c] = true end
for _, want in ipairs({"hl.config", "hl.bind", "hl.on", "hl.workspace_rule"}) do
  if not seen[want] then error("never called " .. want) end
end
local function called(prefix, needle)
  for _, c in ipairs(calls) do
    if c:sub(1, #prefix) == prefix and c:find(needle, 1, true) then return true end
  end
  return false
end
for _, want in ipairs({
  {"hl.on:", "hyprland.start"}, {"hl.on:", "hyprland.shutdown"},
  {"hl.exec_cmd:", "systemctl --user restart uqd.service"},
  {"hl.exec_cmd:", "systemctl --user stop uqd.service"},
}) do
  if not called(want[1], want[2]) then error("missing " .. want[1] .. want[2]) end
end
for _, want in ipairs({
  {"hyprland.start", "systemctl --user restart uqd.service"},
  {"hyprland.start", "systemctl --user restart uq-shell.service"},
  {"hyprland.start", "XDG_SESSION_TYPE"},
  {"hyprland.shutdown", "systemctl --user stop uqd.service"},
  {"hyprland.shutdown", "stop uq-shell.service"},
}) do
  if not (in_hook[want[1]] or ""):find(want[2], 1, true) then
    error("missing in " .. want[1] .. " hook: " .. want[2])
  end
end
local start = in_hook["hyprland.start"] or ""
if start:find("uqd.service &&", 1, true) or start:find("&& systemctl --user restart uq-shell", 1, true) then
  error("the shell must start even when uqd fails: no && before the uq-shell restart")
end
if not called("hl.dsp.exec_cmd:", "systemctl --user restart uq-lock.service || hyprlock") then
  error("missing the SUPER+CTRL+L recovery bind")
end
print("ok " .. #calls)
"#;

#[test]
fn base_lua_enables_session_lock_restore() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for needle in [
        "allow_session_lock_restore = true",
        "session_lock_xray",
        "key_press_enables_dpms",
        "mouse_move_enables_dpms",
    ] {
        assert!(base.contains(needle), "base.lua lacks {needle}");
    }
}

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
        let (p, _) = tree(&root, &emit_generated(t, &layouts(), &mouse()));
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
        let (_, main) = tree(&root, &emit_generated(t, &layouts(), &mouse()));
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
