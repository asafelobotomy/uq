-- uQ base Hyprland configuration (installed to /usr/share/uq/hypr/base.lua; read-only).
--
-- Load order, from ~/.config/hypr/hyprland.lua:
--   1. this file            uQ defaults
--   2. uq/generated.lua     written by uqd: theme colours, gaps, per-workspace layouts
--   3. uq/user.lua          yours; read last, never rewritten by uQ
--
-- Adapted from Hyprland's example config (BSD-3-Clause, Copyright (c) 2022-2026, vaxerski).

-- The terminal chosen in Settings › Default apps, started as the launcher starts apps
-- (keybinds/defaults.rs TERMINAL); should uqctl fail, kyko, or kitty where kyko is not installed,
-- in a scope of its own in app.slice.
local terminal = "uqctl launch --terminal 2>/dev/null || systemd-run --user --scope --quiet --collect --slice=app.slice --unit=app-uq-terminal-$$ -- sh -c 'command -v kyko >/dev/null && exec kyko; exec kitty'"
local mainMod  = "SUPER"

------------------
---- MONITORS ----
------------------

-- Per-output hl.monitor lines come from generated.lua (Displays). This catch-all is the
-- fallback for any output with no stored entry, and what a reload falls back to on revert.
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
-- scdr's session agent (optional) follows Hyprland focus for scdr's scheduling roles. It is tied
-- to graphical-session.target, which a plain Hyprland session never reaches, so it is (re)started
-- here as well, once the environment is in place, if the user enabled it; and stopped with uQ.
-- `uq-dev session` (a nested dev desktop, docs/developing.md) sets UQ_DEV_SESSION and runs a
-- checkout's own uqd and shell: there both hooks do nothing, or they would repoint the installed
-- uqd and shell at the nested session and stop them, bar and all, when it closes.
local session_env = "WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORMTHEME DISPLAY XCURSOR_SIZE HYPRCURSOR_SIZE XCURSOR_THEME XDG_SESSION_TYPE XDG_SESSION_ID"
hl.on("hyprland.start", function()
    hl.exec_cmd("test -n \"$UQ_DEV_SESSION\" || { dbus-update-activation-environment --systemd " .. session_env .. "; systemctl --user restart uqd.service; systemctl --user restart uq-shell.service; systemctl --user -q is-enabled scdr-session.service && systemctl --user restart scdr-session.service; }")
end)
-- The shell and uqlip are stopped here and waited for (os.execute blocks; exec_cmd doesn't):
-- Hyprland closes its Wayland socket right after this hook, and a shell that loses its compositor
-- first exits with an error (a failed unit in the journal at every logout and shutdown). The
-- timeout keeps a hung stop from holding up the logout.
hl.on("hyprland.shutdown", function()
    if (os.getenv("UQ_DEV_SESSION") or "") == "" then
        -- Tells uqd the session is ending: it notes the open apps (restore last session) while
        -- the windows are still there, and runs the logout commands when it is stopped below.
        os.execute("timeout 2 systemctl --user kill -s USR1 uqd.service")
        os.execute("timeout 5 systemctl --user stop uq-shell.service uqlip.service")
    end
    hl.exec_cmd("test -n \"$UQ_DEV_SESSION\" || { systemctl --user stop uqd.service; systemctl --user stop scdr-session.service; }")
end)

-----------------------------
---- ENVIRONMENT VARIABLES ----
-----------------------------

hl.env("XCURSOR_SIZE", "24")
hl.env("HYPRCURSOR_SIZE", "24")
-- Qt apps get uQ's colours, icons, fonts and widget style from ~/.config/kdeglobals through
-- plasma-integration's `kde` platform theme (Dolphin and every other KDE app need it), which
-- also themes plain Qt apps. Without it, plain Qt apps fall back to qt6ct (`uqctl setup`).
local function have_file(path)
    local f = io.open(path, "r")
    if f then f:close() end
    return f ~= nil
end
local kde_theme = have_file("/usr/lib/qt6/plugins/platformthemes/KDEPlasmaPlatformTheme6.so")
hl.env("QT_QPA_PLATFORMTHEME", kde_theme and "kde" or "qt6ct")

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
    -- A Super+click on a window only becomes a move once the pointer has travelled this far (px).
    -- At 0 (Hyprland's default) the click lifts a tiled window out of its layout at once, and the
    -- others reflow for what is only a click.
    binds = {
        drag_threshold = 8,
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
        -- An app that asks to be activated (a notification's View/Activate action, a tray
        -- click, xdg-activation) gets focus and its workspace instead of only an urgent mark.
        focus_on_activate = true,
        -- Session lock (slice 5): survive a lock client crash, keep the lock opaque, and wake
        -- the screen on input.
        allow_session_lock_restore = true,
        session_lock_xray          = false,
        key_press_enables_dpms     = true,
        mouse_move_enables_dpms    = true,
    },
})

-- Animation curves and leaves are rendered into generated.lua (look.animation_speed scales them).

-- The shell's popups, menus, OSD and notifications come and go in QML (shell/core/UqCard.qml,
-- the same speeds as the app menu and quick settings, which stay mapped), and so does kyko's
-- drop-down (it slides and dims at uQ's motion speeds itself). Hyprland's own layer
-- fade would run on top of that at another speed, so it is off for them. The bar, wallpaper and
-- the rest keep Hyprland's animation.
hl.layer_rule({
    name    = "uq-base:shell-popups",
    enabled = true,
    match   = { namespace = "^(uq-bar-popup|uq-desktop-menu|uq-clip|uq-osd|uq-notif|uq-notif-centre|uq-polkit|kyko)$" },
    no_anim = true,
})

---------------
---- INPUT ----
---------------

-- Keyboard layouts, repeat, touchpad options, mouse settings and hl.device overrides come
-- from generated.lua, which loads after this and wins; these are the fallbacks.
hl.config({
    input = {
        kb_layout    = "us",
        -- Num Lock is on when the session starts; to turn that off, set it false in user.lua.
        numlock_by_default = true,
        -- Click to focus: a window takes the keyboard only when clicked (hover still scrolls the
        -- window under the cursor). 1 is focus-follows-mouse; set it in user.lua to have that.
        follow_mouse = 2,
        sensitivity  = 0,
        touchpad = {
            natural_scroll = false,
        },
    },
})

-- numlock-devices:begin (scripts/numlock-live.sh runs this block on a real Hyprland)
-- Hyprland keeps a Num Lock state per keyboard and tells apps the OR of all of them whenever focus
-- moves. numlock_by_default turns Num Lock on for *every* keyboard, including button-only "keyboards"
-- nobody can press Num Lock on (the power and sleep buttons, the display's brightness keys, laptop
-- hotkey devices) and emobie's typing device. Those would then hold Num Lock on for good: after you
-- turn it off, the next window you focus (or the lock screen) gets it back on. So they start with it
-- off; your real keyboards still start with it on. Duplicates get a -1, -2 suffix in Hyprland.
-- A second real keyboard you never type on (a laptop's own while you use an external one) does the
-- same; set numlock_by_default = false for it in user.lua (its name is in `hyprctl devices`).
for _, base in ipairs({
    "power-button", "sleep-button", "video-bus", "intel-hid-events", "intel-hid-5-button-array",
    "asus-wmi-hotkeys", "thinkpad-extra-buttons", "dell-wmi-hotkeys", "hp-wmi-hotkeys",
    "ideapad-extra-buttons", "huawei-wmi-hotkeys", "acer-wmi-hotkeys", "msi-wmi-hotkeys",
    "framework-laptop-hotkeys", "emobie-inject",
}) do
    for _, suffix in ipairs({ "", "-1", "-2", "-3" }) do
        hl.device({ name = base .. suffix, numlock_by_default = false })
    end
end
-- numlock-devices:end

-- Touchpad gestures come from generated.lua (Input page), so a second copy here can't
-- shadow them.

---------------------
---- KEYBINDINGS ----
---------------------

-- Fallback shortcuts. uqd renders the real set into generated.lua, which starts with
-- hl.unbind("all") and replaces all of these. They are here so a missing or old
-- generated.lua still leaves a working desktop. Keep them equal to
-- crates/uq-core/src/keybinds/defaults.rs (test base_lua_binds_equal_the_default_table).
hl.bind(mainMod .. " + Return", hl.dsp.exec_cmd(terminal))
-- kyko's drop-down terminal (Settings › kyko picks where it comes from).
hl.bind("F12",                  hl.dsp.exec_cmd("kyko toggle"))
hl.bind(mainMod .. " + Q",      hl.dsp.window.close())
hl.bind(mainMod .. " + V",      hl.dsp.window.float({ action = "toggle" }))
hl.bind(mainMod .. " + SHIFT + M", hl.dsp.exec_cmd("command -v hyprshutdown >/dev/null 2>&1 && hyprshutdown || hyprctl dispatch 'hl.dsp.exit()'"))

-- Launchers, quick settings, the notification centre, the lock and the power menu: Quickshell GlobalShortcuts (appid "uq"). Never locked, so they
-- don't work over the lock screen. The app menu bind fires when a tap of left SUPER is released; Hyprland shadows it
-- once another key or button has been pressed and released while SUPER is held.
hl.bind(mainMod .. " + Space",   hl.dsp.global("uq:lookseedo"))
-- SUPER+.: look=see>do_ opened at ":", the emoji search (an emoji provider such as emobie answers it).
hl.bind(mainMod .. " + period",  hl.dsp.global("uq:emoji"))
-- The Calculator key: look=see>do_ opened at "=", its calculator.
hl.bind("XF86Calculator",        hl.dsp.global("uq:calc"))
-- SUPER+SHIFT+V: the clipboard history (uqlip).
hl.bind(mainMod .. " + SHIFT + V", hl.dsp.global("uq:clip"))
hl.bind(mainMod .. " + SUPER_L", hl.dsp.global("uq:appmenu"), { release = true })
hl.bind(mainMod .. " + A",       hl.dsp.global("uq:quick-settings"))
hl.bind(mainMod .. " + N",       hl.dsp.global("uq:notif-centre"))
-- SUPER+L: Session.lock() in the shell, which starts uq-lock.service (ignored when already locked).
hl.bind(mainMod .. " + L",       hl.dsp.global("uq:lock"))
-- SUPER+Escape: the power menu (restart and shut down count down 30 s before they run).
hl.bind(mainMod .. " + Escape",  hl.dsp.global("uq:power-menu"))
-- SUPER+ALT+B: edit the bar (move, resize, add and remove islands; Esc finishes).
hl.bind(mainMod .. " + ALT + B", hl.dsp.global("uq:edit-bar"))
-- Screenshots (uq-screenshot): Print does what Settings > Input > Print key says (an area by
-- default); Shift+Print the screen; Super+Print the focused window. Each is saved to
-- ~/Pictures/Screenshots, copied, and shown in the shell's floating preview.
hl.bind("Print",                  hl.dsp.exec_cmd("uq-screenshot print"))
hl.bind("SHIFT + Print",          hl.dsp.exec_cmd("uq-screenshot screen"))
hl.bind(mainMod .. " + Print",    hl.dsp.exec_cmd("uq-screenshot window"))

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

-- Columns in the scrolling layout: Full or back to Half, move a column, make it narrower or wider
hl.bind(mainMod .. " + F",              hl.dsp.window.fullscreen({ mode = "maximized" }))
hl.bind(mainMod .. " + SHIFT + left",   hl.dsp.layout("swapcol l"))
hl.bind(mainMod .. " + SHIFT + right",  hl.dsp.layout("swapcol r"))
hl.bind(mainMod .. " + CTRL + left",    hl.dsp.layout("colresize -0.1"))
hl.bind(mainMod .. " + CTRL + right",   hl.dsp.layout("colresize +0.1"))

-- Move with the mouse. Resizing is not bound: a mouse resize would take tiled windows from any
-- edge, the screen's outer ones too. Tiled windows resize by the grips in the gaps between them
-- (shell/windows/ResizeGrips.qml); bind hl.dsp.window.resize() in user.lua for floating ones.
-- The two hooks tell the shell when the button goes down and comes up (custom events), so it
-- holds its own window rearranging still meanwhile.
hl.bind(mainMod .. " + mouse:272", hl.dsp.event("uq-mouse:begin"), { non_consuming = true })
hl.bind(mainMod .. " + mouse:272", hl.dsp.event("uq-mouse:end"),   { release = true, ignore_mods = true, non_consuming = true })
hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(),   { mouse = true })

-- Lock recovery: if the lock surface dies, this restarts it (locked = true, so it works over the
-- lock screen). uq-lock.service arrives in a later slice; until then hyprlock takes over.
hl.bind(mainMod .. " + CTRL + L", hl.dsp.exec_cmd("systemctl --user restart uq-lock.service || hyprlock"), { locked = true })

-- Volume, mic and brightness keys go through the shell (OSD included); see shell/osd/OsdRoutes.qml.
hl.bind("XF86AudioRaiseVolume",  hl.dsp.global("uq:vol-up"),      { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume",  hl.dsp.global("uq:vol-down"),    { locked = true, repeating = true })
hl.bind("XF86AudioMute",         hl.dsp.global("uq:mute"),        { locked = true })
hl.bind("XF86AudioMicMute",      hl.dsp.global("uq:mic-mute"),    { locked = true })
hl.bind("XF86MonBrightnessUp",   hl.dsp.global("uq:bright-up"),   { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.global("uq:bright-down"), { locked = true, repeating = true })
-- Media keys
hl.bind("XF86AudioNext",  hl.dsp.exec_cmd("playerctl next"),       { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPlay",  hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPrev",  hl.dsp.exec_cmd("playerctl previous"),   { locked = true })
