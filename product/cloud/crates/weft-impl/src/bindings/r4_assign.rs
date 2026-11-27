//! `r4-assign` (`ess/components.yaml`): `weft.scheduling.PlacementBound` ->
//! `weft.nodes.Assign`, mapping `instance_id`, `node_id`, `decision_id` (as
//! `placement_decision_id`) and `decided_at` (as `assigned_at`: the lease starts at the instant
//! the Scheduler stamped).
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `already-assigned` (a repeated delivery), `placement-not-bound` (released before
//! the assignment) and `no-open-session` (the repair step assigns it once the node redials);
//! retries every other refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::nodes::{Assign, AssignOutcome};
use weft_types::scheduling::PlacementBound;

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r4-assign",
    drop: &["already-assigned", "placement-not-bound", "no-open-session"],
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
    let input = Assign {
        instance_id: bound.instance_id.clone(),
        node_id: bound.node_id.clone(),
        placement_decision_id: bound.decision_id.clone(),
        assigned_at: bound.decided_at.clone(),
    };
    Some(
        match invoke(system, |system| system.weft_control.assign(input)) {
            Ok((AssignOutcome::Assigned { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
