//! `r1-submit-scheduling-request` (`ess/components.yaml`): `weft.workloads.InstanceCreated`
//! where `job_id` is not defined -> `weft.scheduling.SubmitSchedulingRequest`, mapping
//! `instance_id`, `account_id`, `formation_id`, `claims`, `minimum_isolation` (as
//! `min_isolation`), `zones` and `priority`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `already-requested` (a repeated delivery); retries every other refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::scheduling::{SubmitSchedulingRequest, SubmitSchedulingRequestOutcome};
use weft_types::workloads::InstanceCreated;

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r1-submit-scheduling-request",
    drop: &["already-requested"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsInstanceCreated(created) => Some(created.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    InstanceCreated::decode(value).map(SystemEvent::WorkloadsInstanceCreated)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceCreated(created) = event else {
        return None;
    };
    if created.job_id.is_some() {
        return None;
    }
    let input = SubmitSchedulingRequest {
        instance_id: created.instance_id.clone(),
        account_id: created.account_id.clone(),
        formation_id: created.formation_id.clone(),
        claims: created.claims.clone(),
        min_isolation: created.minimum_isolation,
        zones: created.zones.clone(),
        priority: created.priority.clone(),
        gang_id: None,
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.submit_scheduling_request(input)
        }) {
            Ok((SubmitSchedulingRequestOutcome::Requested { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
