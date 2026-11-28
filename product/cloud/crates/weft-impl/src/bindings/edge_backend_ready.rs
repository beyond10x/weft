//! `edge-backend-ready` (`ess/components.yaml`): `weft.workloads.InstanceReady` ->
//! `weft.edge.RecordBackendReady`, mapping `instance_id` and `workload_id`, invoked through
//! `WeftEdge::record_backend_ready`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `known` (the instance is a backend already: `edge-backend-restore` moves it back
//! into traffic, or this is a repeated delivery); retries every other refusal.
//!
//! The held form of `InstanceReady` is written here ([`encode_ready`], [`decode_ready`]) from the
//! codecs of its two identities, for this entry and `edge-backend-restore`.

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::edge::{RecordBackendReady, RecordBackendReadyOutcome};
use weft_types::shared::{InstanceId, WorkloadId};
use weft_types::workloads::InstanceReady;

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "edge-backend-ready",
    drop: &["known"],
    invoke: deliver,
    encode: encode_ready,
    decode: decode_ready,
};

/// `InstanceReady` in the form of the state file; `None` for another event.
pub fn encode_ready(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsInstanceReady(ready) => Some(json!({
            "instance_id": ready.instance_id.encode(),
            "workload_id": ready.workload_id.encode(),
        })),
        _ => None,
    }
}

/// The `InstanceReady` written by [`encode_ready`].
///
/// # Errors
///
/// Where `value` is not such an event: the place and what was found.
pub fn decode_ready(value: &Value) -> Result<SystemEvent, String> {
    let instance_id = InstanceId::decode(member(value, "instance_id")?)
        .map_err(|error| at("instance_id", error))?;
    let workload_id = WorkloadId::decode(member(value, "workload_id")?)
        .map_err(|error| at("workload_id", error))?;
    Ok(SystemEvent::WorkloadsInstanceReady(InstanceReady {
        instance_id,
        workload_id,
    }))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceReady(ready) = event else {
        return None;
    };
    let input = RecordBackendReady {
        instance_id: ready.instance_id.clone(),
        workload_id: ready.workload_id.clone(),
    };
    Some(
        match invoke(system, |system| {
            system.weft_edge.record_backend_ready(input)
        }) {
            Ok((RecordBackendReadyOutcome::Added { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
