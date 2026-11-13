//! The one table of Hyprland names that shortcuts render.
//!
//! Read from the pinned series' own source, at both tags:
//! - 0.56.2 (`efb50993780079460b0cbed1363e2166a2de1d9f`),
//!   `src/config/lua/bindings/LuaBindingsToplevel.cpp`: `hlBind` at line 132 (options read with
//!   `getBool`/`readOptString` at lines 162-249), `parseKeyString` modifier names at lines
//!   30-46, registered at line 537;
//! - 0.57.0 (`0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`), the same file: options at lines
//!   104-203 (`mouse` is read at line 140), registered at line 472. 0.56.2 never reads `mouse`.
//! - `hl.unbind`, `hl.define_submap` and `hl.dsp.submap` exist at both tags
//!   (`LuaBindingsToplevel.cpp`, `LuaBindingsDispatchers.cpp`).
//! - `hyprctl binds -j` prints `modmask`, `key`, `description`, `dispatcher` and `arg` per bind:
//!   `src/debug/HyprCtl.cpp`, `bindsRequest`, line 1026 at 0.56.2; at 0.57.0 the same function
//!   is in `src/ipc/s1/Commands.cpp` (line 1199), and still prints `description` (the bind's
//!   `METADATA.description`, `""` when unset). `key` is empty for a `code:<n>` bind.
//! - Modifier bits are Hyprland's `eKeyboardModifiers`, read at both tags: 0.56.2
//!   `src/devices/IKeyboard.hpp` lines 13-22, 0.57.0 `src/input/Keys.hpp` lines 8-18 (moved):
//!   `SHIFT` 1<<0, `CAPS` 1<<1, `CTRL` 1<<2, `ALT` 1<<3, `MOD2` 1<<4, `MOD3` 1<<5,
//!   `META` (Super) 1<<6, `MOD5` 1<<7, so `binds -j`'s `modmask` maps straight back.
//!
//! An unknown or misspelled `hl.bind` option is silently ignored by Hyprland (`--verify-config`
//! passes it), so every name here is spelled with `_` and tested.

pub struct OptionName {
    pub lua: &'static str,
    pub source: &'static str,
}

const fn opt(lua: &'static str, source: &'static str) -> OptionName {
    OptionName { lua, source }
}

/// The option names uQ renders, in render order.
pub const BIND_OPTIONS: [OptionName; 5] = [
    opt("locked", "LuaBindingsToplevel.cpp 0.56.2:189, 0.57.0:131"),
    opt(
        "repeating",
        "LuaBindingsToplevel.cpp 0.56.2:188, 0.57.0:130",
    ),
    opt("release", "LuaBindingsToplevel.cpp 0.56.2:190, 0.57.0:132"),
    opt(
        "mouse",
        "LuaBindingsToplevel.cpp 0.57.0:140 (ignored at 0.56.2)",
    ),
    opt(
        "description",
        "LuaBindingsToplevel.cpp 0.56.2:199, 0.57.0:154",
    ),
];

/// Every option key `hl.bind` reads (both tags), for the mock's whitelist.
pub const ALL_BIND_OPTIONS: [&str; 17] = [
    "repeating",
    "locked",
    "release",
    "non_consuming",
    "auto_consuming",
    "transparent",
    "ignore_mods",
    "dont_inhibit",
    "long_press",
    "submap_universal",
    "click",
    "drag",
    "allow_input_capture",
    "mouse",
    "description",
    "desc",
    "device",
];

/// Canonical name, Hyprland's modifier bit, aliases. Render order: SUPER, SHIFT, CTRL, ALT.
pub const MODIFIERS: [(&str, u8, &[&str]); 4] = [
    ("SUPER", 64, &["WIN", "LOGO", "MOD4", "META"]),
    ("SHIFT", 1, &[]),
    ("CTRL", 4, &["CONTROL"]),
    ("ALT", 8, &["MOD1"]),
];

/// Hyprland accepts these; uQ doesn't (R3).
pub const REFUSED_MODIFIERS: [&str; 4] = ["CAPS", "MOD2", "MOD3", "MOD5"];

/// The `GlobalShortcut` names the shell registers (`appid: "uq"`).
pub const SHELL_NAMES: [&str; 14] = [
    "spotlight",
    "emoji",
    "appmenu",
    "quick-settings",
    "notif-centre",
    "lock",
    "power-menu",
    "edit-bar",
    "vol-up",
    "vol-down",
    "mute",
    "mic-mute",
    "bright-up",
    "bright-down",
];

/// The dispatchers uQ renders (`hl.dsp.<name>`), from `LuaBindingsDispatchers.cpp`.
pub const DISPATCHERS: [&str; 11] = [
    "exec_cmd",
    "global",
    "focus",
    "submap",
    "window.close",
    "window.float",
    "window.move",
    "window.pin",
    "window.drag",
    "window.resize",
    "workspace.toggle_special",
];

pub const DESCRIPTION_PREFIX: &str = "uq:";
pub const CAPTURE_SUBMAP: &str = "uq_capture";
/// The one bind inside the capture submap. Hyprland registers a submap when a bind is added to it
/// (an empty one gives "submap doesn't exist (wasn't registered!)" on `hl.dsp.submap`, seen in
/// CI's live checks), so the submap holds a bind on a key nobody has, which only leaves it.
pub const CAPTURE_PLACEHOLDER: &str =
    "hl.bind(\"SUPER + SHIFT + CTRL + ALT + XF86Launch9\", hl.dsp.submap(\"reset\"))";
pub const UNBIND_ALL: &str = "hl.unbind(\"all\")";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn snake(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    }

    #[test]
    fn option_names_use_underscores() {
        for o in BIND_OPTIONS {
            assert!(snake(o.lua), "{}", o.lua);
        }
        for o in ALL_BIND_OPTIONS {
            assert!(snake(o), "{o}");
        }
    }

    #[test]
    fn bind_options_are_known() {
        for o in BIND_OPTIONS {
            assert!(ALL_BIND_OPTIONS.contains(&o.lua), "{}", o.lua);
        }
    }

    #[test]
    fn modifier_aliases_are_unique() {
        let mut seen = BTreeSet::new();
        let mut bits = 0u8;
        for (name, bit, aliases) in MODIFIERS {
            assert_eq!(bits & bit, 0, "{name} repeats a bit");
            bits |= bit;
            for n in std::iter::once(&name).chain(aliases) {
                assert!(seen.insert(*n), "{n} repeats");
            }
        }
        for r in REFUSED_MODIFIERS {
            assert!(seen.insert(r), "{r} is also accepted");
        }
    }

    #[test]
    fn shell_names_match_the_shell() {
        let mut found = BTreeSet::new();
        for f in [
            "shell/launcher/LauncherRoutes.qml",
            "shell/osd/OsdRoutes.qml",
        ] {
            let text = std::fs::read_to_string(repo().join(f)).expect(f);
            // One GlobalShortcut block: collect its `appid` and `name`.
            for block in text.split("GlobalShortcut {").skip(1) {
                let block = block.split('}').next().unwrap_or("");
                let get = |k: &str| {
                    block.lines().find_map(|l| {
                        let l = l.trim().strip_prefix(k)?.trim_start();
                        let v = l.strip_prefix(':')?.trim();
                        Some(v.trim_matches('"').to_string())
                    })
                };
                if get("appid").as_deref() == Some("uq")
                    && let Some(n) = get("name")
                {
                    found.insert(n);
                }
            }
        }
        let want: BTreeSet<String> = SHELL_NAMES.iter().map(|s| s.to_string()).collect();
        assert_eq!(found, want);
    }
}
