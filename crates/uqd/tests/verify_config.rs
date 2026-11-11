//! uQ's base config plus every generated variant must load.
//!
//! - `lua_mock_runs_every_variant` runs the files under a stock Lua interpreter with a mock `hl`
//!   table: catches syntax and runtime errors without Hyprland installed.
//! - `all_variants_pass_hyprland_verify` (ignored by default) runs `Hyprland --verify-config`;
//!   CI runs it with `--ignored` in an Arch container that has Hyprland.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uq_core::lua::{Layout, StartupRender, emit_generated};
use uq_core::paths::Paths;
use uq_core::rules::{
    LayerEffects, LayerMatch, LayerRule, Rules, WindowEffects, WindowMatch, WindowRule,
    WorkspaceRule,
};
use uq_core::settings::{
    AccelProfile, DeviceOverride, ForceSplit, KbLayout, MouseSettings, NewStatus, Orientation,
    OutputConfig, Position, Settings,
};
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
"#;

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
        let (p, _) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
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
    std::fs::write(&mock, MOCK).unwrap();
    let hostile = hostile_rules();
    let gen_ = emit_generated(
        t,
        &layouts(),
        &hostile,
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
    for t in reg.iter() {
        let root = dir.path().join(format!("{}-{}", t.id, t.variant));
        let (_, main) = tree(
            &root,
            &emit_generated(
                t,
                &layouts(),
                &rules(),
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

fn startup_all() -> StartupRender {
    StartupRender {
        zero_scaling: true,
        env: vec![
            ("GDK_SCALE".into(), "2".into()),
            ("MOZ_ENABLE_WAYLAND".into(), "1".into()),
            ("QUOTED".into(), "\") hl.exec_cmd(\"x".into()),
        ],
        export: true,
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
    std::fs::write(&mock, MOCK).unwrap();
    let gen_ = emit_generated(t, &layouts(), &rules(), &settings(), &startup_all());
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
    let gen_ = emit_generated(t, &layouts(), &rules(), &settings(), &startup_all());
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

fn which(bin: &str) -> Result<PathBuf, ()> {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(bin))
                .find(|p| p.is_file())
        })
        .ok_or(())
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
