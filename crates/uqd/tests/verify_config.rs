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
-- What each returned proxy stands for, e.g. `hl.dsp.global(uq:spotlight)`, and every hl.bind
-- call as {key, dsp, opts}.
local desc = setmetatable({}, { __mode = "k" })
local binds = {}
local function proxy(name)
  return setmetatable({}, {
    __index = function(_, k) return proxy(name .. "." .. k) end,
    __call = function(_, ...)
      calls[#calls + 1] = name
      local args = {...}
      local first = args[1]
      local strs = {}
      if name == "hl.bind" then
        binds[#binds + 1] = { key = args[1], dsp = desc[args[2]] or tostring(args[2]), opts = args[3] }
      end
      for _, a in ipairs(args) do
        if type(a) == "string" then
          strs[#strs + 1] = a
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
      local ret = proxy(name .. "()")
      desc[ret] = name .. "(" .. table.concat(strs, ",") .. ")"
      return ret
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
  {"hyprland.start", "XDG_SESSION_ID"},
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
local function bind_for(dsp)
  for _, b in ipairs(binds) do
    if b.dsp == dsp then return b end
  end
  error("no bind runs " .. dsp)
end
local function opt(b, k) return type(b.opts) == "table" and b.opts[k] or nil end
local spotlight = bind_for("hl.dsp.global(uq:spotlight)")
if spotlight.key ~= "SUPER + Space" then error("uq:spotlight is bound to " .. tostring(spotlight.key)) end
local appmenu = bind_for("hl.dsp.global(uq:appmenu)")
if appmenu.key ~= "SUPER + SUPER_L" then error("uq:appmenu is bound to " .. tostring(appmenu.key)) end
if opt(appmenu, "release") ~= true then error("the uq:appmenu bind needs release = true (tap SUPER)") end
local quick = bind_for("hl.dsp.global(uq:quick-settings)")
if quick.key ~= "SUPER + A" then error("uq:quick-settings is bound to " .. tostring(quick.key)) end
local notif = bind_for("hl.dsp.global(uq:notif-centre)")
if notif.key ~= "SUPER + N" then error("uq:notif-centre is bound to " .. tostring(notif.key)) end
local lock = bind_for("hl.dsp.global(uq:lock)")
if lock.key ~= "SUPER + L" then error("uq:lock is bound to " .. tostring(lock.key)) end
for _, b in ipairs(binds) do
  if b.key == "SUPER + L" and b ~= lock then error("SUPER + L is also bound to " .. b.dsp) end
end
local power = bind_for("hl.dsp.global(uq:power-menu)")
if power.key ~= "SUPER + Escape" then error("uq:power-menu is bound to " .. tostring(power.key)) end
for _, b in ipairs(binds) do
  if b.key == "SUPER + Escape" and b ~= power then error("SUPER + Escape is also bound to " .. b.dsp) end
end
for _, b in ipairs({spotlight, appmenu, quick, notif, lock, power}) do
  if opt(b, "locked") then error(b.dsp .. " must not be locked") end
end
local osd_keys = {
  XF86AudioRaiseVolume = "vol-up", XF86AudioLowerVolume = "vol-down",
  XF86AudioMute = "mute", XF86AudioMicMute = "mic-mute",
  XF86MonBrightnessUp = "bright-up", XF86MonBrightnessDown = "bright-down",
}
for key, name in pairs(osd_keys) do
  local b = bind_for("hl.dsp.global(uq:" .. name .. ")")
  if b.key ~= key then error("uq:" .. name .. " is bound to " .. tostring(b.key)) end
  if opt(b, "locked") ~= true then error(b.dsp .. " needs locked = true") end
  -- Level keys repeat while held; mute keys must not, or holding one flaps the mute.
  local toggles = name == "mute" or name == "mic-mute"
  if (opt(b, "repeating") == true) == toggles then
    error(b.dsp .. (toggles and " must not repeat" or " needs repeating = true"))
  end
end
for _, b in ipairs(binds) do
  if b.dsp:find("wpctl", 1, true) or b.dsp:find("brightnessctl", 1, true) then
    error("a wpctl/brightnessctl exec bind is left: " .. b.dsp)
  end
end
print("ok " .. #calls)
"#;

/// Spec §2.12: the launcher binds never work over the lock screen.
#[test]
fn base_lua_launcher_binds_are_not_locked() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for name in [
        "uq:spotlight",
        "uq:appmenu",
        "uq:quick-settings",
        "uq:notif-centre",
        "uq:lock",
        "uq:power-menu",
    ] {
        let lines: Vec<&str> = base.lines().filter(|l| l.contains(name)).collect();
        assert!(!lines.is_empty(), "base.lua has no {name} bind");
        for line in lines {
            assert!(
                !line.contains("locked"),
                "{name} bind must not be locked: {line}"
            );
        }
    }
}

/// Spec §2.12: the OSD keys go through the shell, locked (level keys repeating, mute toggles not),
/// with no exec binds left.
#[test]
fn base_lua_osd_binds_are_locked() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for (key, name) in [
        ("XF86AudioRaiseVolume", "uq:vol-up"),
        ("XF86AudioLowerVolume", "uq:vol-down"),
        ("XF86AudioMute", "uq:mute"),
        ("XF86AudioMicMute", "uq:mic-mute"),
        ("XF86MonBrightnessUp", "uq:bright-up"),
        ("XF86MonBrightnessDown", "uq:bright-down"),
    ] {
        let lines: Vec<&str> = base.lines().filter(|l| l.contains(name)).collect();
        assert_eq!(lines.len(), 1, "expected exactly one {name} bind");
        let line = lines[0];
        assert!(line.contains(key), "{name} is not on {key}: {line}");
        assert!(
            line.contains("locked = true"),
            "{name} bind must be locked: {line}"
        );
        // Level keys repeat while held; the mute toggles must not, or holding one flaps it.
        let toggle = name.ends_with("mute");
        assert_eq!(
            line.contains("repeating = true"),
            !toggle,
            "{name} bind: repeating must be {}: {line}",
            !toggle
        );
    }
    for line in base.lines() {
        assert!(
            !(line.contains("wpctl") || line.contains("brightnessctl")),
            "exec bind left in base.lua: {line}"
        );
    }
}

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
