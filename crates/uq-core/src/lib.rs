//! uQ core library: theme model, renderers, Lua emitter, state and schedule.
pub mod atomic;
pub mod autostart;
pub mod client;
pub mod compat;
pub mod contrast;
pub mod desktop_entry;
pub mod diag;
pub mod env;
pub mod flatpak;
pub mod fuzzy;
pub mod gestures;
pub mod history;
pub mod integration;
pub mod kdeglobals;
pub mod keybinds;
pub mod launcher_providers;
pub mod lua;
pub mod mimeapps;
pub mod paths;
pub mod profile;
pub mod qt6ct;
pub mod reclaim;
pub mod render;
pub mod rules;
pub mod schedule;
pub mod settings;
pub mod startup_explain;
pub mod state;
pub mod stub;
pub mod terminal_pref;
#[cfg(feature = "test-bus")]
pub mod test_bus;
pub mod theme;
pub mod welcome;

/// uq-consts, the values uQ shares with Bindle and its apps (`native/uq-consts`). uQ compiles the
/// same generated file the apps vendor, so its constants can't drift from theirs.
#[path = "../../../native/uq-consts/uq_consts.rs"]
#[rustfmt::skip]
pub mod uq_consts;
