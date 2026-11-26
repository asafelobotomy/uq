//! Edits of the bar as Settings sends them, applied to the layout uqd has.

use super::*;

/// One edit of the bar as Settings asks for it, applied to the layout as uqd has it now. Each
/// names the island it means (`id` at `zone`/`index`), so an edit made on an older view is
/// refused instead of changing another island.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BarEdit {
    /// Move to `to_index` of `to_zone` (counted after taking it out).
    Move {
        id: String,
        zone: String,
        index: usize,
        to_zone: String,
        to_index: usize,
    },
    /// Add at `index` of `zone`, or at its end.
    Add {
        id: String,
        zone: String,
        #[serde(default)]
        index: Option<usize>,
    },
    Remove {
        id: String,
        zone: String,
        index: usize,
    },
    Size {
        id: String,
        zone: String,
        index: usize,
        size: Size,
        /// The width for a Custom size, in Icon sizes.
        #[serde(default)]
        units: Option<u32>,
    },
    /// Replace `options.<island>`.
    Options { island: String, value: Value },
    /// Back to a new user's bar (options kept).
    Reset,
}

pub(crate) fn stale() -> SettingError {
    invalid("shell.bar", "the bar changed meanwhile; try again")
}

impl BarLayout {
    pub(crate) fn check_at(&self, id: &str, zone: &str, index: usize) -> Result<(), SettingError> {
        let z = self
            .zone(zone)
            .ok_or_else(|| invalid("shell.bar", format!("unknown zone {zone:?}")))?;
        match z.get(index) {
            Some(it) if it.id == id => Ok(()),
            _ => Err(stale()),
        }
    }

    /// Apply `edit`; on error `self` is unchanged.
    pub fn apply(&mut self, edit: &BarEdit) -> Result<(), SettingError> {
        let mut next = self.clone();
        match edit {
            BarEdit::Move {
                id,
                zone,
                index,
                to_zone,
                to_index,
            } => {
                next.check_at(id, zone, *index)?;
                if next.zone(to_zone).is_none() {
                    return Err(invalid("shell.bar", format!("unknown zone {to_zone:?}")));
                }
                let item = next.zone_mut(zone).expect("checked").remove(*index);
                let to = next.zone_mut(to_zone).expect("checked");
                let at = (*to_index).min(to.len());
                to.insert(at, item);
            }
            BarEdit::Add { id, zone, index } => {
                let z = next
                    .zone_mut(zone)
                    .ok_or_else(|| invalid("shell.bar", format!("unknown zone {zone:?}")))?;
                let at = index.unwrap_or(z.len()).min(z.len());
                z.insert(at, BarItem::new(id));
            }
            BarEdit::Remove { id, zone, index } => {
                next.check_at(id, zone, *index)?;
                next.zone_mut(zone).expect("checked").remove(*index);
            }
            BarEdit::Size {
                id,
                zone,
                index,
                size,
                units,
            } => {
                next.check_at(id, zone, *index)?;
                let it = &mut next.zone_mut(zone).expect("checked")[*index];
                it.size = (*size != default_size(id)).then_some(*size);
                it.units = *units;
            }
            BarEdit::Options { island, value } => {
                let mut opts = serde_json::to_value(&next.options).expect("options serialise");
                if !BarOptions::FIELDS.contains(&island.as_str()) {
                    return Err(invalid(
                        "shell.bar.options",
                        format!("{island:?} has no options"),
                    ));
                }
                opts[island.as_str()] = value.clone();
                next.options = serde_json::from_value(opts)
                    .map_err(|e| invalid("shell.bar.options", e.to_string()))?;
            }
            BarEdit::Reset => {
                let options = std::mem::take(&mut next.options);
                next = BarLayout {
                    options,
                    ..BarLayout::default()
                };
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}
