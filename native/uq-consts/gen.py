#!/usr/bin/env python3
"""Writes uq-consts' per-language files from consts.toml.

    native/uq-consts/gen.py          write them
    native/uq-consts/gen.py --check  exit 1 if any is out of date (CI)

Each output starts with a `uq-consts X.Y.Z` line, which check-uq-native.sh compares with the
version a repo pins. Python 3.11+ (tomllib), nothing else.
"""

import json
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE / "consts.toml"
OUTPUTS = ["uq_consts.rs", "uqconsts.h", "uq_consts.py", "uq-consts.js", "uq-consts.sh", "UqThemeBase.qml"]

# Groups of plain values (every key a constant), in output order. paths and palette are special.
GROUPS = ["install", "dbus", "api", "motion", "shape", "type"]
COLOR_KEYS = [
    "bg", "surface", "surface_alt", "overlay", "text", "text_muted",
    "accent", "accent_text", "border", "success", "warning", "error",
]


def camel(s):
    head, *rest = s.split("_")
    return head + "".join(w.capitalize() for w in rest)


def load():
    c = tomllib.loads(SRC.read_text())
    v = c["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", v):
        sys.exit(f"gen.py: version {v!r} is not X.Y.Z")
    for name in ("dark", "light"):
        if list(c["palette"][name]) != COLOR_KEYS:
            sys.exit(f"gen.py: palette.{name} must list exactly {COLOR_KEYS} in that order")
        for k, col in c["palette"][name].items():
            if not re.fullmatch(r"#[0-9a-f]{6}", col):
                sys.exit(f"gen.py: palette.{name}.{k} {col!r} is not #rrggbb")
    for g in GROUPS:
        for k, val in c[g].items():
            if isinstance(val, bool) or not isinstance(val, (int, float, str)):
                sys.exit(f"gen.py: {g}.{k} must be a number or a string")
            if isinstance(val, str) and not re.fullmatch(r"[A-Za-z0-9_./-]+", val):
                sys.exit(f"gen.py: {g}.{k} {val!r} has characters the generators don't quote")
    for base in ("state", "cache"):
        for k, val in c["paths"][base].items():
            if val.startswith("/") or ".." in val.split("/"):
                sys.exit(f"gen.py: paths.{base}.{k} must be relative")
    return c


def num(v):
    return repr(v) if isinstance(v, float) else str(v)


# ---------------------------------------------------------------------------------------- Rust

def rust(c):
    v = c["version"]
    o = [f"//! uq-consts {v}", "//!",
         "//! The values uQ, Bindle and uQ's native apps share (paths, install folders, D-Bus names, the",
         "//! `uq_api`, motion, shape, type and the fallback palette). Generated from uQ's",
         "//! `native/uq-consts/consts.toml` by gen.py: vendor this file unchanged and declare it with",
         "//! `#[rustfmt::skip] mod uq_consts;`. No dependencies; builds on editions 2021 and 2024.",
         "#![allow(dead_code)]", "",
         "/// The version of this file; `check-uq-native.sh` compares it with the one a repo pins.",
         f'pub const VERSION: &str = "{v}";', ""]
    o += ["/// Per-user paths. `state_home()` is `$XDG_STATE_HOME` when it is absolute, else",
          "/// `~/.local/state`; `cache_home()` likewise with `$XDG_CACHE_HOME` and `~/.cache`.",
          "pub mod paths {",
          "    use std::ffi::OsString;",
          "    use std::path::PathBuf;", ""]
    for base in ("state", "cache"):
        for k, val in c["paths"][base].items():
            o.append(f"    /// Relative to `{base}_home()`.")
            o.append(f'    pub const {k.upper()}: &str = "{val}";')
    o += ["",
          "    fn base(get: &dyn Fn(&str) -> Option<OsString>, var: &str, home_rel: &str) -> Option<PathBuf> {",
          "        match get(var).map(PathBuf::from).filter(|p| p.is_absolute()) {",
          "            Some(p) => Some(p),",
          "            None => get(\"HOME\").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(home_rel)),",
          "        }",
          "    }", "",
          "    /// `state_home()` from an environment lookup, for tests.",
          "    pub fn state_home_from(get: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {",
          '        base(get, "XDG_STATE_HOME", ".local/state")',
          "    }", "",
          "    /// `cache_home()` from an environment lookup, for tests.",
          "    pub fn cache_home_from(get: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {",
          '        base(get, "XDG_CACHE_HOME", ".cache")',
          "    }", "",
          "    pub fn state_home() -> Option<PathBuf> {",
          "        state_home_from(&|k| std::env::var_os(k))",
          "    }", "",
          "    pub fn cache_home() -> Option<PathBuf> {",
          "        cache_home_from(&|k| std::env::var_os(k))",
          "    }", ""]
    for base in ("state", "cache"):
        for k in c["paths"][base]:
            o += [f"    pub fn {k}() -> Option<PathBuf> {{",
                  f"        {base}_home().map(|b| b.join({k.upper()}))",
                  "    }"]
    o += ["}", ""]
    for g in GROUPS:
        mod = "type_scale" if g == "type" else g
        o.append(f"pub mod {mod} {{")
        for k, val in c[g].items():
            if isinstance(val, str):
                o.append(f'    pub const {k.upper()}: &str = "{val}";')
            elif isinstance(val, float):
                o.append(f"    pub const {k.upper()}: f64 = {num(val)};")
            else:
                o.append(f"    pub const {k.upper()}: u32 = {val};")
        o += ["}", ""]
    o += ["/// uQ's Graphite theme, the palette to fall back to where uQ isn't running.",
          "pub mod palette {",
          "    /// The colour tokens every uQ theme defines, in this order.",
          f"    pub const KEYS: [&str; {len(COLOR_KEYS)}] = [{', '.join(json.dumps(k) for k in COLOR_KEYS)}];"]
    for name in ("dark", "light"):
        pairs = ", ".join(f'("{k}", "{col}")' for k, col in c["palette"][name].items())
        o.append(f"    pub const {name.upper()}: [(&str, &str); {len(COLOR_KEYS)}] = [{pairs}];")
    o += ["}", ""]
    return "\n".join(o)


# ----------------------------------------------------------------------------------------- C++

def cpp(c):
    v = c["version"]
    o = [f"// uq-consts {v}", "//",
         "// The values uQ, Bindle and uQ's native apps share. Generated from uQ's",
         "// native/uq-consts/consts.toml by gen.py: vendor this header unchanged. Header-only; the path",
         "// functions need Qt Core.",
         "#pragma once", "",
         "#include <QDir>", "#include <QString>", "",
         "namespace uqconsts {", "",
         f'inline constexpr const char *version = "{v}";', "",
         "namespace paths {", ""]
    for base in ("state", "cache"):
        for k, val in c["paths"][base].items():
            o.append(f'inline constexpr const char *{camel(k)} = "{val}"; // relative to {base}Home()')
    o += ["",
          "inline QString base(const char *var, const char *homeRel)",
          "{",
          "    const QString v = qEnvironmentVariable(var);",
          "    if (QDir::isAbsolutePath(v))",
          "        return v;",
          "    return QDir::homePath() + QLatin1Char('/') + QLatin1String(homeRel);",
          "}", "",
          "/// $XDG_STATE_HOME when it is absolute, else ~/.local/state.",
          'inline QString stateHome() { return base("XDG_STATE_HOME", ".local/state"); }',
          "/// $XDG_CACHE_HOME when it is absolute, else ~/.cache.",
          'inline QString cacheHome() { return base("XDG_CACHE_HOME", ".cache"); }', ""]
    for base in ("state", "cache"):
        for k in c["paths"][base]:
            fn = camel(k) + "Path"
            o.append(f"inline QString {fn}() {{ return {base}Home() + QLatin1Char('/') + QLatin1String({camel(k)}); }}")
    o += ["", "} // namespace paths", ""]
    for g in GROUPS:
        ns = "typeScale" if g == "type" else g
        o.append(f"namespace {ns} {{")
        for k, val in c[g].items():
            if isinstance(val, str):
                o.append(f'inline constexpr const char *{camel(k)} = "{val}";')
            elif isinstance(val, float):
                o.append(f"inline constexpr double {camel(k)} = {num(val)};")
            else:
                o.append(f"inline constexpr int {camel(k)} = {val};")
        o += [f"}} // namespace {ns}", ""]
    o += ["/// uQ's Graphite theme, the palette to fall back to where uQ isn't running.",
          "namespace palette {",
          "struct Color {", "    const char *key;", "    const char *value;", "};"]
    for name in ("dark", "light"):
        o.append(f"inline constexpr Color {name}[] = {{")
        for k, col in c["palette"][name].items():
            o.append(f'    {{"{k}", "{col}"}},')
        o.append("};")
    o += ["} // namespace palette", "", "} // namespace uqconsts", ""]
    return "\n".join(o)


# -------------------------------------------------------------------------------------- Python

def python(c):
    v = c["version"]
    o = [f'"""uq-consts {v}', "",
         "The values uQ, Bindle and uQ's native apps share. Generated from uQ's",
         "native/uq-consts/consts.toml by gen.py: vendor this file unchanged. Standard library only.",
         '"""', "", "import os", "from pathlib import Path", "",
         f'VERSION = "{v}"', ""]
    for base in ("state", "cache"):
        for k, val in c["paths"][base].items():
            o.append(f'{base.upper()}_{k.upper()} = "{val}"  # relative to {base}_home()')
    o += ["", "",
          "def _base(env, var, home_rel):",
          "    v = env.get(var, \"\")",
          "    if os.path.isabs(v):",
          "        return Path(v)",
          "    home = env.get(\"HOME\", \"\")",
          "    return Path(home) / home_rel if home else None",
          "", "",
          "def state_home(env=None):",
          '    """$XDG_STATE_HOME when it is absolute, else ~/.local/state (None without HOME)."""',
          '    return _base(os.environ if env is None else env, "XDG_STATE_HOME", ".local/state")',
          "", "",
          "def cache_home(env=None):",
          '    """$XDG_CACHE_HOME when it is absolute, else ~/.cache (None without HOME)."""',
          '    return _base(os.environ if env is None else env, "XDG_CACHE_HOME", ".cache")',
          ""]
    for base in ("state", "cache"):
        for k in c["paths"][base]:
            o += ["", f"def {k}(env=None):",
                  f"    b = {base}_home(env)",
                  f"    return b / {base.upper()}_{k.upper()} if b else None", ""]
    o.append("")
    for g in GROUPS:
        for k, val in c[g].items():
            o.append(f"{g.upper()}_{k.upper()} = {json.dumps(val) if isinstance(val, str) else num(val)}")
        o.append("")
    o.append(f"PALETTE_KEYS = ({', '.join(json.dumps(k) for k in COLOR_KEYS)})")
    for name in ("dark", "light"):
        o.append(f"PALETTE_{name.upper()} = {{")
        for k, col in c["palette"][name].items():
            o.append(f'    "{k}": "{col}",')
        o.append("}")
    o.append("")
    return "\n".join(o)


# ------------------------------------------------------------------------------------------ JS

def js(c):
    v = c["version"]
    o = [f"// uq-consts {v}",
         "// The values uQ, Bindle and uQ's native apps share. Generated from uQ's",
         "// native/uq-consts/consts.toml by gen.py: vendor this file unchanged. An ES module with no",
         "// dependencies and no file system access (paths are relative to the XDG base directories).",
         "", f'export const VERSION = "{v}";', ""]
    for base in ("state", "cache"):
        obj = {camel(k): val for k, val in c["paths"][base].items()}
        o.append(f"export const {base}Paths = Object.freeze({json.dumps(obj, indent=2)});")
        o.append("")
    for g in GROUPS:
        name = "typeScale" if g == "type" else g
        obj = {camel(k): val for k, val in c[g].items()}
        o.append(f"export const {name} = Object.freeze({json.dumps(obj, indent=2)});")
        o.append("")
    o.append(f"export const paletteKeys = Object.freeze({json.dumps(COLOR_KEYS)});")
    o.append("export const palette = Object.freeze({")
    for name in ("dark", "light"):
        o.append(f"  {name}: Object.freeze({json.dumps(c['palette'][name], indent=4)[:-1]}  }}),")
    o += ["});", ""]
    return "\n".join(o)


# --------------------------------------------------------------------------------------- shell

def shell(c):
    v = c["version"]
    o = [f"# uq-consts {v}",
         "# The values uQ, Bindle and uQ's native apps share, for shell scripts and PKGBUILDs.",
         "# Generated from uQ's native/uq-consts/consts.toml by gen.py: vendor this file unchanged and",
         "# source it (`. scripts/uq-consts.sh`). It only sets variables and defines functions.",
         "# shellcheck shell=sh disable=SC2034", "",
         f"UQ_CONSTS_VERSION={v}"]
    for base in ("state", "cache"):
        for k, val in c["paths"][base].items():
            o.append(f"UQ_{base.upper()}_{k.upper()}={val}")
    for g in GROUPS:
        for k, val in c[g].items():
            o.append(f"UQ_{g.upper()}_{k.upper()}={val}")
    for name in ("dark", "light"):
        for k, col in c["palette"][name].items():
            o.append(f"UQ_PALETTE_{name.upper()}_{k.upper()}='{col}'")
    o += ["",
          "# $XDG_STATE_HOME when it is absolute, else ~/.local/state.",
          'uq_state_home() { case "${XDG_STATE_HOME:-}" in /*) printf %s "$XDG_STATE_HOME" ;; *) printf %s "$HOME/.local/state" ;; esac; }',
          "# $XDG_CACHE_HOME when it is absolute, else ~/.cache.",
          'uq_cache_home() { case "${XDG_CACHE_HOME:-}" in /*) printf %s "$XDG_CACHE_HOME" ;; *) printf %s "$HOME/.cache" ;; esac; }']
    for base in ("state", "cache"):
        for k in c["paths"][base]:
            o.append(f'uq_{k}() {{ printf %s/%s "$(uq_{base}_home)" "$UQ_{base.upper()}_{k.upper()}"; }}')
    o.append("")
    return "\n".join(o)


# ----------------------------------------------------------------------------------------- QML

def qml(c):
    v = c["version"]
    m, s, t, d = c["motion"], c["shape"], c["type"], c["palette"]["dark"]
    s_radius = s["fallback_radius"]
    o = [f"// uq-consts {v}",
         "// The base of a uQ native app's QML Theme singleton. Generated from uQ's",
         "// native/uq-consts/consts.toml by gen.py: vendor it unchanged next to your Theme.qml and",
         "// register it in the same QML module. Then Theme.qml is:",
         "//",
         "//     pragma Singleton",
         "//     import QtQuick",
         "//     UqThemeBase { tokens: App.theme.tokens }   // uq-theme's tokens, plus your own extras",
         "//",
         "// On uQ every value follows uQ's theme (and Reduce motion) live; elsewhere it is uQ's",
         "// Graphite palette with the app font, so the app looks the same as uQ's own apps.",
         "import QtQuick", "",
         "QtObject {",
         "    // uq-theme's tokens (load() / Watcher.tokens); {present: false} off uQ.",
         "    property var tokens: ({ present: false })",
         "    // Off uQ: colours to use instead of Graphite's, for an app that follows the desktop's own",
         "    // palette there (for example Kirigami.Theme), as {bg: ..., accent: ...}. Missing keys",
         "    // stay Graphite. Don't derive them from this object's own colours (a binding loop).",
         "    property var fallbackColors: ({})",
         f"    property int fallbackRadius: {s_radius}",
         "    readonly property bool uq: !!tokens && tokens.present === true",
         '    readonly property bool dark: !uq || tokens.variant !== "light"', "",
         "    function pick(key, fallback) {",
         "        if (uq && tokens.colors && tokens.colors[key])",
         "            return tokens.colors[key];",
         "        return fallbackColors && fallbackColors[key] ? fallbackColors[key] : fallback;",
         "    }",
         "    function num(key, fallback) {",
         '        return uq && typeof tokens[key] === "number" ? tokens[key] : fallback;',
         "    }", ""]
    for k, col in d.items():
        o.append(f'    readonly property color {camel(k)}: pick("{k}", "{col}")')
    o += ["",
          "    // Corners: containers use radius, a row, button or field inside one radiusControl, a chip",
          "    // or thumbnail inside a row radiusInner, so nested corners stay concentric.",
          "    readonly property int radius: num(\"radius\", fallbackRadius)",
          f"    readonly property int radiusControl: Math.max(0, radius - {s['control_inset']})",
          f"    readonly property int radiusInner: Math.max(0, radius - {s['inner_inset']})",
          '    readonly property int gapIn: num("gapIn", 5)',
          '    readonly property int gapOut: num("gapOut", 12)',
          '    readonly property int borderWidth: num("borderWidth", 1)',
          f"    readonly property int gap: {s['gap']}", "",
          "    readonly property string fontUi: uq && tokens.fontUi ? tokens.fontUi : Qt.application.font.family",
          '    readonly property string fontMono: uq && tokens.fontMono ? tokens.fontMono : "monospace"',
          "    readonly property real bodySize: uq && tokens.fontSize > 0 ? tokens.fontSize",
          f"        : (Qt.application.font.pointSize > 0 ? Qt.application.font.pointSize : {t['fallback_size']})",
          f"    readonly property real smallSize: Math.max({t['small_min']}, bodySize - {t['small_offset']})",
          f"    readonly property real headingSize: Math.round(bodySize * {num(t['heading_scale'])})", "",
          "    // Motion: the same speeds and curves as uQ's shell. Use these, not literal ms. All 0 when",
          "    // the user turned on Reduce motion in uQ.",
          "    readonly property var motion: uq && tokens.motion ? tokens.motion : ({})",
          "    readonly property bool reduceMotion: motion.reduce === true",
          f'    readonly property int motionFast: reduceMotion ? 0 : (motion.fast >= 0 ? motion.fast : {m["fast"]})',
          f'    readonly property int motionBase: reduceMotion ? 0 : (motion.base >= 0 ? motion.base : {m["base"]})',
          f'    readonly property int motionSlow: reduceMotion ? 0 : (motion.slow >= 0 ? motion.slow : {m["slow"]})',
          f"    readonly property int easing: Easing.{m['easing']}",
          f"    readonly property int easingExit: Easing.{m['easing_exit']}",
          f"    readonly property real popScale: reduceMotion ? 1 : {num(m['pop_scale'])}",
          "}", ""]
    return "\n".join(o)


def main():
    c = load()
    files = {"uq_consts.rs": rust(c), "uqconsts.h": cpp(c), "uq_consts.py": python(c),
             "uq-consts.js": js(c), "uq-consts.sh": shell(c), "UqThemeBase.qml": qml(c)}
    assert list(files) == OUTPUTS
    stale = []
    for name, body in files.items():
        p = HERE / name
        if "--check" in sys.argv[1:]:
            if not p.exists() or p.read_text() != body:
                stale.append(name)
        else:
            p.write_text(body)
    if stale:
        sys.exit(f"gen.py: out of date: {', '.join(stale)} (run native/uq-consts/gen.py)")


if __name__ == "__main__":
    main()
