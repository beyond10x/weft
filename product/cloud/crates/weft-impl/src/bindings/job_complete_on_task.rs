//! `job-complete-on-task` (`ess/components.yaml`, block "R1 for a Job workload", the owner's
//! decision D-C2 = A: finished = the Job is `Succeeded`): `weft.workloads.TaskSucceeded` ->
//! `weft.workloads.CompleteJob`, mapping `job_id` and no output (a snippet writes none). The
//! declared mapping takes `outputs` through the conversion `JobId -> List<ArtifactDigest>` of
//! `ess/components.yaml`, which stands for the Job's outputs: a binding maps no list literal.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `outstanding` (another task of the Job has not succeeded yet: the last task's
//! `TaskSucceeded` completes it) and `wrong-state` (a repeated delivery, the Job already
//! `Succeeded`, or a Job that ended otherwise first); retries every other refusal.
//!
//! The held form of `TaskSucceeded` is written here ([`encode`], [`decode`]) from the codecs of
//! its fields; it names no scope, so the invocation acts as no caller (`CompleteJob` reads none).

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::shared::JobId;
use weft_types::workloads::{CompleteJob, CompleteJobOutcome, TaskSucceeded};

use super::{invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "job-complete-on-task",
    drop: &["outstanding", "wrong-state"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsTaskSucceeded(succeeded) => Some(json!({
            "job_id": succeeded.job_id.encode(),
            "task_index": succeeded.task_index.encode(),
        })),
        _ => None,
    }
}

fn field<T: Codec>(value: &Value, name: &str) -> Result<T, String> {
    T::decode(member(value, name)?).map_err(|error| at(name, error))
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    Ok(SystemEvent::WorkloadsTaskSucceeded(TaskSucceeded {
        job_id: field::<JobId>(value, "job_id")?,
        task_index: field::<i64>(value, "task_index")?,
    }))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsTaskSucceeded(succeeded) = event else {
        return None;
    };
    let input = CompleteJob {
        job_id: succeeded.job_id.clone(),
        outputs: Vec::new(),
    };
    Some(
        match invoke(system, |system| system.weft_control.complete_job(input)) {
            Ok((CompleteJobOutcome::Succeeded { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
