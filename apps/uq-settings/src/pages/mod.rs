//! Page models: the Rust side of pages that need one.

pub mod about;
pub mod appearance;
pub mod bluetooth;
pub mod compat;
pub mod datetime;
pub mod defaultapps;
pub mod displays;
#[cfg(test)]
pub mod fake_system;
#[cfg(test)]
pub mod fake_uqd;
pub mod history;
pub mod input;
pub mod keybinds;
pub mod login_screen;
pub mod look;
pub mod network;
pub mod power;
pub mod profiles;
pub mod prompt;
pub mod proxies;
pub mod region;
pub mod rules;
pub mod runner;
pub mod shell;
pub mod sound;
pub mod startup;
pub mod sysbus;
pub mod themes;
pub mod users;
pub mod wallpaper;
pub mod welcome;
