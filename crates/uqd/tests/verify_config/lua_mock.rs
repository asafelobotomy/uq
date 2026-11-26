//! The mock `hl` table base.lua and the generated files run under, with a stock Lua.

use super::*;

/// Every `hl.bind` option Hyprland reads (`hypr::ALL_BIND_OPTIONS`) is injected at
/// `__KNOWN_OPTS__`: any other key is silently ignored by Hyprland, so the mock fails on it.
pub(crate) const MOCK_TEMPLATE: &str = r#"
local KNOWN_OPTS = __KNOWN_OPTS__
-- The `hl.gesture` fields and directions Hyprland reads (`gestures::hypr`): any other key is
-- silently ignored by Hyprland, and an unknown direction is a config error.
local GESTURE_FIELDS = __GESTURE_FIELDS__
local GESTURE_DIRS = __GESTURE_DIRS__
local calls = {}
-- exec_cmd strings seen while a given hl.on("<event>", fn) callback runs.
local hook, in_hook = nil, {}
-- base.lua's shutdown hook stops the shell with os.execute (it must block): record it, never run it.
local executed = {}
os.execute = function(cmd)
  cmd = tostring(cmd)
  calls[#calls + 1] = "os.execute:" .. cmd
  if hook then
    in_hook[hook] = (in_hook[hook] or "") .. cmd .. "\n"
    executed[hook] = (executed[hook] or "") .. cmd .. "\n"
  end
  return true, "exit", 0
end
-- What each returned proxy stands for, e.g. `hl.dsp.global(uq:lookseedo)`, and every hl.bind
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
      -- Table arguments count too (`{ workspace = 3 }`), as sorted k=v pairs, so two binds that
      -- differ only in a table compare unequal.
      local function show(t)
        local kv = {}
        for k, v in pairs(t) do
          if type(v) == "string" or type(v) == "number" or type(v) == "boolean" then
            kv[#kv + 1] = tostring(k) .. "=" .. tostring(v)
          end
        end
        table.sort(kv)
        return "{" .. table.concat(kv, ",") .. "}"
      end
      local tbls = ""
      if name ~= "hl.bind" and name ~= "hl.config" then
        for _, a in ipairs(args) do
          if type(a) == "table" and not desc[a] and getmetatable(a) == nil then tbls = tbls .. show(a) end
        end
      end
      if name == "hl.gesture" then
        local t = args[1]
        if type(t) ~= "table" then error("hl.gesture takes one table") end
        for k in pairs(t) do
          local known = false
          for _, n in ipairs(GESTURE_FIELDS) do if n == k then known = true end end
          if not known or tostring(k):find("-", 1, true) then
            error("unknown hl.gesture field " .. tostring(k))
          end
        end
        local dir_ok = false
        for _, d in ipairs(GESTURE_DIRS) do if d == t.direction then dir_ok = true end end
        if not dir_ok then error("unknown hl.gesture direction " .. tostring(t.direction)) end
        if math.type(t.fingers) ~= "integer" or t.fingers < 2 or t.fingers > 9 then
          error("hl.gesture fingers must be a whole number from 2 to 9")
        end
        if t.mods ~= nil and type(t.mods) ~= "string" then error("hl.gesture mods must be text") end
        local a = t.action
        if type(a) == "function" then
          a()
        elseif type(a) == "table" then
          -- A returned `hl.dsp.*` proxy is a dispatcher object at Hyprland 0.57, which
          -- `hlGesture` refuses; only a function (or a plain table of functions) works.
          if getmetatable(a) ~= nil then
            error("hl.gesture action is a dispatcher object: wrap it in a function")
          end
        elseif type(a) ~= "string" then
          error("hl.gesture needs an action")
        end
      end
      -- generated.lua starts its shortcuts with hl.unbind("all"): base.lua's fallback copies go.
      if name == "hl.unbind" and args[1] == "all" then binds = {} end
      if name == "hl.bind" then
        local opts = args[3]
        if type(opts) == "table" then
          for k in pairs(opts) do
            local known = false
            for _, n in ipairs(KNOWN_OPTS) do if n == k then known = true end end
            if not known or tostring(k):find("-", 1, true) then
              error("unknown hl.bind option " .. tostring(k))
            end
          end
        end
        binds[#binds + 1] = { key = args[1], dsp = desc[args[2]] or tostring(args[2]), opts = opts }
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
      if #strs == 0 and tbls ~= "" then
        desc[ret] = name .. tbls
      else
        desc[ret] = name .. "(" .. table.concat(strs, ",") .. ")" .. tbls
      end
      return ret
    end,
  })
end
hl = proxy("hl")
local only = arg[1] == "--binds-only"
for i = only and 2 or 1, #arg do dofile(arg[i]) end
-- `bind <key> | <dispatcher> | <sorted options, without the description>`, in order.
local function print_binds()
  for _, b in ipairs(binds) do
    local kv = {}
    if type(b.opts) == "table" then
      for k, v in pairs(b.opts) do
        if k ~= "description" and k ~= "desc" then kv[#kv + 1] = tostring(k) .. "=" .. tostring(v) end
      end
    end
    table.sort(kv)
    print("bind " .. tostring(b.key) .. " | " .. b.dsp .. " | " .. table.concat(kv, ","))
  end
end
if only then
  print("ok binds")
  print_binds()
  return
end
local seen = {}
for _, c in ipairs(calls) do seen[c] = true end
for _, want in ipairs({"hl.config", "hl.bind", "hl.on", "hl.workspace_rule", "hl.monitor", "hl.device"}) do
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
  {"hyprland.start", "is-enabled scdr-session.service && systemctl --user restart scdr-session.service"},
  {"hyprland.shutdown", "stop scdr-session.service"},
  {"hyprland.start", 'test -n "$UQ_DEV_SESSION" || {'},
  {"hyprland.shutdown", 'test -n "$UQ_DEV_SESSION" || {'},
}) do
  if not (in_hook[want[1]] or ""):find(want[2], 1, true) then
    error("missing in " .. want[1] .. " hook: " .. want[2])
  end
end
-- The shell and uqlip are stopped and waited for before Hyprland closes its socket.
if not (executed["hyprland.shutdown"] or ""):find("systemctl --user stop uq-shell.service uqlip.service", 1, true) then
  error("the shutdown hook must stop uq-shell and uqlip with os.execute (blocking), not exec_cmd")
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
local lookseedo = bind_for("hl.dsp.global(uq:lookseedo)")
if lookseedo.key ~= "SUPER + Space" then error("uq:lookseedo is bound to " .. tostring(lookseedo.key)) end
local emoji = bind_for("hl.dsp.global(uq:emoji)")
if emoji.key ~= "SUPER + period" then error("uq:emoji is bound to " .. tostring(emoji.key)) end
for _, b in ipairs(binds) do
  if b.key == "SUPER + period" and b ~= emoji then error("SUPER + period is also bound to " .. b.dsp) end
end
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
for _, b in ipairs({lookseedo, emoji, appmenu, quick, notif, lock, power}) do
  if opt(b, "locked") then error(b.dsp .. " must not be locked") end
end
-- Nor may any other launcher bind, such as one of your own.
for _, b in ipairs(binds) do
  for _, n in ipairs({"lookseedo", "appmenu", "quick-settings", "notif-centre", "lock", "power-menu"}) do
    if b.dsp == "hl.dsp.global(uq:" .. n .. ")" and opt(b, "locked") then
      error(b.key .. " opens " .. n .. " over the lock screen")
    end
  end
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
for _, c in ipairs(calls) do print(c) end
print_binds()
"#;

/// The mock with the option whitelists filled in.
pub(crate) fn mock() -> String {
    let quoted = |names: Vec<&str>| {
        let list: Vec<String> = names.iter().map(|o| format!("{o:?}")).collect();
        format!("{{{}}}", list.join(", "))
    };
    MOCK_TEMPLATE
        .replace("__KNOWN_OPTS__", &quoted(hypr::ALL_BIND_OPTIONS.to_vec()))
        .replace(
            "__GESTURE_FIELDS__",
            &quoted(gestures::hypr::ALL_FIELDS.to_vec()),
        )
        .replace(
            "__GESTURE_DIRS__",
            &quoted(gestures::hypr::DIRECTIONS.iter().map(|d| d.lua).collect()),
        )
}

/// Runs the mock over `files` and returns (success, stdout, stderr).
pub(crate) fn run_mock(
    lua: &Path,
    dir: &Path,
    binds_only: bool,
    files: &[&Path],
) -> (bool, String, String) {
    let m = dir.join("mock.lua");
    std::fs::write(&m, mock()).unwrap();
    let mut cmd = Command::new(lua);
    cmd.arg(&m);
    if binds_only {
        cmd.arg("--binds-only");
    }
    let out = cmd.args(files).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}
