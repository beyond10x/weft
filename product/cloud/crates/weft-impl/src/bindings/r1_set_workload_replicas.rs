//! `r1-set-workload-replicas` (`ess/components.yaml`): `weft.deployments.ReleaseStepStarted` where
//! `kind == Rollout` and `workload_id` and `replicas` are defined ->
//! `weft.workloads.SetWorkloadReplicas`, mapping `workload_id`, `scope_id` and `replicas`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `workload-retired` and `not-a-service`; retries every other refusal.
//!
//! The invocation acts in the event's scope (the runner's caller of a hand-run invocation,
//! [`super`]), which `SetWorkloadReplicas` compares with the mapped `scope_id`. The held form of
//! `ReleaseStepStarted` is written here ([`encode`], [`decode`]) from the codecs of its fields.

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::deployments::{ReleaseStepStarted, StepKind};
use weft_types::shared::{ComponentName, JobId, ReleaseId, ReleaseStepId, ScopeId, WorkloadId};
use weft_types::workloads::{SetWorkloadReplicas, SetWorkloadReplicasOutcome};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r1-set-workload-replicas",
    drop: &["workload-retired", "not-a-service"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::DeploymentsReleaseStepStarted(started) => Some(json!({
            "step_id": started.step_id.encode(),
            "release_id": started.release_id.encode(),
            "scope_id": started.scope_id.encode(),
            "position": started.position.encode(),
            "kind": started.kind.encode(),
            "component": started.component.encode(),
            "job_id": started.job_id.encode(),
            "workload_id": started.workload_id.encode(),
            "replicas": started.replicas.encode(),
        })),
        _ => None,
    }
}

fn field<T: Codec>(value: &Value, name: &str) -> Result<T, String> {
    T::decode(member(value, name)?).map_err(|error| at(name, error))
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    Ok(SystemEvent::DeploymentsReleaseStepStarted(
        ReleaseStepStarted {
            step_id: field::<ReleaseStepId>(value, "step_id")?,
            release_id: field::<ReleaseId>(value, "release_id")?,
            scope_id: field::<ScopeId>(value, "scope_id")?,
            position: field::<i64>(value, "position")?,
            kind: field::<StepKind>(value, "kind")?,
            component: field::<Option<ComponentName>>(value, "component")?,
            job_id: field::<Option<JobId>>(value, "job_id")?,
            workload_id: field::<Option<WorkloadId>>(value, "workload_id")?,
            replicas: field::<Option<i64>>(value, "replicas")?,
        },
    ))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::DeploymentsReleaseStepStarted(started) = event else {
        return None;
    };
    if started.kind != StepKind::Rollout {
        return None;
    }
    let (Some(workload_id), Some(replicas)) = (&started.workload_id, started.replicas) else {
        return None;
    };
    let input = SetWorkloadReplicas {
        workload_id: workload_id.clone(),
        scope_id: started.scope_id.clone(),
        replicas,
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.set_workload_replicas(input)
        }) {
            Ok((SetWorkloadReplicasOutcome::Set { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
