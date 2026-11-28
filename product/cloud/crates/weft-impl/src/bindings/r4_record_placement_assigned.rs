//! `r4-record-placement-assigned` (`ess/components.yaml`): `weft.nodes.InstanceAssigned` ->
//! `weft.scheduling.RecordPlacementAssigned`, mapping `placement_decision_id` (as
//! `decision_id`).
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `not-bound` (the placement was released after the assignment); retries every other
//! refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::nodes::InstanceAssigned;
use weft_types::scheduling::{RecordPlacementAssigned, RecordPlacementAssignedOutcome};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r4-record-placement-assigned",
    drop: &["not-bound"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::NodesInstanceAssigned(assigned) => Some(assigned.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    InstanceAssigned::decode(value).map(SystemEvent::NodesInstanceAssigned)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::NodesInstanceAssigned(assigned) = event else {
        return None;
    };
    let input = RecordPlacementAssigned {
        decision_id: assigned.placement_decision_id.clone(),
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.record_placement_assigned(input)
        }) {
            Ok((RecordPlacementAssignedOutcome::Recorded { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
