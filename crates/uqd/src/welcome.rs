//! Welcome's bookkeeping (`[welcome]` in `state.toml`, Phase 5 spec §5.1) for `GetWelcome`,
//! `SetWelcomeDone` and `ResetWelcome`.
//!
//! These writes skip `persist` on purpose, as launch counts do: whether Welcome ran is not a
//! setting, so it never appears in History, Undo never re-opens Welcome, and no restore point
//! is made for it.

use crate::service::{Desktop, DesktopError, io_err};
use uq_core::welcome::{CURRENT_WELCOME_VERSION, Welcome};

impl Desktop {
    pub fn welcome(&self) -> Welcome {
        self.state.welcome
    }

    /// The user finished or closed Welcome at `version` (1 ..= the current version).
    pub fn set_welcome_done(&mut self, version: u32) -> Result<(), DesktopError> {
        if version == 0 || version > CURRENT_WELCOME_VERSION {
            return Err(DesktopError::InvalidArgument(format!(
                "welcome version must be 1 to {CURRENT_WELCOME_VERSION}, not {version}"
            )));
        }
        self.save_welcome(Welcome {
            done: true,
            version,
        })
    }

    /// Show Welcome again from the start (`uqctl welcome --reset`, Settings → About).
    pub fn reset_welcome(&mut self) -> Result<(), DesktopError> {
        self.save_welcome(Welcome::default())
    }

    /// Save `state.toml` with `w`; on failure memory keeps the old value.
    fn save_welcome(&mut self, w: Welcome) -> Result<(), DesktopError> {
        if self.state.welcome == w {
            return Ok(());
        }
        let old = std::mem::replace(&mut self.state.welcome, w);
        if let Err(e) = self.state.save(&self.paths.state_file) {
            self.state.welcome = old;
            return Err(io_err("saving state")(e));
        }
        Ok(())
    }
}
