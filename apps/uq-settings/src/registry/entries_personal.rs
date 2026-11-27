//! Search entries for the Personal pages: appearance, wallpaper and look.

use super::*;

pub(crate) static APPEARANCE_SETTINGS: [SearchEntry; 14] = [
    entry(
        "theme",
        "Theme",
        &["colors", "colours", "palette", "scheme"],
    ),
    entry(
        "variant",
        "Light or dark",
        &["variant", "mode", "dark mode", "light mode"],
    ),
    entry(
        "schedule",
        "Switch automatically",
        &["schedule", "sunrise", "sunset", "time", "auto"],
    ),
    entry(
        "ui-font",
        "Interface font",
        &["typeface", "text", "system font"],
    ),
    entry(
        "mono-font",
        "Monospace font",
        &["typeface", "terminal", "code"],
    ),
    entry(
        "font-size",
        "Font size",
        &["text size", "scale", "bigger text"],
    ),
    entry("cursor-theme", "Cursor theme", &["mouse", "pointer"]),
    entry(
        "cursor-size",
        "Cursor size",
        &["mouse", "pointer", "bigger cursor"],
    ),
    entry("icon-theme", "Icon theme", &["icons", "symbols"]),
    entry(
        "theme-apps",
        "Theme other apps",
        &[
            "qt", "gtk", "kde", "qt6ct", "flatpak", "colours", "colors", "apps", "match",
        ],
    ),
    entry(
        "kvantum",
        "Style Qt apps to match",
        &["kvantum", "qt", "widgets", "style"],
    ),
    entry(
        "theme-new",
        "New theme from this one",
        &["custom", "create", "colours", "colors", "edit", "make"],
    ),
    entry(
        "theme-import",
        "Import a theme",
        &["share", "file", "load", "open", "uqtheme"],
    ),
    entry(
        "theme-export",
        "Export a theme",
        &["share", "file", "save", "backup", "uqtheme"],
    ),
];

pub(crate) static WALLPAPER_SETTINGS: [SearchEntry; 6] = [
    entry(
        "wallpaper-display",
        "Wallpaper display",
        &["monitor", "screen", "each display", "per display"],
    ),
    entry(
        "wallpaper-image",
        "Wallpaper image",
        &["background", "picture", "desktop background", "photo"],
    ),
    entry(
        "wallpaper-fit",
        "Wallpaper fit",
        &["fill", "stretch", "center", "tile", "scale", "span"],
    ),
    entry(
        "wallpaper-slideshow",
        "Wallpaper slideshow folder",
        &["folder", "rotate", "cycle", "slideshow", "background"],
    ),
    entry(
        "wallpaper-interval",
        "Slideshow interval",
        &["change every", "minutes", "rotate", "time"],
    ),
    entry(
        "wallpaper-color",
        "Background colour",
        &["color", "solid", "fallback", "backdrop"],
    ),
];

pub(crate) static LOOK_SETTINGS: [SearchEntry; 16] = [
    entry(
        "gaps-inner",
        "Gaps between windows",
        &["gap", "spacing", "padding", "inner"],
    ),
    entry(
        "gaps-outer",
        "Gaps around the screen",
        &["gap", "margin", "outer", "edge"],
    ),
    entry(
        "border-width",
        "Window border width",
        &["outline", "frame", "thickness"],
    ),
    entry(
        "rounding",
        "Corner rounding",
        &["radius", "round corners", "corners"],
    ),
    entry(
        "blur",
        "Blur behind windows",
        &["frosted", "glass", "transparency"],
    ),
    entry(
        "blur-size",
        "Blur size",
        &["blur radius", "frosted", "strength"],
    ),
    entry(
        "blur-passes",
        "Blur passes",
        &["blur quality", "frosted", "iterations"],
    ),
    entry("shadow", "Window shadows", &["drop shadow", "depth"]),
    entry(
        "animations",
        "Animations",
        &["motion", "transitions", "effects"],
    ),
    entry(
        "animation-speed",
        "Animation speed",
        &["faster", "slower", "motion", "transitions"],
    ),
    entry(
        "reduce-motion",
        "Reduce motion",
        &["animations", "accessibility", "fade", "motion sickness"],
    ),
    entry(
        "default-layout",
        "Default layout",
        &["tiling", "dwindle", "master", "scrolling"],
    ),
    entry(
        "workspace-layouts",
        "Layout for each workspace",
        &["tiling", "workspace rules", "per workspace"],
    ),
    entry(
        "dwindle-options",
        "Dwindle layout options",
        &["split", "tiling", "preserve split"],
    ),
    entry(
        "master-options",
        "Master layout options",
        &["master", "mfact", "orientation", "stack"],
    ),
    entry(
        "scrolling-options",
        "Scrolling layout options",
        &["columns", "column width", "tiling"],
    ),
];
