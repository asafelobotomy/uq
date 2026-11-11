//! `uq-settings-helper`: uQ's only code that runs as root (Phase 3 spec §3, Phase 5 §4.2).
//!
//! A D-Bus-activated service on the system bus, `org.uq.SettingsHelper1`. Each method is one
//! closed operation: it checks its arguments against closed sets or strict patterns, asks
//! polkit whether the caller may do it, and then writes one fixed file from a fixed template.
//! Caller strings never become paths, and only become file content after validation.
//!
//! The layers, each testable on its own:
//! - [`values`]: the closed value sets and name patterns;
//! - [`templates`]: the file bodies rendered from validated values;
//! - [`write`]: atomic, symlink-safe writes;
//! - [`authority`]: the polkit check, behind a trait;
//! - [`passwd`]: the user lookup, behind a trait;
//! - [`service`]: the operations and the D-Bus interface;
//! - [`idle`]: the 30 s idle exit.

pub mod authority;
pub mod config;
pub mod idle;
pub mod passwd;
pub mod service;
pub mod templates;
pub mod values;
pub mod write;

pub use authority::{Action, Authority, Polkit};
pub use config::Config;
pub use service::{Helper, HelperError, serve};

/// The well-known bus name, object path and interface name.
pub const NAME: &str = "org.uq.SettingsHelper1";
pub const PATH: &str = "/org/uq/SettingsHelper1";
pub const IFACE: &str = "org.uq.SettingsHelper1";
