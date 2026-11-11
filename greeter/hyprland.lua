-- uq-greeter's Hyprland config (installed to /usr/share/uq/greeter/hyprland.lua; fixed).
--
-- /usr/lib/uq/uq-greeter-session starts Hyprland with a small wrapper that requires this file and
-- then sets the keyboard layout from `localectl` (so passwords type as expected). There are no key
-- binds: VT switching (Ctrl+Alt+F1…F12) is Hyprland's own and needs none. No animations, no
-- splash, no update or donation pop-ups. The only client is the greeter (Quickshell), started
-- below; when it exits, so does Hyprland.

hl.monitor({
    output   = "",
    mode     = "preferred",
    position = "auto",
    scale    = "auto",
})

hl.config({
    general = {
        border_size = 0,
    },
    decoration = {
        rounding = 0,
        blur   = { enabled = false },
        shadow = { enabled = false },
    },
    animations = {
        enabled = false,
    },
    input = {
        kb_layout    = "us",
        follow_mouse = 1,
    },
    misc = {
        disable_hyprland_logo           = true,
        disable_splash_rendering        = true,
        force_default_wallpaper         = 0,
        -- Started by uq-greeter-session, not start-hyprland: its watchdog would restart a crashed
        -- greeter compositor in safe mode, without this config. Crash recovery is the script's.
        disable_watchdog_warning        = true,
        disable_xdg_env_checks          = true,
        disable_hyprland_guiutils_check = true,
        key_press_enables_dpms          = true,
        mouse_move_enables_dpms         = true,
    },
    ecosystem = {
        no_update_news  = true,
        no_donation_nag = true,
    },
})

hl.on("hyprland.start", function()
    hl.exec_cmd("/usr/lib/uq/uq-greeter-session --shell")
end)
