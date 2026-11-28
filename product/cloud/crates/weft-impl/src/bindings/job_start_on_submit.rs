//! `job-start-on-submit` (`ess/components.yaml`, block "R1 for a Job workload", the owner's
//! decision D-C2 = A): `weft.workloads.JobSubmitted` where `gang == false` ->
//! `weft.workloads.StartJob`, mapping `job_id` and `bound_tasks` (the event's `parallelism`).
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `wrong-state` (a repeated delivery: the Job already `Running`); retries every
//! other refusal. A gang Job waits for its whole gang to be bound, which no loop does yet.
//!
//! A repeated delivery of the `ReleaseStepStarted` the Job came from submits a second Job
//! (`r1-rollout-submit-job`), which this binding starts too. The held form of `JobSubmitted` is
//! written here ([`encode`], [`decode`]) from the codecs of its fields; it names no scope, so
//! the invocation acts as no caller (`StartJob` reads none).

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::shared::{JobId, WorkloadId};
use weft_types::workloads::{JobSubmitted, StartJob, StartJobOutcome};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "job-start-on-submit",
    drop: &["wrong-state"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsJobSubmitted(submitted) => Some(json!({
            "job_id": submitted.job_id.encode(),
            "workload_id": submitted.workload_id.encode(),
            "parallelism": submitted.parallelism.encode(),
            "gang": submitted.gang.encode(),
        })),
        _ => None,
    }
}

fn field<T: Codec>(value: &Value, name: &str) -> Result<T, String> {
    T::decode(member(value, name)?).map_err(|error| at(name, error))
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    Ok(SystemEvent::WorkloadsJobSubmitted(JobSubmitted {
        job_id: field::<JobId>(value, "job_id")?,
        workload_id: field::<WorkloadId>(value, "workload_id")?,
        parallelism: field::<i64>(value, "parallelism")?,
        gang: field::<bool>(value, "gang")?,
    }))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsJobSubmitted(submitted) = event else {
        return None;
    };
    if submitted.gang {
        return None;
    }
    let input = StartJob {
        job_id: submitted.job_id.clone(),
        bound_tasks: submitted.parallelism,
    };
    Some(
        match invoke(system, |system| system.weft_control.start_job(input)) {
            Ok((StartJobOutcome::Started { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
