//! `r1-create-replica` (`ess/components.yaml`): `weft.workloads.ReplicaRequested` ->
//! `weft.workloads.CreateInstance`, mapping `workload_id`, `service_id` and `replica_index`.
//!
//! Removed from the synthesis input for ESS
//! `story:rust-binding-wraps-a-plain-copy-into-an-optional-member` (E3): the event's plain
//! `service_id` and `replica_index` are copied into the command's Optional members here, by hand.
//! Drops `replica-live` (a repeated delivery) and `workload-retired`; retries every other refusal.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::workloads::{CreateInstance, CreateInstanceOutcome, ReplicaRequested};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r1-create-replica",
    drop: &["replica-live", "workload-retired"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsReplicaRequested(requested) => Some(requested.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    ReplicaRequested::decode(value).map(SystemEvent::WorkloadsReplicaRequested)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsReplicaRequested(requested) = event else {
        return None;
    };
    let input = CreateInstance {
        workload_id: requested.workload_id.clone(),
        job_id: None,
        task_index: None,
        invocation_id: None,
        box_id: None,
        session_id: None,
        replica_index: Some(requested.replica_index),
        warm_pool_id: None,
        service_id: Some(requested.service_id.clone()),
        scope_node_id: None,
    };
    Some(
        match invoke(system, |system| system.weft_control.create_instance(input)) {
            Ok((CreateInstanceOutcome::Created { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
