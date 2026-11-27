//! The fake uqd's `Undo` and `Restore`, as uqd does them: conflicts, the keys an entry changed,
//! and writes with origin `restore`.

use super::*;

impl FakeUqd {
    pub(super) fn undo_now(&self, entry_id: &str, force: bool) -> Result<String, FakeError> {
        let entry = {
            let entries = self.history.entries.lock().unwrap();
            let Some(pos) = entries.iter().position(|e| e.id == entry_id) else {
                return Err(FakeError::InvalidArgument(format!(
                    "no history entry {entry_id:?}"
                )));
            };
            let entry = entries[pos].clone();
            if !force && entries[pos + 1..].iter().any(|e| e.domain == entry.domain) {
                return Err(FakeError::Conflict(format!(
                    "{} was changed again after this. Undo anyway?",
                    entry.domain
                )));
            }
            entry
        };
        let before = entry.before.clone();
        let ids = self.write("restore", |s| match entry.domain.as_str() {
            "theme" => {
                s.theme = serde_json::from_value(before["theme"].clone()).map_err(invalid)?;
                s.variant = serde_json::from_value(before["variant"].clone()).map_err(invalid)?;
                Ok(())
            }
            "schedule" => {
                s.schedule = serde_json::from_value(before).map_err(invalid)?;
                Ok(())
            }
            "settings" => {
                for (k, v) in before.as_object().into_iter().flatten() {
                    if entry.after.get(k) != Some(v) {
                        s.settings.apply(k, v.clone()).map_err(invalid)?;
                    }
                }
                Ok(())
            }
            domain => {
                let current = s.section(domain).map_err(invalid)?;
                s.set_section(domain, revert_changed(&current, &before, &entry.after))
                    .map_err(invalid)
            }
        })?;
        Ok(ids.last().cloned().unwrap_or_default())
    }

    pub(super) fn restore_now(&self, id: &str) -> Result<(), FakeError> {
        let next = {
            let points = self.history.points.lock().unwrap();
            let Some((_, state)) = points.iter().find(|(p, _)| p.id == id) else {
                return Err(FakeError::InvalidArgument(format!("no restore point {id}")));
            };
            state.clone()
        };
        // As uqd: save the current settings first, so the restore can be reversed.
        let current = self.state.lock().unwrap().clone();
        self.history.add_point(
            "Before restoring a restore point",
            PointKind::Auto(AutoReason::BeforeRestore),
            current,
        );
        self.write("restore", |s| {
            let launch_counts = std::mem::take(&mut s.launch_counts);
            *s = next;
            s.launch_counts = launch_counts;
            Ok(())
        })?;
        Ok(())
    }
}
