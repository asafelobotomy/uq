# uq-consts 1.0.0
# The values uQ, Bindle and uQ's native apps share, for shell scripts and PKGBUILDs.
# Generated from uQ's native/uq-consts/consts.toml by gen.py: vendor this file unchanged and
# source it (`. scripts/uq-consts.sh`). It only sets variables and defines functions.
# shellcheck shell=sh disable=SC2034

UQ_CONSTS_VERSION=1.0.0
UQ_STATE_UQ_DIR=uq
UQ_STATE_THEME_DIR=uq/theme
UQ_STATE_TOKENS=uq/theme/tokens.json
UQ_STATE_CRASHES_DIR=uq/crashes
UQ_CACHE_QUICKSHELL_CRASHES_DIR=quickshell/crashes
UQ_INSTALL_DATA_DIR=/usr/share/uq
UQ_INSTALL_PAGES_DIR=/usr/share/uq/settings/pages
UQ_INSTALL_WIDGETS_DIR=/usr/share/uq/shell/widgets
UQ_INSTALL_LAUNCHER_DIR=/usr/share/uq/launcher
UQ_INSTALL_ACTIONS_DIR=/usr/share/uq/launcher/actions
UQ_INSTALL_INTEGRATIONS_DIR=/usr/share/uq/integrations
UQ_INSTALL_DIAG_DIR=/usr/share/uq/diag
UQ_INSTALL_THEMES_DIR=/usr/share/uq/themes
UQ_INSTALL_RENDERED_THEMES_DIR=/usr/share/uq/themes/rendered
UQ_DBUS_DESKTOP_NAME=org.uq.Desktop1
UQ_DBUS_DESKTOP_PATH=/org/uq/Desktop1
UQ_DBUS_DESKTOP_INTERFACE=org.uq.Desktop1
UQ_DBUS_LAUNCHER_PROVIDER_PATH=/org/uq/Launcher1
UQ_DBUS_LAUNCHER_PROVIDER_INTERFACE=org.uq.Launcher1.Provider
UQ_API_UQ_API=3
UQ_API_PAGE_DEFAULT_API=1
UQ_MOTION_FAST=100
UQ_MOTION_BASE=140
UQ_MOTION_SLOW=220
UQ_MOTION_POP_SCALE=0.96
UQ_MOTION_EASING=OutCubic
UQ_MOTION_EASING_EXIT=InCubic
UQ_SHAPE_FALLBACK_RADIUS=10
UQ_SHAPE_CONTROL_INSET=2
UQ_SHAPE_INNER_INSET=4
UQ_SHAPE_GAP=8
UQ_TYPE_FALLBACK_SIZE=11
UQ_TYPE_SMALL_OFFSET=1
UQ_TYPE_SMALL_MIN=8
UQ_TYPE_HEADING_SCALE=1.5
UQ_PALETTE_DARK_BG='#1f1f22'
UQ_PALETTE_DARK_SURFACE='#2b2b2e'
UQ_PALETTE_DARK_SURFACE_ALT='#353538'
UQ_PALETTE_DARK_OVERLAY='#464649'
UQ_PALETTE_DARK_TEXT='#e4e4e7'
UQ_PALETTE_DARK_TEXT_MUTED='#a9a9ac'
UQ_PALETTE_DARK_ACCENT='#8ab4f8'
UQ_PALETTE_DARK_ACCENT_TEXT='#1f1f22'
UQ_PALETTE_DARK_BORDER='#5a5a5d'
UQ_PALETTE_DARK_SUCCESS='#81c995'
UQ_PALETTE_DARK_WARNING='#fdd663'
UQ_PALETTE_DARK_ERROR='#f28b82'
UQ_PALETTE_LIGHT_BG='#f4f4f5'
UQ_PALETTE_LIGHT_SURFACE='#e7e7e8'
UQ_PALETTE_LIGHT_SURFACE_ALT='#dcdcdd'
UQ_PALETTE_LIGHT_OVERLAY='#c8c8c9'
UQ_PALETTE_LIGHT_TEXT='#18181b'
UQ_PALETTE_LIGHT_TEXT_MUTED='#5a5a5c'
UQ_PALETTE_LIGHT_ACCENT='#2b5bb8'
UQ_PALETTE_LIGHT_ACCENT_TEXT='#f4f4f5'
UQ_PALETTE_LIGHT_BORDER='#b2b2b4'
UQ_PALETTE_LIGHT_SUCCESS='#1e7a3a'
UQ_PALETTE_LIGHT_WARNING='#8a6100'
UQ_PALETTE_LIGHT_ERROR='#b3261e'

# $XDG_STATE_HOME when it is absolute, else ~/.local/state.
uq_state_home() { case "${XDG_STATE_HOME:-}" in /*) printf %s "$XDG_STATE_HOME" ;; *) printf %s "$HOME/.local/state" ;; esac; }
# $XDG_CACHE_HOME when it is absolute, else ~/.cache.
uq_cache_home() { case "${XDG_CACHE_HOME:-}" in /*) printf %s "$XDG_CACHE_HOME" ;; *) printf %s "$HOME/.cache" ;; esac; }
uq_uq_dir() { printf %s/%s "$(uq_state_home)" "$UQ_STATE_UQ_DIR"; }
uq_theme_dir() { printf %s/%s "$(uq_state_home)" "$UQ_STATE_THEME_DIR"; }
uq_tokens() { printf %s/%s "$(uq_state_home)" "$UQ_STATE_TOKENS"; }
uq_crashes_dir() { printf %s/%s "$(uq_state_home)" "$UQ_STATE_CRASHES_DIR"; }
uq_quickshell_crashes_dir() { printf %s/%s "$(uq_cache_home)" "$UQ_CACHE_QUICKSHELL_CRASHES_DIR"; }
