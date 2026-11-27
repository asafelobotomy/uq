//! Restore last session: last session's apps become jobs of this login's run, each moved to
//! its workspace once its first window shows.

use uq_core::autostart::scan::{Plan, Resolved};

use super::Job;

/// The jobs for `restore`'s apps; an app that can't run here is left out.
pub(super) fn jobs(restore: Vec<(Resolved, i64)>) -> Vec<Job> {
    restore
        .into_iter()
        .filter_map(|(e, ws)| match e.plan {
            Plan::Run {
                argv,
                cwd: Some(cwd),
                ..
            } => Some(Job {
                id: e.id,
                argv,
                cwd,
                delay: 0,
                minimize: false,
                retry: false,
                when: None,
                workspace: Some(ws),
                at_login: true,
            }),
            _ => None,
        })
        .collect()
}
