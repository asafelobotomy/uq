//! Welcome's steps (Phase 5 spec §5.1): which ones show, and moving between them.
//!
//! Hello and Done always show. Displays shows only with more than one screen, Fingerprint only
//! with a reader and no finger enrolled yet. After a release that raises
//! [`CURRENT_WELCOME_VERSION`], a user who finished an older Welcome sees only the steps added
//! since (each step's [`Step::since`]).

use serde::Serialize;
use uq_core::welcome::CURRENT_WELCOME_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Hello,
    Look,
    Wallpaper,
    Layouts,
    Displays,
    Apps,
    Fingerprint,
    Done,
}

impl Step {
    /// Every step, in the order they show.
    pub const ALL: [Step; 8] = [
        Step::Hello,
        Step::Look,
        Step::Wallpaper,
        Step::Layouts,
        Step::Displays,
        Step::Apps,
        Step::Fingerprint,
        Step::Done,
    ];

    /// The Welcome version that added the step.
    pub fn since(self) -> u32 {
        1
    }

    /// The step's title, as the window shows it.
    pub fn title(self) -> &'static str {
        match self {
            Step::Hello => "Welcome to uQ",
            Step::Look => "Choose a look",
            Step::Wallpaper => "Pick a wallpaper",
            Step::Layouts => "How windows are arranged",
            Step::Displays => "Arrange your screens",
            Step::Apps => "Apps made for uQ",
            Step::Fingerprint => "Unlock with your fingerprint",
            Step::Done => "All set",
        }
    }
}

/// What decides which steps show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Context {
    /// Connected screens (at least 1 is assumed when unknown).
    pub monitors: usize,
    /// A fingerprint reader is there and no finger is enrolled yet.
    pub fingerprint: bool,
    /// The Welcome version the user already went through (0: none, every step shows).
    pub seen_version: u32,
}

/// The steps to show for `ctx`, in order.
pub fn steps(ctx: &Context) -> Vec<Step> {
    Step::ALL
        .into_iter()
        .filter(|s| match s {
            Step::Hello | Step::Done => true,
            Step::Displays => ctx.monitors > 1 && s.since() > ctx.seen_version,
            Step::Fingerprint => ctx.fingerprint && s.since() > ctx.seen_version,
            _ => s.since() > ctx.seen_version,
        })
        .collect()
}

/// Where the user is in Welcome. Every change is applied the moment it is made, so Skip only
/// moves on (and remembers the step was skipped); nothing waits for Next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    steps: Vec<Step>,
    index: usize,
    skipped: Vec<Step>,
}

impl Flow {
    pub fn new(ctx: &Context) -> Flow {
        Flow {
            steps: steps(ctx),
            index: 0,
            skipped: Vec::new(),
        }
    }

    pub fn current(&self) -> Step {
        self.steps[self.index]
    }

    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    pub fn is_last(&self) -> bool {
        self.index + 1 == self.steps.len()
    }

    #[cfg(test)]
    pub fn skipped(&self) -> &[Step] {
        &self.skipped
    }

    /// The next step; the last one (Done) stays.
    pub fn next(&mut self) {
        if !self.is_last() {
            self.index += 1;
        }
    }

    /// The step before; the first stays.
    pub fn back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    /// Leave this step as it is and go on. Done can't be skipped.
    pub fn skip(&mut self) {
        if self.is_last() {
            return;
        }
        let s = self.current();
        if !self.skipped.contains(&s) {
            self.skipped.push(s);
        }
        self.next();
    }

    /// `{steps: [{id, title}], index, current, first, last, skipped, current_version}`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "steps": self.steps.iter().map(|s| serde_json::json!({"id": s, "title": s.title()}))
                .collect::<Vec<_>>(),
            "index": self.index,
            "count": self.steps.len(),
            "current": self.current(),
            "first": self.is_first(),
            "last": self.is_last(),
            "skipped": self.skipped,
            "version": CURRENT_WELCOME_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Step::*;

    fn ctx(monitors: usize, fingerprint: bool) -> Context {
        Context {
            monitors,
            fingerprint,
            seen_version: 0,
        }
    }

    #[test]
    fn one_screen_no_reader() {
        assert_eq!(
            steps(&ctx(1, false)),
            [Hello, Look, Wallpaper, Layouts, Apps, Done]
        );
        // Unknown screens count as one.
        assert_eq!(steps(&ctx(0, false)), steps(&ctx(1, false)));
    }

    #[test]
    fn several_screens_show_displays() {
        assert_eq!(
            steps(&ctx(2, false)),
            [Hello, Look, Wallpaper, Layouts, Displays, Apps, Done]
        );
        assert_eq!(steps(&ctx(3, false)).len(), 7);
    }

    #[test]
    fn a_reader_without_a_finger_shows_fingerprint() {
        assert_eq!(
            steps(&ctx(1, true)),
            [Hello, Look, Wallpaper, Layouts, Apps, Fingerprint, Done]
        );
        assert_eq!(steps(&ctx(2, true)), Step::ALL);
    }

    #[test]
    fn a_finished_welcome_shows_only_newer_steps() {
        let seen = Context {
            monitors: 2,
            fingerprint: true,
            seen_version: CURRENT_WELCOME_VERSION,
        };
        assert_eq!(steps(&seen), [Hello, Done], "nothing new since");
        // Every step so far came with version 1.
        assert!(
            Step::ALL
                .iter()
                .all(|s| s.since() <= CURRENT_WELCOME_VERSION)
        );
    }

    #[test]
    fn next_back_and_bounds() {
        let mut f = Flow::new(&ctx(1, false));
        assert!(f.is_first() && !f.is_last());
        assert_eq!(f.current(), Hello);
        f.back();
        assert_eq!(f.current(), Hello, "back at the start stays");
        f.next();
        assert_eq!(f.current(), Look);
        f.back();
        assert_eq!(f.current(), Hello);
        for _ in 0..20 {
            f.next();
        }
        assert_eq!(f.current(), Done);
        assert!(f.is_last());
        f.next();
        assert_eq!(f.current(), Done, "next at the end stays");
        f.back();
        assert_eq!(f.current(), Apps);
    }

    #[test]
    fn every_step_can_be_skipped_but_done() {
        let mut f = Flow::new(&ctx(2, true));
        while !f.is_last() {
            f.skip();
        }
        assert_eq!(f.current(), Done);
        assert_eq!(
            f.skipped(),
            [Hello, Look, Wallpaper, Layouts, Displays, Apps, Fingerprint]
        );
        f.skip();
        assert_eq!(f.current(), Done);
        assert!(!f.skipped().contains(&Done));
        // Coming back and skipping again doesn't list a step twice.
        f.back();
        f.skip();
        assert_eq!(f.skipped().len(), 7);
    }

    #[test]
    fn json_names_the_steps() {
        let mut f = Flow::new(&ctx(1, false));
        f.skip();
        let v = f.to_json();
        assert_eq!(v["current"], "look");
        assert_eq!(v["index"], 1);
        assert_eq!(v["count"], 6);
        assert_eq!(v["first"], false);
        assert_eq!(v["skipped"], serde_json::json!(["hello"]));
        assert_eq!(
            v["steps"][0],
            serde_json::json!({"id": "hello", "title": "Welcome to uQ"})
        );
    }
}
