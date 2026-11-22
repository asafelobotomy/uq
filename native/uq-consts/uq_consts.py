"""uq-consts 1.0.0

The values uQ, Bindle and uQ's native apps share. Generated from uQ's
native/uq-consts/consts.toml by gen.py: vendor this file unchanged. Standard library only.
"""

import os
from pathlib import Path

VERSION = "1.0.0"

STATE_UQ_DIR = "uq"  # relative to state_home()
STATE_THEME_DIR = "uq/theme"  # relative to state_home()
STATE_TOKENS = "uq/theme/tokens.json"  # relative to state_home()
STATE_CRASHES_DIR = "uq/crashes"  # relative to state_home()
CACHE_QUICKSHELL_CRASHES_DIR = "quickshell/crashes"  # relative to cache_home()


def _base(env, var, home_rel):
    v = env.get(var, "")
    if os.path.isabs(v):
        return Path(v)
    home = env.get("HOME", "")
    return Path(home) / home_rel if home else None


def state_home(env=None):
    """$XDG_STATE_HOME when it is absolute, else ~/.local/state (None without HOME)."""
    return _base(os.environ if env is None else env, "XDG_STATE_HOME", ".local/state")


def cache_home(env=None):
    """$XDG_CACHE_HOME when it is absolute, else ~/.cache (None without HOME)."""
    return _base(os.environ if env is None else env, "XDG_CACHE_HOME", ".cache")


def uq_dir(env=None):
    b = state_home(env)
    return b / STATE_UQ_DIR if b else None


def theme_dir(env=None):
    b = state_home(env)
    return b / STATE_THEME_DIR if b else None


def tokens(env=None):
    b = state_home(env)
    return b / STATE_TOKENS if b else None


def crashes_dir(env=None):
    b = state_home(env)
    return b / STATE_CRASHES_DIR if b else None


def quickshell_crashes_dir(env=None):
    b = cache_home(env)
    return b / CACHE_QUICKSHELL_CRASHES_DIR if b else None


INSTALL_DATA_DIR = "/usr/share/uq"
INSTALL_PAGES_DIR = "/usr/share/uq/settings/pages"
INSTALL_WIDGETS_DIR = "/usr/share/uq/shell/widgets"
INSTALL_LAUNCHER_DIR = "/usr/share/uq/launcher"
INSTALL_ACTIONS_DIR = "/usr/share/uq/launcher/actions"
INSTALL_INTEGRATIONS_DIR = "/usr/share/uq/integrations"
INSTALL_DIAG_DIR = "/usr/share/uq/diag"
INSTALL_THEMES_DIR = "/usr/share/uq/themes"
INSTALL_RENDERED_THEMES_DIR = "/usr/share/uq/themes/rendered"

DBUS_DESKTOP_NAME = "org.uq.Desktop1"
DBUS_DESKTOP_PATH = "/org/uq/Desktop1"
DBUS_DESKTOP_INTERFACE = "org.uq.Desktop1"
DBUS_LAUNCHER_PROVIDER_PATH = "/org/uq/Launcher1"
DBUS_LAUNCHER_PROVIDER_INTERFACE = "org.uq.Launcher1.Provider"

API_UQ_API = 3
API_PAGE_DEFAULT_API = 1

MOTION_FAST = 100
MOTION_BASE = 140
MOTION_SLOW = 220
MOTION_POP_SCALE = 0.96
MOTION_EASING = "OutCubic"
MOTION_EASING_EXIT = "InCubic"

SHAPE_FALLBACK_RADIUS = 10
SHAPE_CONTROL_INSET = 2
SHAPE_INNER_INSET = 4
SHAPE_GAP = 8

TYPE_FALLBACK_SIZE = 11
TYPE_SMALL_OFFSET = 1
TYPE_SMALL_MIN = 8
TYPE_HEADING_SCALE = 1.5

PALETTE_KEYS = ("bg", "surface", "surface_alt", "overlay", "text", "text_muted", "accent", "accent_text", "border", "success", "warning", "error")
PALETTE_DARK = {
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
    "error": "#f28b82",
}
PALETTE_LIGHT = {
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
    "error": "#b3261e",
}
