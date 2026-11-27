#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::compat::launch::main_scale;
use crate::settings::{
    BarChoice, BarShow, ManualSlot, MonitorId, MonitorProfile, Monitors, OutputConfig, Position,
    SavedOutput, Strategy, WorkspacePlan,
};
use std::collections::BTreeSet;

fn out(name: &str, serial: &str, x: i32, y: i32) -> LiveOutput {
    LiveOutput {
        name: name.into(),
        description: format!("Dell Inc. DELL U2720Q {serial}"),
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: serial.into(),
        width: 2560,
        height: 1440,
        refresh_rate: 143.912,
        x,
        y,
        scale: 1.0,
        ..LiveOutput::default()
    }
}

fn mid(serial: &str) -> MonitorId {
    MonitorId {
        make: "Dell Inc.".into(),
        model: "DELL U2720Q".into(),
        serial: serial.into(),
    }
}

fn saved(serial: &str, hint: &str, x: i32) -> SavedOutput {
    SavedOutput {
        monitor: mid(serial),
        hint: hint.into(),
        config: OutputConfig {
            position: Some(Position { x, y: 0 }),
            scale: Some(1.5),
            ..OutputConfig::default()
        },
    }
}

fn profile(id: &str, name: &str, outputs: Vec<SavedOutput>) -> MonitorProfile {
    MonitorProfile {
        id: id.into(),
        name: name.into(),
        outputs,
        workspaces: None,
    }
}

fn with_profiles(p: Vec<MonitorProfile>) -> Monitors {
    Monitors {
        profiles: p,
        ..Monitors::default()
    }
}

fn names(r: &[(String, OutputConfig)]) -> Vec<&str> {
    r.iter().map(|(n, _)| n.as_str()).collect()
}

fn screens_n(n: usize) -> Vec<LiveOutput> {
    (0..n)
        .map(|i| out(&format!("DP-{}", i + 1), &i.to_string(), i as i32 * 2560, 0))
        .collect()
}

fn plan_of(strategy: Strategy, count: u8, n: usize) -> Vec<(u8, String, bool)> {
    let p = WorkspacePlan {
        strategy,
        count,
        ..WorkspacePlan::default()
    };
    plan(&p, &screens_n(n))
        .into_iter()
        .map(|p| (p.workspace, p.output, p.default))
        .collect()
}

fn share(planned: &[(u8, String, bool)], output: &str) -> Vec<u8> {
    planned
        .iter()
        .filter(|p| p.1 == output)
        .map(|p| p.0)
        .collect()
}

mod matching;
mod split_roles;
