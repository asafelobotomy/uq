//! Settings history: who changed what (`Origin`), the per-domain diff and the on-disk journal.

mod diff;
mod journal;
mod labels;
mod restore;

pub use diff::{DomainChange, Labels, diff, revert_changed, summarize};
pub(crate) use diff::{get_path, put_path, segment_leaves, summarize_leaves};
pub use journal::{Entry, Journal, MAX_AGE_DAYS, MAX_ENTRIES};
pub use labels::label;
pub use restore::{AutoReason, PointKind, RestorePoint, RestoreStore};

/// Where a change came from. `Schedule` and `Hotplug` transitions are never journalled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Settings,
    Uqctl,
    Shell,
    Profile(String),
    Restore,
    Import,
    Schedule,
    /// uqd's own switch to a saved display arrangement; never journalled, never parsed.
    Hotplug,
    Unknown,
}

impl Origin {
    /// Empty or unrecognised text is `Unknown`; `profile:<id>` is `Profile(id)`. `schedule` is
    /// `Unknown` too: only uqd's own timer makes `Schedule` changes, which skip the journal.
    pub fn parse(s: &str) -> Origin {
        match s {
            "settings" => Origin::Settings,
            "uqctl" => Origin::Uqctl,
            "shell" => Origin::Shell,
            "restore" => Origin::Restore,
            "import" => Origin::Import,
            _ => match s.strip_prefix("profile:") {
                Some(id) if !id.is_empty() => Origin::Profile(id.to_string()),
                _ => Origin::Unknown,
            },
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Origin::Settings => "settings".into(),
            Origin::Uqctl => "uqctl".into(),
            Origin::Shell => "shell".into(),
            Origin::Profile(id) => format!("profile:{id}"),
            Origin::Restore => "restore".into(),
            Origin::Import => "import".into(),
            Origin::Schedule => "schedule".into(),
            Origin::Hotplug => "hotplug".into(),
            Origin::Unknown => "unknown".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_parse_round_trip() {
        for o in [
            Origin::Settings,
            Origin::Uqctl,
            Origin::Shell,
            Origin::Profile("gaming".into()),
            Origin::Restore,
            Origin::Import,
            Origin::Unknown,
        ] {
            assert_eq!(Origin::parse(&o.as_str()), o);
        }
        // Schedule flips are uqd's own; a caller can't claim one to skip the journal.
        assert_eq!(Origin::parse("schedule"), Origin::Unknown);
        assert_eq!(Origin::parse("hotplug"), Origin::Unknown);
        assert_eq!(Origin::Hotplug.as_str(), "hotplug");
        assert_eq!(Origin::parse("profile:gaming").as_str(), "profile:gaming");
        assert_eq!(Origin::parse(""), Origin::Unknown);
        assert_eq!(Origin::parse("bogus"), Origin::Unknown);
        assert_eq!(Origin::parse("profile:"), Origin::Unknown);
    }
}
