//! The light/dark schedule: set it, and switch variant when a boundary passes.

use super::*;

impl Desktop {
    pub async fn set_schedule(&mut self, s: Schedule, origin: Origin) -> Result<(), DesktopError> {
        self.set_schedule_at(s, Local::now(), origin).await
    }

    /// Store a schedule and switch to the variant it wants at `now`.
    pub async fn set_schedule_at<Tz: TimeZone>(
        &mut self,
        s: Schedule,
        now: DateTime<Tz>,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        s.validate().map_err(DesktopError::InvalidSchedule)?;
        let mut next = self.state.clone();
        // While the active profile sets the variant, the schedule waits: no mark, so the first
        // tick after Back to normal applies it.
        let held = self.profile_holds_variant();
        let wanted = variant_at(&s, &now).filter(|_| !held);
        next.schedule_mark = last_event(&s, &now)
            .filter(|_| !held)
            .map(|(at, _)| at.with_timezone(&Utc));
        next.schedule = s;
        let change = match wanted {
            Some(v) if v != next.variant => {
                next.variant = v;
                Change::Theme
            }
            _ => Change::Quiet,
        };
        self.commit(next, change, origin).await
    }

    /// Apply a scheduled transition that is newer than the last one applied. Returns whether the
    /// variant switched. A manual variant change therefore holds until the next boundary.
    /// Called at `next_schedule_event` (a wall-clock timer, so suspend/resume and clock changes
    /// are caught at once) and after any desktop event.
    /// While the active profile sets the variant nothing happens and the mark stays, so the first
    /// tick after Back to normal switches as usual.
    pub async fn tick_schedule_at<Tz: TimeZone>(
        &mut self,
        now: DateTime<Tz>,
    ) -> Result<bool, DesktopError> {
        if self.profile_holds_variant() {
            return Ok(false);
        }
        let Some((at, v)) = last_event(&self.state.schedule, &now) else {
            return Ok(false);
        };
        let at = at.with_timezone(&Utc);
        if self.state.schedule_mark.is_some_and(|mark| mark >= at) {
            return Ok(false);
        }
        let mut next = self.state.clone();
        next.schedule_mark = Some(at);
        if v == next.variant {
            self.commit(next, Change::Quiet, Origin::Schedule)
                .await
                .map(|()| false)
        } else {
            next.variant = v;
            self.commit(next, Change::Theme, Origin::Schedule)
                .await
                .map(|()| true)
        }
    }

    /// When `tick_schedule_at` next has something to do: the next scheduled transition after
    /// `now`. `None` while the schedule is off or the active profile holds the variant (ending
    /// the profile is an event, after which this is asked again).
    pub fn next_schedule_event<Tz: TimeZone>(&self, now: DateTime<Tz>) -> Option<DateTime<Utc>> {
        if self.profile_holds_variant() {
            return None;
        }
        next_event(&self.state.schedule, &now).map(|at| at.with_timezone(&Utc))
    }
}
