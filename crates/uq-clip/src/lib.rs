//! uQ's clipboard manager. `uq-clipd` watches the Wayland clipboard and keeps a private history;
//! `uqctl clip` and the shell talk to it over a socket (see [`ipc`]).

pub mod config;
pub mod daemon;
pub mod ipc;
pub mod privacy;
pub mod store;
pub mod watch;
