use super::extras::{NEEDS_NAME, NOT_THERE};
use super::*;
use crate::pages::fake_uqd::{self, Rig};
use crate::pages::runner::run;
use uq_core::state::State;

async fn rig() -> (Rig, DisplaysState) {
    let r = fake_uqd::rig(State::default()).await;
    let page = DisplaysState::new(r.client.clone());
    (r, page)
}

fn stored(r: &Rig) -> Monitors {
    r.state.lock().unwrap().settings.monitors.clone()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn ok(out: &crate::pages::runner::Outcome) {
    assert_eq!(out.error, None, "{out:?}");
}

fn bar(show: &str, outputs: &[&str]) -> Op {
    Op::SetBar {
        show: show.into(),
        outputs: outputs.iter().map(|s| s.to_string()).collect(),
    }
}

fn plan_op(strategy: &str, count: u8) -> Op {
    Op::SetPlan {
        strategy: strategy.into(),
        count,
        move_back: true,
    }
}

fn remember(name: &str) -> Op {
    Op::Remember { name: name.into() }
}

/// A scale change on DP-1, applied and kept: the layout the saved arrangement no longer has.
async fn keep_scale(page: &DisplaysState) {
    let edit = Op::Edit {
        output: "DP-1".into(),
        field: "scale".into(),
        value: json!(1.5),
    };
    ok(&run(page, edit).await);
    ok(&run(page, Op::Apply).await);
    ok(&run(page, Op::Keep).await);
}

mod main_bar;
mod remember;
