//! `r4-bind-placement` (`ess/components.yaml`): `weft.scheduling.InstancePlaced` ->
//! `weft.scheduling.BindPlacement`, mapping `decision_id` and `request_id`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `gang-member` (BindGang binds a gang's members) and `wrong-state` (a repeated
//! delivery, or the decision was released first); retries every other refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::scheduling::{BindPlacement, BindPlacementOutcome, InstancePlaced};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r4-bind-placement",
    drop: &["gang-member", "wrong-state"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::SchedulingInstancePlaced(placed) => Some(placed.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    InstancePlaced::decode(value).map(SystemEvent::SchedulingInstancePlaced)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::SchedulingInstancePlaced(placed) = event else {
        return None;
    };
    let input = BindPlacement {
        decision_id: placed.decision_id.clone(),
        request_id: placed.request_id.clone(),
    };
    Some(
        match invoke(system, |system| system.weft_control.bind_placement(input)) {
            Ok((BindPlacementOutcome::Bound { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
