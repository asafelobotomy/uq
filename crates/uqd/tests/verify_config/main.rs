//! uQ's base config plus every generated variant must load.
//!
//! - `lua_mock_runs_every_variant` runs the files under a stock Lua interpreter with a mock `hl`
//!   table: catches syntax and runtime errors without Hyprland installed.
//! - `all_variants_pass_hyprland_verify` (ignored by default) runs `Hyprland --verify-config`;
//!   CI runs it with `--ignored` in an Arch container that has Hyprland.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uq_core::gestures::{self, Gesture};
use uq_core::keybinds::Keybinds;
use uq_core::keybinds::hypr::{self, SHELL_NAMES};
use uq_core::keybinds::keys::{Group, KEYS, KeyKind};
use uq_core::lua::{
    Layout, MonitorRender, StartupRender, emit_generated, emit_generated_with_apps,
};
use uq_core::rules::{
    LayerEffects, LayerMatch, LayerRule, Rules, WindowEffects, WindowMatch, WindowRule,
    WorkspaceRule,
};
use uq_core::settings::{
    AccelProfile, DeviceOverride, ForceSplit, KbLayout, MouseSettings, NewStatus, Orientation,
    OutputConfig, Position, Settings,
};
use uq_core::theme::ThemeRegistry;
mod inputs;
use inputs::*;
mod lua_mock;
use lua_mock::*;

#[path = "../common/mod.rs"]
mod common;
use common::verify::{repo, tree, which};

fn bind_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.starts_with("bind ")).collect()
}

const GREETER_MOCK: &str = r#"
local calls, hook, in_hook, configs = {}, nil, {}, {}
local function proxy(name)
  return setmetatable({}, {
    __index = function(_, k) return proxy(name .. "." .. k) end,
    __call = function(_, ...)
      calls[#calls + 1] = name
      for _, a in ipairs({...}) do
        if type(a) == "string" and hook and name == "hl.exec_cmd" then
          in_hook[hook] = (in_hook[hook] or "") .. a .. "\n"
        end
        if name == "hl.config" and type(a) == "table" then configs[#configs + 1] = a end
        if type(a) == "function" then
          local outer = hook
          if name == "hl.on" then hook = ({...})[1] end
          a()
          hook = outer
        end
      end
      return proxy(name .. "()")
    end,
  })
end
hl = proxy("hl")
dofile(arg[1])
for _, c in ipairs(calls) do
  if c == "hl.bind" or c:sub(1, 7) == "hl.dsp." then error("the greeter config must have no binds: " .. c) end
end
local start = in_hook["hyprland.start"] or ""
if not start:find("/usr/lib/uq/uq-greeter-session --shell", 1, true) then
  error("hyprland.start must run uq-greeter-session --shell, got: " .. start)
end
local anim, watchdog = nil, nil
for _, c in ipairs(configs) do
  if c.animations then anim = c.animations.enabled end
  if c.misc then watchdog = c.misc.disable_watchdog_warning end
end
if anim ~= false then error("animations must be off") end
if watchdog ~= true then error("disable_watchdog_warning must be set") end
print("ok")
"#;

mod base_lua;
mod binds;
mod gesture_config;
mod greeter;
mod mock;
mod rules;
mod startup;
