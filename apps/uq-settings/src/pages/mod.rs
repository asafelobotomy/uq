//! Page models: the Rust side of pages that need one.

pub mod appearance;
pub mod compat;
pub mod displays;
#[cfg(test)]
pub mod fake_system;
#[cfg(test)]
pub mod fake_uqd;
pub mod history;
pub mod input;
pub mod login_screen;
pub mod look;
pub mod power;
pub mod profiles;
pub mod proxies;
pub mod rules;
pub mod runner;
pub mod shell;
pub mod startup;
pub mod sysbus;
pub mod users;
pub mod wallpaper;
