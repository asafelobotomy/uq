//! Page models: the Rust side of pages that need one.

pub mod appearance;
pub mod displays;
#[cfg(test)]
pub mod fake_uqd;
pub mod history;
pub mod input;
pub mod look;
#[allow(
    dead_code,
    reason = "the Profiles page model (slice B, Task 8) wires it to QML"
)]
pub mod profiles;
pub mod runner;
pub mod shell;
pub mod wallpaper;
