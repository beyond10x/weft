//! `job-task-instances` (`ess/components.yaml`, block "R1 for a Job workload", the owner's
//! decision D-C2 = A: Instance -> Job, one instance per task): `weft.workloads.JobStarted` ->
//! `weft.workloads.CreateInstance` for each task index below `bound_tasks`, mapping the Job's
//! workload (read from the Job's row), `job_id` and the task index.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2), and because ESS maps a binding only from its event and invokes one command per event
//! (the OPEN (ESS) of the block). Drops nothing; retries every refusal.
//!
//! The Job and the instances are read from the ports of the attempt ([`super::attempt_ports`]).
//! A task index that already has a live instance of the Job (`Pending`, `Scheduled`,
//! `Starting`, `Running`, `Paused`: [`crate::quota::live`]) is skipped, so a repeated delivery
//! of `JobStarted` makes no second instance of a task; when every task has one, the attempt
//! invokes nothing and is applied with no event. A repeated delivery of the `ReleaseStepStarted`
//! the Job came from submits a second Job (`r1-rollout-submit-job`), whose tasks get instances
//! of their own.
//!
//! The attempt is one answer: the events of every instance created, or the first refusal or
//! unmet obligation. The invocations differ only in the task index, which no refusal of
//! `CreateInstance` reads, so a refusal comes at the first invocation. OPEN: an unmet
//! obligation after the first task leaves the earlier tasks' instances standing; the next
//! attempt skips them, and their `InstanceCreated` reaches no binding.
//!
//! The held form of `JobStarted` is written here ([`encode`], [`decode`]) from the codecs of its
//! fields; it names no scope, so the invocations act as no caller (`CreateInstance` reads none).

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::behaviour::{InstanceStorage, JobStorage};
use weft_types::shared::JobId;
use weft_types::workloads::{CreateInstance, CreateInstanceOutcome, JobStarted};

use super::{attempt_ports, invoke, outcome_name, Entry, Invoked};
use crate::quota::live;
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "job-task-instances",
    drop: &[],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsJobStarted(started) => Some(json!({
            "job_id": started.job_id.encode(),
            "bound_tasks": started.bound_tasks.encode(),
        })),
        _ => None,
    }
}

fn field<T: Codec>(value: &Value, name: &str) -> Result<T, String> {
    T::decode(member(value, name)?).map_err(|error| at(name, error))
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    Ok(SystemEvent::WorkloadsJobStarted(JobStarted {
        job_id: field::<JobId>(value, "job_id")?,
        bound_tasks: field::<i64>(value, "bound_tasks")?,
    }))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsJobStarted(started) = event else {
        return None;
    };
    let ports = attempt_ports()?;
    // StartJob moved the Job the event names, so its row is there; none: nothing to make.
    let job = JobStorage::get(&ports, &started.job_id)?;
    let taken: Vec<i64> = InstanceStorage::list(&ports)
        .into_iter()
        .filter(|row| live(row.state) && row.data.job_id.as_ref() == Some(&started.job_id))
        .filter_map(|row| row.data.task_index)
        .collect();
    let mut published = Vec::new();
    for task_index in (0..started.bound_tasks).filter(|index| !taken.contains(index)) {
        let input = CreateInstance {
            workload_id: job.data.workload_id.clone(),
            job_id: Some(started.job_id.clone()),
            task_index: Some(task_index),
            invocation_id: None,
            box_id: None,
            session_id: None,
            replica_index: None,
            warm_pool_id: None,
            service_id: None,
            scope_node_id: None,
        };
        match invoke(system, |system| system.weft_control.create_instance(input)) {
            Ok((CreateInstanceOutcome::Created { .. }, events)) => published.extend(events),
            Ok((outcome, _)) => return Some(Invoked::Refused(outcome_name(&outcome))),
            Err(unmet) => return Some(Invoked::Unmet(unmet)),
        }
    }
    Some(Invoked::Accepted(published))
}
