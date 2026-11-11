//! The fake uqd's `Explain("compat")`, built as uqd builds it: the generic option row for
//! `sharp_x11`, reported as `default` while it is off (F6), then uq-core's rows for the other
//! switches and each app fix.

use serde_json::Value;
use uq_core::startup_explain::{FileReport, default_when_off, explain_compat_extra};
use uq_core::state::State;

/// Add the compat rows to `generic` (the generic option rows) in place. There is no `user.lua`
/// in the fake.
pub fn explain(generic: &mut Value, state: &State, files: &FileReport, rejected: Option<&str>) {
    default_when_off(generic, state);
    let extra = explain_compat_extra(state, None, rejected, files, "user.lua");
    if let (Value::Object(rows), Value::Object(extra)) = (generic, extra) {
        rows.extend(extra);
    }
}
