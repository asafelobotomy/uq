//! Every key provenance explains, with where uQ renders it.

use super::*;

pub(crate) const IFACE: &str = "org.gnome.desktop.interface";

pub(crate) const fn hypr(
    key: &'static str,
    setting_id: &'static str,
    label: &'static str,
    path: &'static str,
    null_source: NullSource,
) -> OptionRow {
    OptionRow {
        key,
        setting_id,
        label,
        hypr: Some(path),
        gsettings: None,
        null_source,
    }
}

pub(crate) const fn gsettings(
    key: &'static str,
    setting_id: &'static str,
    label: &'static str,
    gkey: &'static str,
) -> OptionRow {
    OptionRow {
        key,
        setting_id,
        label,
        hypr: None,
        gsettings: Some((IFACE, gkey)),
        null_source: NullSource::Theme,
    }
}

use NullSource::{Default as Dflt, Theme as Tok};

/// Every key provenance explains. Option paths are the `hl.config` table paths `generated.lua`
/// renders (see `crate::lua`), checked against Hyprland 0.56.2's `ConfigValues.cpp`.
pub static OPTIONS: &[OptionRow] = &[
    hypr(
        "look.gaps_in",
        "gaps-inner",
        "Gaps between windows",
        "general:gaps_in",
        Tok,
    ),
    hypr(
        "look.gaps_out",
        "gaps-outer",
        "Gaps around the screen",
        "general:gaps_out",
        Tok,
    ),
    hypr(
        "look.border_width",
        "border-width",
        "Window border width",
        "general:border_size",
        Tok,
    ),
    hypr(
        "look.rounding",
        "rounding",
        "Corner rounding",
        "decoration:rounding",
        Tok,
    ),
    hypr(
        "look.blur",
        "blur",
        "Blur behind windows",
        "decoration:blur:enabled",
        Dflt,
    ),
    hypr(
        "look.blur_size",
        "blur-size",
        "Blur size",
        "decoration:blur:size",
        Dflt,
    ),
    hypr(
        "look.blur_passes",
        "blur-passes",
        "Blur passes",
        "decoration:blur:passes",
        Dflt,
    ),
    hypr(
        "look.shadow",
        "shadow",
        "Window shadows",
        "decoration:shadow:enabled",
        Dflt,
    ),
    hypr(
        "look.animations",
        "animations",
        "Animations",
        "animations:enabled",
        Dflt,
    ),
    // Scales every `hl.animation` speed; Hyprland has no single option to read back.
    OptionRow {
        key: "look.animation_speed",
        setting_id: "animation-speed",
        label: "Animation speed",
        hypr: None,
        gsettings: None,
        null_source: Dflt,
    },
    hypr(
        "layouts.default",
        "default-layout",
        "Default layout",
        "general:layout",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.preserve_split",
        "dwindle-options",
        "Dwindle: keep the split",
        "dwindle:preserve_split",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.smart_split",
        "dwindle-options",
        "Dwindle: split by cursor position",
        "dwindle:smart_split",
        Dflt,
    ),
    hypr(
        "layouts.dwindle.force_split",
        "dwindle-options",
        "Dwindle: new window side",
        "dwindle:force_split",
        Dflt,
    ),
    hypr(
        "layouts.master.mfact",
        "master-options",
        "Master: master area size",
        "master:mfact",
        Dflt,
    ),
    hypr(
        "layouts.master.new_status",
        "master-options",
        "Master: new windows",
        "master:new_status",
        Dflt,
    ),
    hypr(
        "layouts.master.orientation",
        "master-options",
        "Master: master area position",
        "master:orientation",
        Dflt,
    ),
    hypr(
        "layouts.scrolling.column_width",
        "scrolling-options",
        "Scrolling: column width",
        "scrolling:column_width",
        Dflt,
    ),
    hypr(
        "layouts.scrolling.fullscreen_on_one_column",
        "scrolling-options",
        "Scrolling: one column fills the screen",
        "scrolling:fullscreen_on_one_column",
        Dflt,
    ),
    gsettings(
        "appearance.ui_font",
        "ui-font",
        "Interface font",
        "font-name",
    ),
    gsettings(
        "appearance.mono_font",
        "mono-font",
        "Monospace font",
        "monospace-font-name",
    ),
    gsettings(
        "appearance.font_size",
        "font-size",
        "Font size",
        "font-name",
    ),
    gsettings(
        "appearance.cursor_theme",
        "cursor-theme",
        "Cursor theme",
        "cursor-theme",
    ),
    gsettings(
        "appearance.cursor_size",
        "cursor-size",
        "Cursor size",
        "cursor-size",
    ),
    gsettings(
        "appearance.icon_theme",
        "icon-theme",
        "Icon theme",
        "icon-theme",
    ),
    // The `input` table generated.lua renders (`crate::lua::input`): keyboard, mouse (always
    // rendered), then the touchpad. Labels are the history labels (`crate::history::labels`).
    hypr(
        "input.keyboard.layouts",
        "keyboard-layouts",
        "Keyboard layouts",
        "input:kb_layout",
        Dflt,
    ),
    hypr(
        "input.keyboard.switch_combo",
        "layout-switch",
        "Layout switch keys",
        "input:kb_options",
        Dflt,
    ),
    hypr(
        "input.keyboard.repeat_rate",
        "repeat-rate",
        "Key repeat rate",
        "input:repeat_rate",
        Dflt,
    ),
    hypr(
        "input.keyboard.repeat_delay",
        "repeat-delay",
        "Key repeat delay",
        "input:repeat_delay",
        Dflt,
    ),
    hypr(
        "input.mouse.sensitivity",
        "mouse-speed",
        "Pointer speed",
        "input:sensitivity",
        Dflt,
    ),
    hypr(
        "input.mouse.accel_profile",
        "mouse-accel",
        "Pointer acceleration",
        "input:accel_profile",
        Dflt,
    ),
    hypr(
        "input.mouse.natural_scroll",
        "mouse-natural-scroll",
        "Natural scrolling",
        "input:natural_scroll",
        Dflt,
    ),
    hypr(
        "input.mouse.left_handed",
        "mouse-left-handed",
        "Left-handed buttons",
        "input:left_handed",
        Dflt,
    ),
    hypr(
        "input.mouse.scroll_factor",
        "mouse-scroll-speed",
        "Scroll speed",
        "input:scroll_factor",
        Dflt,
    ),
    hypr(
        "input.touchpad.natural_scroll",
        "touchpad-natural-scroll",
        "Touchpad natural scrolling",
        "input:touchpad:natural_scroll",
        Dflt,
    ),
    hypr(
        "input.touchpad.tap_to_click",
        "tap-to-click",
        "Tap to click",
        "input:touchpad:tap_to_click",
        Dflt,
    ),
    hypr(
        "input.touchpad.disable_while_typing",
        "disable-while-typing",
        "Touchpad off while typing",
        "input:touchpad:disable_while_typing",
        Dflt,
    ),
    hypr(
        "input.touchpad.scroll_factor",
        "touchpad-scroll-speed",
        "Touchpad scroll speed",
        "input:touchpad:scroll_factor",
        Dflt,
    ),
    // `gestures:workspace_swipe_*`: ConfigValues.cpp lines 400-406 at v0.56.2, 487-493 at
    // v0.57.0. The option names live in `crate::gestures::hypr::SWIPE_OPTIONS`.
    hypr(
        "input.swipe.distance",
        "gesture-swipe-distance",
        "Swipe distance",
        "gestures:workspace_swipe_distance",
        Dflt,
    ),
    hypr(
        "input.swipe.invert",
        "gesture-swipe-invert",
        "Reverse swipe direction",
        "gestures:workspace_swipe_invert",
        Dflt,
    ),
    hypr(
        "input.swipe.create_new",
        "gesture-swipe-create",
        "Swipe past the last workspace to make a new one",
        "gestures:workspace_swipe_create_new",
        Dflt,
    ),
    hypr(
        "input.swipe.cancel_ratio",
        "gesture-swipe-cancel",
        "How far to swipe before it switches",
        "gestures:workspace_swipe_cancel_ratio",
        Dflt,
    ),
    // `xwayland:force_zero_scaling`: ConfigValues.cpp line 549 at v0.56.2, line 653 at v0.57.0
    // (see `crate::compat::hypr`).
    hypr(
        "compat.sharp_x11",
        "sharp-x11",
        "Sharp X11 apps",
        crate::compat::hypr::XWAYLAND_ZERO_SCALING,
        Dflt,
    ),
];
