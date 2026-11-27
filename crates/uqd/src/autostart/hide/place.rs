//! Restore last session: move a reopened app's windows to the workspace they were on.

use super::{GRACE, POLL, SystemHider, WINDOW_WAIT, lua_string};
use std::collections::BTreeSet;

/// The Lua that moves a window (an address that passed `valid_address`) to a numbered
/// workspace without following it.
pub fn move_lua(address: &str, workspace: i64) -> String {
    format!(
        "return hl.dispatch(hl.dsp.window.move({{ workspace = \"{workspace}\", follow = false, window = {} }}))",
        lua_string(&format!("address:{address}"))
    )
}

impl SystemHider {
    /// Move the windows of `unit` that open within [`WINDOW_WAIT`] (and the [`GRACE`] after the
    /// first) to `workspace`. A window already there is left alone.
    pub(super) async fn place_windows(&self, unit: &str, workspace: i64) -> bool {
        if !(1..=1000).contains(&workspace) {
            return false;
        }
        let start = tokio::time::Instant::now();
        let mut seen = BTreeSet::new();
        let mut first: Option<tokio::time::Instant> = None;
        let mut moved = false;
        loop {
            let over = match first {
                Some(f) => f.elapsed() >= GRACE,
                None => start.elapsed() >= WINDOW_WAIT,
            };
            if over {
                return moved;
            }
            let Ok(fresh) = self.fresh(unit, &seen).await else {
                return moved;
            };
            for c in fresh {
                seen.insert(c.address.clone());
                first.get_or_insert(tokio::time::Instant::now());
                if c.workspace == workspace {
                    continue;
                }
                match self.hypr.eval(&move_lua(&c.address, workspace)).await {
                    Ok(()) => moved = true,
                    Err(e) => {
                        tracing::warn!("[START-003] restore last session: moving a window: {e}");
                        return moved;
                    }
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_move_is_silent_and_by_address() {
        assert_eq!(
            move_lua("0x55aa", 3),
            "return hl.dispatch(hl.dsp.window.move({ workspace = \"3\", follow = false, window = \"address:0x55aa\" }))"
        );
    }
}
