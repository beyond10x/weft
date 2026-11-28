//! `edge-backend-restarting` (`ess/components.yaml`): `weft.workloads.InstanceHealthStarting` ->
//! `weft.edge.RecordBackendNotReady`, mapping `instance_id`, invoked through
//! `WeftEdge::record_backend_not_ready`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `not-ready` (the instance was never ready, so it has no backend yet, as on its
//! first start; or it is out of traffic already, or this is a repeated delivery); retries every
//! other refusal.
//!
//! The held form of `InstanceHealthStarting` is written here ([`encode_health_starting`],
//! [`decode_health_starting`]) from the codecs of its two identities.

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::edge::{RecordBackendNotReady, RecordBackendNotReadyOutcome};
use weft_types::shared::{InstanceId, WorkloadId};
use weft_types::workloads::InstanceHealthStarting;

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "edge-backend-restarting",
    drop: &["not-ready"],
    invoke: deliver,
    encode: encode_health_starting,
    decode: decode_health_starting,
};

/// `InstanceHealthStarting` in the form of the state file; `None` for another event.
pub fn encode_health_starting(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsInstanceHealthStarting(starting) => Some(json!({
            "instance_id": starting.instance_id.encode(),
            "workload_id": starting.workload_id.encode(),
        })),
        _ => None,
    }
}

/// The `InstanceHealthStarting` written by [`encode_health_starting`].
///
/// # Errors
///
/// Where `value` is not such an event: the place and what was found.
pub fn decode_health_starting(value: &Value) -> Result<SystemEvent, String> {
    let instance_id = InstanceId::decode(member(value, "instance_id")?)
        .map_err(|error| at("instance_id", error))?;
    let workload_id = WorkloadId::decode(member(value, "workload_id")?)
        .map_err(|error| at("workload_id", error))?;
    Ok(SystemEvent::WorkloadsInstanceHealthStarting(
        InstanceHealthStarting {
            instance_id,
            workload_id,
        },
    ))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceHealthStarting(starting) = event else {
        return None;
    };
    let input = RecordBackendNotReady {
        instance_id: starting.instance_id.clone(),
    };
    Some(
        match invoke(system, |system| {
            system.weft_edge.record_backend_not_ready(input)
        }) {
            Ok((RecordBackendNotReadyOutcome::TakenOut { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
