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

mod common;
use common::verify::{repo, tree, which};

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

/// Every look and layout option away from "unset", so CI's `--verify-config` checks each name.
fn settings() -> Settings {
    let mut s = Settings {
        mouse: mouse(),
        ..Settings::default()
    };
    s.look.gaps_in = Some(7);
    s.look.gaps_out = Some(18);
    s.look.border_width = Some(3);
    s.look.rounding = Some(12);
    s.look.blur = Some(true);
    s.look.blur_size = Some(5);
    s.look.blur_passes = Some(3);
    s.look.shadow = Some(false);
    s.look.animations = Some(true);
    s.look.animation_speed = Some(1.5);
    let lo = &mut s.layout_options;
    lo.default = Some(Layout::Scrolling);
    lo.dwindle.preserve_split = Some(true);
    lo.dwindle.smart_split = Some(true);
    lo.dwindle.force_split = Some(ForceSplit::Left);
    lo.master.mfact = Some(0.65);
    lo.master.new_status = Some(NewStatus::Slave);
    lo.master.orientation = Some(Orientation::Right);
    lo.scrolling.column_width = Some(0.6);
    lo.scrolling.fullscreen_on_one_column = Some(true);
    // Every monitor, keyboard, touchpad and device option away from "unset".
    s.monitors.outputs.insert(
        "DP-1".into(),
        OutputConfig {
            mode: Some("2560x1440@144".into()),
            position: Some(Position { x: 0, y: 0 }),
            scale: Some(1.25),
            transform: Some(1),
            enabled: Some(true),
            vrr: Some(1),
            ..Default::default()
        },
    );
    s.monitors.outputs.insert(
        "HDMI-A-1".into(),
        OutputConfig {
            mirror_of: Some("DP-1".into()),
            ..Default::default()
        },
    );
    s.monitors.outputs.insert(
        "eDP-1".into(),
        OutputConfig {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let k = &mut s.input.keyboard;
    k.layouts = vec![
        KbLayout {
            layout: "us".into(),
            variant: None,
        },
        KbLayout {
            layout: "de".into(),
            variant: Some("nodeadkeys".into()),
        },
    ];
    k.switch_combo = Some("grp:alt_shift_toggle".into());
    k.repeat_rate = Some(35);
    k.repeat_delay = Some(250);
    let t = &mut s.input.touchpad;
    t.natural_scroll = Some(true);
    t.tap_to_click = Some(false);
    t.disable_while_typing = Some(true);
    t.scroll_factor = Some(0.8);
    s.input.devices.insert(
        "logitech-g-pro".into(),
        DeviceOverride {
            sensitivity: Some(-0.4),
            accel_profile: Some(AccelProfile::Flat),
            natural_scroll: Some(false),
            left_handed: Some(true),
            scroll_factor: Some(1.5),
        },
    );
    s
}

fn layouts() -> BTreeMap<String, Layout> {
    BTreeMap::from([
        ("1".into(), Layout::Master),
        ("2".into(), Layout::Scrolling),
        ("name:web".into(), Layout::Dwindle),
    ])
}

/// One window rule using every effect (split where they contradict), one panel rule with every
/// effect and one workspace rule with every field.
fn rules() -> Rules {
    let m = |class: &str| WindowMatch {
        class: Some(class.into()),
        ..Default::default()
    };
    let all_match = WindowMatch {
        class: Some("^(a|b)$".into()),
        title: Some("t.*".into()),
        initial_class: Some("c".into()),
        initial_title: Some("d".into()),
        tag: Some("web".into()),
        workspace: Some("2".into()),
        xwayland: Some(false),
        floating: Some(true),
        fullscreen: Some(false),
        pinned: Some(false),
        modal: Some(false),
    };
    let rule = |id: &str, enabled, matches, effects| WindowRule {
        id: id.into(),
        name: format!("Rule {id}"),
        enabled,
        matches,
        effects,
    };
    Rules {
        window: vec![
            rule(
                "r-0001",
                true,
                all_match,
                WindowEffects {
                    float: Some(true),
                    fullscreen: Some(false),
                    maximize: Some(false),
                    workspace: Some("name:web".into()),
                    silent: Some(true),
                    monitor: Some("DP-1".into()),
                    size: Some([800, 600]),
                    pin: Some(true),
                    no_initial_focus: Some(true),
                    opacity: Some(0.9),
                    no_blur: Some(true),
                    no_shadow: Some(true),
                    no_anim: Some(true),
                    rounding: Some(8),
                    border_size: Some(1),
                    idle_inhibit: Some("focus".into()),
                    keep_aspect_ratio: Some(true),
                    tag: Some("pinned".into()),
                    ..Default::default()
                },
            ),
            rule(
                "r-0002",
                false,
                m("other"),
                WindowEffects {
                    tile: Some(true),
                    position: Some([-10, 20]),
                    ..Default::default()
                },
            ),
            rule(
                "r-0003",
                true,
                m("centered"),
                WindowEffects {
                    center: Some(true),
                    ..Default::default()
                },
            ),
        ],
        layer: vec![LayerRule {
            id: "r-0a1b".into(),
            name: "Panel".into(),
            enabled: true,
            matches: LayerMatch {
                namespace: "^(noti-fi)$".into(),
            },
            effects: LayerEffects {
                blur: Some(true),
                ignore_alpha: Some(0.3),
                no_anim: Some(true),
                animation: Some("slide".into()),
            },
        }],
        workspace: vec![
            WorkspaceRule {
                workspace: "2".into(),
                monitor: Some("DP-1".into()),
                default: Some(true),
                persistent: Some(true),
                gaps_in: Some(3),
                gaps_out: Some(6),
                border_size: Some(2),
            },
            WorkspaceRule {
                workspace: "name:chat".into(),
                monitor: None,
                default: None,
                persistent: Some(true),
                gaps_in: None,
                gaps_out: None,
                border_size: None,
            },
        ],
    }
}

/// Rules whose strings try to break out of their Lua string (built directly, past validation).
fn hostile_rules() -> Rules {
    let bad = "a\"b\\c]]d\ne\") hl.exec_cmd(\"x";
    Rules {
        window: (0..3)
            .map(|i| WindowRule {
                id: format!("r-000{i}"),
                name: bad.into(),
                enabled: true,
                matches: WindowMatch {
                    class: Some(bad.into()),
                    title: Some(bad.into()),
                    ..Default::default()
                },
                effects: WindowEffects {
                    monitor: Some(bad.into()),
                    tag: Some(bad.into()),
                    ..Default::default()
                },
            })
            .collect(),
        layer: (0..2)
            .map(|i| LayerRule {
                id: format!("r-001{i}"),
                name: bad.into(),
                enabled: true,
                matches: LayerMatch {
                    namespace: bad.into(),
                },
                effects: LayerEffects {
                    animation: Some(bad.into()),
                    ..Default::default()
                },
            })
            .collect(),
        workspace: vec![WorkspaceRule {
            workspace: bad.into(),
            monitor: Some(bad.into()),
            default: None,
            persistent: None,
            gaps_in: None,
            gaps_out: None,
            border_size: None,
        }],
    }
}

/// A combo no default uses: three modifiers and the `i`th key of letters, digits and F1-F24.
fn free_combo(i: usize) -> String {
    let keys: Vec<&str> = KEYS
        .iter()
        .filter(|k| matches!(k.group, Group::Letter | Group::Digit | Group::Function))
        .map(|k| k.spelling)
        .collect();
    format!("SUPER + CTRL + ALT + {}", keys[i])
}

fn custom_bind(i: usize, name: &str, action: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"id": format!("k-{:04x}", i + 1), "name": name,
                       "combo": free_combo(i), "action": action})
}

/// Every default overridden or kept (the launcher and sound keys stay, as the checks below
/// look for them), `uq.profile.next` bound, and one shortcut of your own per action kind and
/// per value of each closed list.
fn keybinds() -> Keybinds {
    let mut custom = Vec::new();
    let mut add = |name: &str, action: serde_json::Value| {
        custom.push(custom_bind(custom.len(), name, action));
    };
    for n in SHELL_NAMES {
        add(n, serde_json::json!({"kind": "shell", "name": n}));
    }
    for n in ["close", "float", "pin"] {
        add(n, serde_json::json!({"kind": "window", "name": n}));
    }
    for d in ["left", "right", "up", "down"] {
        add(
            d,
            serde_json::json!({"kind": "window", "name": "focus", "direction": d}),
        );
    }
    for (n, w) in [("focus", 1), ("focus", 10), ("move", 1), ("move", 10)] {
        add(
            n,
            serde_json::json!({"kind": "workspace", "name": n, "workspace": w}),
        );
    }
    for n in ["toggle_special", "move_special", "next", "prev"] {
        add(n, serde_json::json!({"kind": "workspace", "name": n}));
    }
    for n in ["terminal", "exit", "profile-next", "settings"] {
        add(n, serde_json::json!({"kind": "uq", "name": n}));
    }
    for l in ["dwindle", "master", "scrolling"] {
        add(
            l,
            serde_json::json!({"kind": "uq", "name": "layout", "layout": l}),
        );
    }
    for m in ["next", "play-pause", "previous"] {
        add(
            m,
            serde_json::json!({"kind": "uq", "name": "media", "media": m}),
        );
    }
    add(
        "app",
        serde_json::json!({"kind": "launch", "app": "org.mozilla.firefox"}),
    );
    add(
        "cmd",
        serde_json::json!({"kind": "command", "command": "grim -g \"$(slurp)\" ~/it's.png"}),
    );
    // Your own volume key: locked and repeating, like the default.
    let mut vol = custom_bind(
        custom.len(),
        "Louder",
        serde_json::json!({"kind": "shell", "name": "vol-up"}),
    );
    vol["combo"] = "SUPER + F7".into();
    vol["locked"] = true.into();
    vol["repeating"] = true.into();
    custom.push(vol);
    let v = serde_json::json!({
        "overrides": [
            {"id": "uq.terminal", "combo": "SUPER + T"},
            {"id": "uq.close", "enabled": false},
            {"id": "uq.float", "combo": "SUPER + SHIFT + V"},
            {"id": "uq.ws.next", "combo": "SUPER + SHIFT + mouse_down"},
            {"id": "uq.window.drag", "combo": "ALT + mouse:272"},
            {"id": "uq.window.resize", "combo": "ALT + mouse:273"},
            {"id": "uq.profile.next", "combo": "SUPER + ALT + P"},
        ],
        "custom": custom,
    });
    Keybinds::from_section(&v, &Keybinds::default()).expect("the fixture shortcuts are valid")
}

/// One shortcut of your own per `KEYS` row a shortcut may use, each on its own free combo with
/// all four modifiers. The table is longer than the limit of 100, so it comes in chunks.
fn keybinds_keys() -> Vec<Keybinds> {
    let rows: Vec<_> = KEYS
        .iter()
        .filter(|k| matches!(k.kind, KeyKind::Key | KeyKind::Wheel))
        .collect();
    rows.chunks(60)
        .map(|chunk| {
            let custom: Vec<serde_json::Value> = chunk
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    serde_json::json!({"id": format!("k-{:04x}", i + 1), "name": k.label,
                        "combo": format!("SUPER + SHIFT + CTRL + ALT + {}", k.spelling),
                        "action": {"kind": "window", "name": "close"}})
                })
                .collect();
            Keybinds::from_section(
                &serde_json::json!({ "custom": custom }),
                &Keybinds::default(),
            )
            .expect("every key is valid")
        })
        .collect()
}

/// The fixtures every variant is rendered with. The mouse buttons and Super keys only exist as
/// defaults; `keybinds()` moves them.
fn fixtures() -> Vec<Keybinds> {
    let mut v = vec![keybinds()];
    v.extend(keybinds_keys());
    v
}

/// Every `hl.bind` option Hyprland reads (`hypr::ALL_BIND_OPTIONS`) is injected at
/// `__KNOWN_OPTS__`: any other key is silently ignored by Hyprland, so the mock fails on it.
const MOCK_TEMPLATE: &str = r#"
local KNOWN_OPTS = __KNOWN_OPTS__
-- The `hl.gesture` fields and directions Hyprland reads (`gestures::hypr`): any other key is
-- silently ignored by Hyprland, and an unknown direction is a config error.
local GESTURE_FIELDS = __GESTURE_FIELDS__
local GESTURE_DIRS = __GESTURE_DIRS__
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
for _, b in ipairs({spotlight, emoji, appmenu, quick, notif, lock, power}) do
  if opt(b, "locked") then error(b.dsp .. " must not be locked") end
end
-- Nor may any other launcher bind, such as one of your own.
for _, b in ipairs(binds) do
  for _, n in ipairs({"spotlight", "appmenu", "quick-settings", "notif-centre", "lock", "power-menu"}) do
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
fn mock() -> String {
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

/// Spec §2.12: the launcher binds never work over the lock screen.
#[test]
fn base_lua_launcher_binds_are_not_locked() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for name in [
        "uq:spotlight",
        "uq:emoji",
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
fn base_lua_has_no_animation_lines() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    for needle in ["hl.animation(", "hl.curve("] {
        assert!(!base.contains(needle), "base.lua still has {needle}");
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

/// Runs the mock over `files` and returns (success, stdout, stderr).
fn run_mock(lua: &Path, dir: &Path, binds_only: bool, files: &[&Path]) -> (bool, String, String) {
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

fn bind_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.starts_with("bind ")).collect()
}

/// R1, the spec's migration test, permanent: `base.lua`'s fallback copies are exactly what
/// uQ's default table renders. Both are run through the mock; the `(key, dispatcher, options)`
/// lists must be equal, in order.
#[test]
fn base_lua_binds_equal_the_default_table() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let rendered = dir.path().join("rendered.lua");
    std::fs::write(&rendered, uq_core::lua::emit_binds(&Keybinds::default())).unwrap();
    let base = repo().join("config/hypr/base.lua");
    let (ok, from_base, err) = run_mock(&lua, dir.path(), true, &[&base]);
    assert!(ok, "{from_base}{err}");
    let (ok, from_table, err) = run_mock(&lua, dir.path(), true, &[&rendered]);
    assert!(ok, "{from_table}{err}");
    let (a, b) = (bind_lines(&from_base), bind_lines(&from_table));
    assert_eq!(a.len(), 55, "{from_base}");
    assert_eq!(a, b, "base.lua and the default table have drifted apart");
}

/// Review Focus 2: Hyprland ignores an unknown `hl.bind` option, so the mock must not.
#[test]
fn mock_rejects_a_dashed_option() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("dashed.lua");
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + Q\", hl.dsp.window.close(), { [\"non-consuming\"] = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(!ok && err.contains("unknown hl.bind option"), "{out}{err}");
    // A misspelling with `_` is just as unknown.
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + Q\", hl.dsp.window.close(), { repeting = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(!ok && err.contains("unknown hl.bind option"), "{out}{err}");
}

/// The mock tells two binds apart that differ only in a table argument.
#[test]
fn mock_sees_table_arguments() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("t.lua");
    std::fs::write(
        &f,
        "hl.bind(\"SUPER + 3\", hl.dsp.focus({ workspace = 3 }))\n\
         hl.bind(\"SUPER + 4\", hl.dsp.focus({ workspace = 4 }))\n\
         hl.unbind(\"all\")\n\
         hl.bind(\"SUPER + 5\", hl.dsp.focus({ workspace = 5 }), { locked = true })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(ok, "{out}{err}");
    assert_eq!(
        bind_lines(&out),
        ["bind SUPER + 5 | hl.dsp.focus{workspace=5} | locked=true"]
    );
}

/// Review Focus 1: hostile names and commands are one `hl.bind` call each after the unbind, and
/// never run as code.
#[test]
fn hostile_binds_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let hostile = [
        "x'; $(id) \"; ]] \\ \") hl.exec_cmd(\"x",
        "a'b\"c\\d]]e$(id);f",
        "''",
    ];
    // Built past validation on purpose: the renderer must hold up on its own.
    let mut kb = Keybinds::default();
    for (i, h) in hostile.iter().enumerate() {
        kb.custom.push(uq_core::keybinds::CustomBind {
            id: format!("k-{:04x}", 2 * i + 1),
            name: (*h).into(),
            combo: format!("SUPER + F{}", i + 1),
            enabled: true,
            locked: false,
            repeating: false,
            action: uq_core::keybinds::Action::Command {
                command: (*h).into(),
            },
        });
        kb.custom.push(uq_core::keybinds::CustomBind {
            id: format!("k-{:04x}", 2 * i + 2),
            name: (*h).into(),
            combo: format!("SUPER + F{}", i + 11),
            enabled: true,
            locked: false,
            repeating: false,
            action: uq_core::keybinds::Action::Launch {
                app: format!("{h}.desktop; rm -rf ~"),
            },
        });
    }
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &kb,
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("hostile-binds"), &gen_);
    let (ok, text, err) = run_mock(
        &lua,
        dir.path(),
        false,
        &[&p.base_lua, &p.generated_lua, &p.user_lua],
    );
    assert!(ok, "{text}{err}");
    let lines: Vec<&str> = text.lines().collect();
    let unbind = lines
        .iter()
        .position(|l| *l == "hl.unbind:all")
        .expect(&text);
    let after = &lines[unbind..];
    let calls = after.iter().filter(|l| **l == "hl.bind").count();
    assert_eq!(calls, 55 + 2 * hostile.len(), "{text}");
    assert_eq!(bind_lines(&text).len(), calls, "{text}");
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
    assert!(!text.contains("hl.exec_cmd:\"x"), "{text}");
    assert!(!text.contains("rm -rf ~\n"), "{text}");
}

/// Spec §2.12 for the rendered file: no launcher bind works over the lock screen, your own
/// included. The mock's own checks run over `generated.lua`'s binds, since it clears
/// `base.lua`'s at `hl.unbind("all")`; this one reads the printed list.
#[test]
fn rendered_launcher_binds_are_not_locked() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (n, kb) in fixtures().iter().enumerate() {
        let gen_ = emit_generated(
            t,
            &layouts(),
            &rules(),
            kb,
            &settings(),
            &StartupRender::default(),
        );
        let f = dir.path().join(format!("gen{n}.lua"));
        std::fs::write(&f, gen_).unwrap();
        // The generated file alone, so base.lua can't be what is being checked.
        let (ok, text, err) = run_mock(&lua, dir.path(), true, &[&f]);
        assert!(ok, "{text}{err}");
        let mut seen = 0;
        for l in bind_lines(&text) {
            let launcher = [
                "spotlight",
                "appmenu",
                "quick-settings",
                "notif-centre",
                "lock",
                "power-menu",
            ]
            .iter()
            .any(|n| l.contains(&format!("hl.dsp.global(uq:{n})")));
            if launcher {
                seen += 1;
                assert!(!l.contains("locked=true"), "{l}");
            }
        }
        assert!(seen >= 6, "{text}");
    }
}

/// `SUPER + SUPER_R` is the one key only a default may take, so it has its own check.
#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn super_key_and_mouse_buttons_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let kb = Keybinds::from_section(
        &serde_json::json!({"overrides": [{"id": "uq.appmenu", "combo": "SUPER + SUPER_R"}]}),
        &Keybinds::default(),
    )
    .unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &kb,
        &settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("modkey"), &gen_);
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

#[test]
fn lua_mock_runs_every_variant() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    for (t, kb) in reg
        .iter()
        .flat_map(|t| fixtures().into_iter().map(move |k| (t, k)))
    {
        let root = dir
            .path()
            .join(format!("{}-{}-{}", t.id, t.variant, kb.custom.len()));
        let (p, _) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
                &kb,
                &settings(),
                &StartupRender::default(),
            ),
        );
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

/// F19: each hostile rule is exactly one call, and the payload never runs as code.
#[test]
fn hostile_rules_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let hostile = hostile_rules();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &hostile,
        &keybinds(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("hostile"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let count = |name: &str| text.lines().filter(|l| *l == name).count();
    assert_eq!(count("hl.window_rule"), hostile.window.len(), "{text}");
    assert_eq!(count("hl.layer_rule"), hostile.layer.len(), "{text}");
    // base.lua's own hl.exec_cmd calls are expected; the payload is not.
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
    assert!(!text.contains("hl.exec_cmd:\"x"), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn all_variants_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (t, kb) in reg
        .iter()
        .flat_map(|t| fixtures().into_iter().map(move |k| (t, k)))
    {
        let root = dir
            .path()
            .join(format!("{}-{}-{}", t.id, t.variant, kb.custom.len()));
        let (_, main) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
                &kb,
                &settings(),
                &StartupRender::default(),
            ),
        );
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

/// Window rules from apps (Phase 4 §3.3): romforge's packaged rule, plus one with several
/// patterns and a hostile title, as uqd renders them.
fn app_rules() -> Vec<WindowRule> {
    let text = r#"
[[window_rule]]
id = "romforge.emulators"
class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
workspace = "name:game"
fullscreen = true

[[window_rule]]
id = "romforge.tools"
class = ["^romforge$", "^romforge-cli$"]
title = "\"]] hl.exec_cmd(\"x\") --"
workspace = 7
float = true
"#;
    uq_core::integration::rules::parse_rules(text, "romforge")
        .unwrap()
        .iter()
        .flat_map(|r| r.window_rules())
        .collect()
}

/// The app rules are one `hl.window_rule` call each, and the title never runs as code.
#[test]
fn app_rules_run_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let apps = app_rules();
    assert_eq!(apps.len(), 3);
    let gen_ = emit_generated_with_apps(
        t,
        &layouts(),
        &rules(),
        &apps,
        &Keybinds::default(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("apps"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let count = |name: &str| text.lines().filter(|l| *l == name).count();
    assert_eq!(count("hl.window_rule"), rules().window.len() + 3, "{text}");
    assert!(!text.contains("hl.exec_cmd:x"), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn app_rules_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated_with_apps(
        t,
        &layouts(),
        &rules(),
        &app_rules(),
        &Keybinds::default(),
        &settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("apps"), &gen_);
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

fn startup_all() -> StartupRender {
    StartupRender {
        zero_scaling: true,
        env: vec![
            ("GDK_SCALE".into(), "2".into()),
            ("MOZ_ENABLE_WAYLAND".into(), "1".into()),
            ("QUOTED".into(), "\") hl.exec_cmd(\"x".into()),
        ],
        export: true,
        ..StartupRender::default()
    }
}

/// Review Focus 1: a hostile variable value is one `hl.env` call with the literal value, and
/// the payload never runs as code. Runs in CI only (K4).
#[test]
fn hostile_env_runs_as_one_call_each() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &settings(),
        &startup_all(),
    );
    let (p, _) = tree(&dir.path().join("env"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The mock prints "ok N", then one line per call and per string argument. Our three
    // variables are three `hl.env` calls with the literal values, back to back, and the payload
    // never reaches an `hl.exec_cmd`.
    assert!(text.starts_with("ok "), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let want = [
        "hl.env",
        "hl.env:GDK_SCALE",
        "hl.env:2",
        "hl.env",
        "hl.env:MOZ_ENABLE_WAYLAND",
        "hl.env:1",
        "hl.env",
        "hl.env:QUOTED",
        "hl.env:\") hl.exec_cmd(\"x",
    ];
    assert!(lines.windows(want.len()).any(|w| w == want), "{text}");
    assert_eq!(
        lines.iter().filter(|l| **l == "hl.env").count(),
        9,
        "{text}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("hl.exec_cmd:") && l.contains("\"x")),
        "{text}"
    );
    assert!(!lines.contains(&"hl.exec_cmd:x"), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn startup_switches_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &settings(),
        &startup_all(),
    );
    let (_, main) = tree(&dir.path().join("startup"), &gen_);
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

/// One gesture per action, every direction, modifiers, a special workspace name and every swipe
/// option: all on different triggers, so none shadows another.
fn gesture_settings() -> Settings {
    let list: Vec<Gesture> = serde_json::from_value(serde_json::json!([
        {"fingers": 3, "direction": "horizontal", "action": "workspace", "enabled": true},
        {"fingers": 3, "direction": "vertical", "action": "workspace", "modifier": "ALT",
         "enabled": true},
        {"fingers": 4, "direction": "left", "action": "move", "enabled": true},
        {"fingers": 4, "direction": "right", "action": "resize", "enabled": true},
        {"fingers": 4, "direction": "up", "action": "special", "workspace": "scratch",
         "enabled": true},
        {"fingers": 5, "direction": "down", "action": "close", "enabled": true},
        {"fingers": 5, "direction": "pinch_in", "action": "fullscreen", "enabled": true},
        {"fingers": 5, "direction": "pinch_out", "action": "float", "enabled": true},
        {"fingers": 3, "direction": "up", "action": "uq:spotlight", "modifier": "SUPER",
         "enabled": true},
        {"fingers": 4, "direction": "down", "action": "uq:quick-settings",
         "modifier": "SUPER SHIFT", "enabled": true},
        {"fingers": 3, "direction": "down", "action": "special", "modifier": "CTRL SHIFT",
         "enabled": true},
        {"fingers": 5, "direction": "left", "action": "close", "enabled": false},
    ]))
    .unwrap();
    gestures::validate(&list).expect("the fixture gestures don't clash");
    let mut s = settings();
    s.input.gestures = Some(list);
    s.input.swipe = gestures::SwipeOptions {
        distance: Some(450),
        invert: Some(false),
        create_new: Some(false),
        cancel_ratio: Some(0.35),
    };
    s
}

/// Gestures render and run under the mock, whose `hl.gesture` checks every key, the direction
/// spelling and the action type.
#[test]
fn gestures_run_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &gesture_settings(),
        &StartupRender::default(),
    );
    assert!(
        gen_.contains("gestures = { workspace_swipe_distance = 450"),
        "{gen_}"
    );
    let (p, _) = tree(&dir.path().join("gestures"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && text.starts_with("ok "),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Eleven enabled gestures (the twelfth is off), and nothing from base.lua.
    assert_eq!(
        text.lines().filter(|l| *l == "hl.gesture").count(),
        11,
        "{text}"
    );
    // The shell actions run their function: a dispatch of the global shortcut.
    assert!(text.contains("hl.dsp.global:uq:spotlight"), "{text}");
    assert!(text.contains("hl.dsp.global:uq:quick-settings"), "{text}");
}

/// The first gesture the person's defaults make also runs: uQ's own two, from a never-set
/// list, pass the same mock.
#[test]
fn default_gestures_run_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &settings(),
        &StartupRender::default(),
    );
    let (p, _) = tree(&dir.path().join("default-gestures"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        text.lines().filter(|l| *l == "hl.gesture").count(),
        2,
        "{text}"
    );
}

/// F3: at Hyprland 0.57 a bare `hl.dsp.global(…)` is a dispatcher object `hl.gesture` refuses,
/// so the mock refuses it too; a misspelt field or direction is refused as well.
#[test]
fn mock_rejects_bad_gestures() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("gesture.lua");
    for (text, want) in [
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = hl.dsp.global(\"uq:spotlight\") })\n",
            "dispatcher object",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = \"close\", modifier = \"SUPER\" })\n",
            "unknown hl.gesture field",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"up\", action = \"close\", [\"workspace-name\"] = \"x\" })\n",
            "unknown hl.gesture field",
        ),
        (
            "hl.gesture({ fingers = 4, direction = \"pinch_in\", action = \"close\" })\n",
            "unknown hl.gesture direction",
        ),
        (
            "hl.gesture({ fingers = 1, direction = \"up\", action = \"close\" })\n",
            "fingers",
        ),
    ] {
        std::fs::write(&f, text).unwrap();
        let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
        assert!(!ok && err.contains(want), "{text}: {out}{err}");
    }
    // The shape uQ writes is accepted.
    std::fs::write(
        &f,
        "hl.gesture({ fingers = 4, direction = \"up\", action = function() hl.dispatch(hl.dsp.global(\"uq:spotlight\")) end })\n",
    )
    .unwrap();
    let (ok, out, err) = run_mock(&lua, dir.path(), true, &[&f]);
    assert!(ok, "{out}{err}");
}

/// R20: `base.lua` has no gesture of its own; a second copy would shadow uQ's.
#[test]
fn base_lua_has_no_gesture() {
    let base = std::fs::read_to_string(repo().join("config/hypr/base.lua")).unwrap();
    let code: Vec<&str> = base
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect();
    assert!(!code.iter().any(|l| l.contains("hl.gesture")), "{code:?}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn gestures_pass_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &rules(),
        &keybinds(),
        &gesture_settings(),
        &StartupRender::default(),
    );
    let (_, main) = tree(&dir.path().join("gestures"), &gen_);
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

/// The workspace plan and the default monitor on top of the rules' own workspace rules: a
/// user rule for workspace 3 with its own fields, two screens, a sequential plan.
fn monitor_plan() -> StartupRender {
    use uq_core::settings::plan::Planned;
    let planned = |workspace: u8, output: &str, default: bool| Planned {
        workspace,
        output: output.into(),
        default,
    };
    StartupRender {
        monitors: MonitorRender {
            planned: vec![
                planned(1, "DP-1", true),
                planned(2, "DP-1", false),
                planned(3, "DP-2", true),
                planned(4, "DP-2", false),
            ],
            default_monitor: Some("DP-1".into()),
        },
        ..StartupRender::default()
    }
}

/// `rules()` plus a rule for workspace 3 with `gaps_in` and no monitor: the plan gives it a
/// monitor and it keeps `gaps_in`.
fn plan_rules() -> Rules {
    let mut r = rules();
    r.workspace.push(WorkspaceRule {
        workspace: "3".into(),
        monitor: None,
        default: None,
        persistent: None,
        gaps_in: Some(4),
        gaps_out: None,
        border_size: None,
    });
    r
}

/// The plan renders one `hl.workspace_rule` per selector and the cursor table runs. Runs
/// under the Lua mock (CI, or locally with a `lua` on PATH).
#[test]
fn monitor_plan_runs_under_the_lua_mock() {
    let Ok(lua) = which("lua") else {
        eprintln!("skipping: no `lua` on PATH");
        return;
    };
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mock = dir.path().join("mock.lua");
    std::fs::write(&mock, self::mock()).unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &plan_rules(),
        &keybinds(),
        &settings(),
        &monitor_plan(),
    );
    assert!(
        gen_.contains(r#"cursor = { default_monitor = "DP-1" },"#),
        "{gen_}"
    );
    let selectors: Vec<&str> = gen_
        .lines()
        .filter(|l| l.starts_with("hl.workspace_rule("))
        .filter_map(|l| l.split("workspace = \"").nth(1)?.split('"').next())
        .collect();
    let unique: std::collections::BTreeSet<_> = selectors.iter().collect();
    assert_eq!(
        selectors.len(),
        unique.len(),
        "one call per selector: {gen_}"
    );
    for ws in ["1", "2", "3", "4"] {
        assert!(selectors.contains(&ws), "{ws} in {selectors:?}");
    }
    assert!(
        gen_.contains(r#"hl.workspace_rule({ workspace = "3", monitor = "DP-2", default = true, gaps_in = 4 })"#),
        "{gen_}"
    );
    let (p, _) = tree(&dir.path().join("monitor-plan"), &gen_);
    let out = Command::new(&lua)
        .arg(&mock)
        .args([&p.base_lua, &p.generated_lua, &p.user_lua])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && text.starts_with("ok "),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let calls = text.lines().filter(|l| *l == "hl.workspace_rule").count();
    assert_eq!(calls, selectors.len(), "{text}");
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn monitor_plan_passes_hyprland_verify() {
    let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
    let t = reg.iter().next().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &plan_rules(),
        &keybinds(),
        &settings(),
        &monitor_plan(),
    );
    let (_, main) = tree(&dir.path().join("monitor-plan"), &gen_);
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

/// uq-greeter's fixed Hyprland config (greeter/hyprland.lua), behind the wrapper that
/// packaging/uq-greeter-session writes (the fixed file, then the localectl keyboard layout).
fn greeter_tree(dir: &Path) -> PathBuf {
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

#[test]
fn greeter_config_runs_under_the_lua_mock() {
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
fn greeter_config_has_no_binds() {
    let text = std::fs::read_to_string(repo().join("greeter/hyprland.lua")).unwrap();
    for needle in ["hl.bind", "hl.dsp", "hl.animation(", "hl.curve("] {
        assert!(!text.contains(needle), "greeter/hyprland.lua has {needle}");
    }
}

#[test]
#[ignore = "needs Hyprland; CI runs it with --ignored"]
fn greeter_config_passes_hyprland_verify() {
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
