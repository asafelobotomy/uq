// uq-consts 1.0.0
// The values uQ, Bindle and uQ's native apps share. Generated from uQ's
// native/uq-consts/consts.toml by gen.py: vendor this file unchanged. An ES module with no
// dependencies and no file system access (paths are relative to the XDG base directories).

export const VERSION = "1.0.0";

export const statePaths = Object.freeze({
  "uqDir": "uq",
  "themeDir": "uq/theme",
  "tokens": "uq/theme/tokens.json",
  "crashesDir": "uq/crashes"
});

export const cachePaths = Object.freeze({
  "quickshellCrashesDir": "quickshell/crashes"
});

export const install = Object.freeze({
  "dataDir": "/usr/share/uq",
  "pagesDir": "/usr/share/uq/settings/pages",
  "widgetsDir": "/usr/share/uq/shell/widgets",
  "launcherDir": "/usr/share/uq/launcher",
  "actionsDir": "/usr/share/uq/launcher/actions",
  "integrationsDir": "/usr/share/uq/integrations",
  "diagDir": "/usr/share/uq/diag",
  "themesDir": "/usr/share/uq/themes",
  "renderedThemesDir": "/usr/share/uq/themes/rendered"
});

export const dbus = Object.freeze({
  "desktopName": "org.uq.Desktop1",
  "desktopPath": "/org/uq/Desktop1",
  "desktopInterface": "org.uq.Desktop1",
  "launcherProviderPath": "/org/uq/Launcher1",
  "launcherProviderInterface": "org.uq.Launcher1.Provider"
});

export const api = Object.freeze({
  "uqApi": 3,
  "pageDefaultApi": 1
});

export const motion = Object.freeze({
  "fast": 100,
  "base": 140,
  "slow": 220,
  "popScale": 0.96,
  "easing": "OutCubic",
  "easingExit": "InCubic"
});

export const shape = Object.freeze({
  "fallbackRadius": 10,
  "controlInset": 2,
  "innerInset": 4,
  "gap": 8
});

export const typeScale = Object.freeze({
  "fallbackSize": 11,
  "smallOffset": 1,
  "smallMin": 8,
  "headingScale": 1.5
});

export const paletteKeys = Object.freeze(["bg", "surface", "surface_alt", "overlay", "text", "text_muted", "accent", "accent_text", "border", "success", "warning", "error"]);
export const palette = Object.freeze({
  dark: Object.freeze({
    "bg": "#1f1f22",
    "surface": "#2b2b2e",
    "surface_alt": "#353538",
    "overlay": "#464649",
    "text": "#e4e4e7",
    "text_muted": "#a9a9ac",
    "accent": "#8ab4f8",
    "accent_text": "#1f1f22",
    "border": "#5a5a5d",
    "success": "#81c995",
    "warning": "#fdd663",
    "error": "#f28b82"
  }),
  light: Object.freeze({
    "bg": "#f4f4f5",
    "surface": "#e7e7e8",
    "surface_alt": "#dcdcdd",
    "overlay": "#c8c8c9",
    "text": "#18181b",
    "text_muted": "#5a5a5c",
    "accent": "#2b5bb8",
    "accent_text": "#f4f4f5",
    "border": "#b2b2b4",
    "success": "#1e7a3a",
    "warning": "#8a6100",
    "error": "#b3261e"
  }),
});
