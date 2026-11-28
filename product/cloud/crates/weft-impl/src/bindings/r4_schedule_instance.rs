//! `r4-schedule-instance` (`ess/components.yaml`): `weft.scheduling.PlacementBound` ->
//! `weft.workloads.ScheduleInstance`, mapping `instance_id`, `node_id` and `isolation`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `wrong-state` (a repeated delivery, or the instance was stopped while pending);
//! retries every other refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::scheduling::PlacementBound;
use weft_types::workloads::{ScheduleInstance, ScheduleInstanceOutcome};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r4-schedule-instance",
    drop: &["wrong-state"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::SchedulingPlacementBound(bound) => Some(bound.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    PlacementBound::decode(value).map(SystemEvent::SchedulingPlacementBound)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::SchedulingPlacementBound(bound) = event else {
        return None;
    };
    let input = ScheduleInstance {
        instance_id: bound.instance_id.clone(),
        node_id: bound.node_id.clone(),
        isolation: bound.isolation,
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.schedule_instance(input)
        }) {
            Ok((ScheduleInstanceOutcome::Scheduled { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
