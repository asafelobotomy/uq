-- uQ base Hyprland configuration (installed to /usr/share/uq/hypr/base.lua; read-only).
--
-- Load order, from ~/.config/hypr/hyprland.lua:
--   1. this file            uQ defaults
--   2. uq/generated.lua     written by uqd: theme colours, gaps, per-workspace layouts
--   3. uq/user.lua          yours; read last, never rewritten by uQ
--
-- Adapted from Hyprland's example config (BSD-3-Clause, Copyright (c) 2022-2026, vaxerski).

local terminal = "kitty"
local mainMod  = "SUPER"

------------------
---- MONITORS ----
------------------

hl.monitor({
    output   = "",
    mode     = "preferred",
    position = "auto",
    scale    = "auto",
})

-------------------
---- AUTOSTART ----
-------------------

-- uqd and the shell run as systemd user units, which do not inherit this session's environment:
-- export what they need to reach Hyprland and the display, then (re)start them so they always
-- talk to this session. Both are stopped again when Hyprland exits, so neither outlives the
-- session and re-themes another desktop. The steps are separated by `;`, not `&&`: uqd failing
-- to start degrades the widgets but must never keep the bar from starting.
local session_env = "WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORMTHEME DISPLAY XCURSOR_SIZE HYPRCURSOR_SIZE XCURSOR_THEME XDG_SESSION_TYPE"
hl.on("hyprland.start", function()
    hl.exec_cmd("dbus-update-activation-environment --systemd " .. session_env .. "; systemctl --user restart uqd.service; systemctl --user restart uq-shell.service")
end)
hl.on("hyprland.shutdown", function()
    hl.exec_cmd("systemctl --user stop uq-shell.service; systemctl --user stop uqd.service")
end)

-----------------------------
---- ENVIRONMENT VARIABLES ----
-----------------------------

hl.env("XCURSOR_SIZE", "24")
hl.env("HYPRCURSOR_SIZE", "24")
-- Qt apps read uQ's colour scheme through qt6ct (see `uqctl setup`).
hl.env("QT_QPA_PLATFORMTHEME", "qt6ct")

-----------------------
---- LOOK AND FEEL ----
-----------------------

-- Colours, gaps, border size and rounding come from the theme, in generated.lua.
hl.config({
    general = {
        resize_on_border = false,
        allow_tearing    = false,
        layout           = "dwindle",
    },
    decoration = {
        rounding_power   = 2,
        active_opacity   = 1.0,
        inactive_opacity = 1.0,
        shadow = {
            enabled      = true,
            range        = 4,
            render_power = 3,
            color        = 0xee1a1a1a,
        },
        blur = {
            enabled  = true,
            size     = 3,
            passes   = 1,
            vibrancy = 0.1696,
        },
    },
    animations = {
        enabled = true,
    },
    dwindle = {
        preserve_split = true,
    },
    scrolling = {
        fullscreen_on_one_column = true,
    },
    misc = {
        force_default_wallpaper = 0,
        disable_hyprland_logo   = true,
        -- Session lock (slice 5): survive a lock client crash, keep the lock opaque, and wake
        -- the screen on input.
        allow_session_lock_restore = true,
        session_lock_xray          = false,
        key_press_enables_dpms     = true,
        mouse_move_enables_dpms    = true,
    },
})

hl.curve("easeOutQuint", { type = "bezier", points = { {0.23, 1},  {0.32, 1} } })
hl.curve("linear",       { type = "bezier", points = { {0, 0},     {1, 1}    } })
hl.curve("almostLinear", { type = "bezier", points = { {0.5, 0.5}, {0.75, 1} } })
hl.curve("quick",        { type = "bezier", points = { {0.15, 0},  {0.1, 1}  } })

hl.animation({ leaf = "global",     enabled = true, speed = 10,   bezier = "default" })
hl.animation({ leaf = "border",     enabled = true, speed = 5.39, bezier = "easeOutQuint" })
hl.animation({ leaf = "windowsIn",  enabled = true, speed = 4.1,  bezier = "easeOutQuint", style = "popin 87%" })
hl.animation({ leaf = "windowsOut", enabled = true, speed = 1.49, bezier = "linear",       style = "popin 87%" })
hl.animation({ leaf = "fade",       enabled = true, speed = 3.03, bezier = "quick" })
hl.animation({ leaf = "layers",     enabled = true, speed = 3.81, bezier = "easeOutQuint" })
hl.animation({ leaf = "workspaces", enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })

---------------
---- INPUT ----
---------------

hl.config({
    input = {
        kb_layout    = "us",
        follow_mouse = 1,
        sensitivity  = 0,
        touchpad = {
            natural_scroll = false,
        },
    },
})

hl.gesture({
    fingers   = 3,
    direction = "horizontal",
    action    = "workspace",
})

---------------------
---- KEYBINDINGS ----
---------------------

hl.bind(mainMod .. " + Return", hl.dsp.exec_cmd(terminal))
hl.bind(mainMod .. " + Q",      hl.dsp.window.close())
hl.bind(mainMod .. " + V",      hl.dsp.window.float({ action = "toggle" }))
hl.bind(mainMod .. " + SHIFT + M", hl.dsp.exec_cmd("command -v hyprshutdown >/dev/null 2>&1 && hyprshutdown || hyprctl dispatch 'hl.dsp.exit()'"))

-- Launchers: Quickshell GlobalShortcuts (appid "uq"). Never locked, so they don't work over the
-- lock screen. The app menu bind fires when a tap of left SUPER is released; Hyprland shadows it
-- once another key or button has been pressed and released while SUPER is held.
hl.bind(mainMod .. " + Space",   hl.dsp.global("uq:spotlight"))
hl.bind(mainMod .. " + SUPER_L", hl.dsp.global("uq:appmenu"), { release = true })

-- Per-workspace layout (persisted by uqd)
hl.bind(mainMod .. " + ALT + D", hl.dsp.exec_cmd("uqctl layout set current dwindle"))
hl.bind(mainMod .. " + ALT + M", hl.dsp.exec_cmd("uqctl layout set current master"))
hl.bind(mainMod .. " + ALT + S", hl.dsp.exec_cmd("uqctl layout set current scrolling"))

-- Focus
hl.bind(mainMod .. " + left",  hl.dsp.focus({ direction = "left" }))
hl.bind(mainMod .. " + right", hl.dsp.focus({ direction = "right" }))
hl.bind(mainMod .. " + up",    hl.dsp.focus({ direction = "up" }))
hl.bind(mainMod .. " + down",  hl.dsp.focus({ direction = "down" }))

-- Workspaces 1-10 (key 0 is workspace 10)
for i = 1, 10 do
    local key = i % 10
    hl.bind(mainMod .. " + " .. key,         hl.dsp.focus({ workspace = i }))
    hl.bind(mainMod .. " + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }))
end

hl.bind(mainMod .. " + S",         hl.dsp.workspace.toggle_special("magic"))
hl.bind(mainMod .. " + SHIFT + S", hl.dsp.window.move({ workspace = "special:magic" }))
hl.bind(mainMod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
hl.bind(mainMod .. " + mouse_up",   hl.dsp.focus({ workspace = "e-1" }))

-- Move/resize with the mouse
hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(),   { mouse = true })
hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })

-- Lock recovery: if the lock surface dies, this restarts it (locked = true, so it works over the
-- lock screen). uq-lock.service arrives in a later slice; until then hyprlock takes over.
hl.bind(mainMod .. " + CTRL + L", hl.dsp.exec_cmd("systemctl --user restart uq-lock.service || hyprlock"), { locked = true })

-- Media and brightness keys
hl.bind("XF86AudioRaiseVolume",  hl.dsp.exec_cmd("wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume",  hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"),      { locked = true, repeating = true })
hl.bind("XF86AudioMute",         hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"),     { locked = true, repeating = true })
hl.bind("XF86AudioMicMute",      hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"),   { locked = true, repeating = true })
hl.bind("XF86MonBrightnessUp",   hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%+"),                  { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("brightnessctl -e4 -n2 set 5%-"),                  { locked = true, repeating = true })
hl.bind("XF86AudioNext",  hl.dsp.exec_cmd("playerctl next"),       { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPlay",  hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPrev",  hl.dsp.exec_cmd("playerctl previous"),   { locked = true })
