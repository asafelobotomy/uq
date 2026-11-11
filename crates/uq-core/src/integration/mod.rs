//! Integrations: the host side of the extension points other apps plug into (Phase 4 spec §2).
//!
//! One module for every host, so a widget and a settings page with the same manifest keys behave
//! the same way (§2.1):
//!
//! - [`manifest`]: `widget.toml` and `page.toml` types and their strict validation (§2.2, §2.3,
//!   §2.6; Phase 3 §2.5), and the folder loader with its symlink hygiene.
//! - [`appaction`]: `.appaction` files, app actions on launcher rows (§2.4.2).
//! - [`command`]: the declared-command runner (§2.3).
//! - [`dbus_json`]: JSON ↔ D-Bus values by signature, and deadline-bound calls (§2.2 limits).
//! - [`rules`]: `rules.toml` window rules and the `integration_rules` domain (§3.3).
//! - [`scan`]: everything found under the integration roots, valid or not (§2.7, §5).
//! - [`watch`]: a debounced inotify watch on the integration directories (§2.2, §2.3).
//!
//! Integration code is trusted package code: these helpers limit mistakes, not attacks (§1
//! non-goals, §6).

pub mod appaction;
pub mod command;
pub mod dbus_json;
pub mod manifest;
pub mod rules;
pub mod scan;
pub mod watch;

pub use manifest::{ManifestError, UQ_API};
