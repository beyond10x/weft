//! `job-task-succeeded-on-exit` (`ess/components.yaml`, block "R1 for a Job workload", the
//! owner's decision D-C2 = A: exit 0 -> `RecordTaskSucceeded`): `weft.workloads.InstanceExited`
//! where `service_id` is absent and `exit_code` is 0 -> `weft.workloads.RecordTaskSucceeded`,
//! mapping the `job_id` and `task_index` of the instance's row. The declared mapping names the
//! conversion `InstanceId -> JobId` of `ess/components.yaml`, which stands for that read, and the
//! literal task index 0.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2), and because ESS maps a binding only from its event and compares no Integer leaf in a
//! binding's `where:` (the OPEN (ESS) of the block). Drops `not-running`; retries every other
//! refusal.
//!
//! The instance is read from the ports of the attempt ([`super::attempt_ports`]): an instance
//! that names no Job (or no task index), or that no row holds, is not reacted to; nor is an exit
//! other than 0 (a failed task is `RetryTask` or `FailJob`, not this binding). The held form of
//! `InstanceExited` is written here ([`encode_exited`], [`decode_exited`]) from the codecs of its
//! fields, for this entry and `edge-backend-exited`, `telemetry-close-log-streams-on-exit` and
//! `telemetry-retire-series-on-exit`; it names no scope, so the invocation acts as no caller
//! (`RecordTaskSucceeded` reads none).
//!
//! OPEN: `RecordTaskSucceeded` counts, and keeps no record of which task succeeded, so a
//! repeated delivery of this `InstanceExited` while the Job runs counts its task twice. With
//! parallelism 1 the first delivery completes the Job (`job-complete-on-task`) and a repeat is
//! refused `not-running`.

use serde_json::{json, Value};
use weft_system::SystemEvent;
use weft_types::behaviour::InstanceStorage;
use weft_types::shared::{InstanceId, ServiceId, WorkloadId};
use weft_types::workloads::{InstanceExited, RecordTaskSucceeded, RecordTaskSucceededOutcome};

use super::{attempt_ports, invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::{at, member, Codec};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "job-task-succeeded-on-exit",
    drop: &["not-running"],
    invoke: deliver,
    encode: encode_exited,
    decode: decode_exited,
};

/// The held form of `InstanceExited`, for this entry and the other hand-run bindings of the event.
pub(super) fn encode_exited(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsInstanceExited(exited) => Some(json!({
            "instance_id": exited.instance_id.encode(),
            "exit_code": exited.exit_code.encode(),
            "workload_id": exited.workload_id.encode(),
            "service_id": exited.service_id.encode(),
            "replica_index": exited.replica_index.encode(),
            "restart": exited.restart.encode(),
        })),
        _ => None,
    }
}

fn field<T: Codec>(value: &Value, name: &str) -> Result<T, String> {
    T::decode(member(value, name)?).map_err(|error| at(name, error))
}

/// The `InstanceExited` written by [`encode_exited`].
pub(super) fn decode_exited(value: &Value) -> Result<SystemEvent, String> {
    Ok(SystemEvent::WorkloadsInstanceExited(InstanceExited {
        instance_id: field::<InstanceId>(value, "instance_id")?,
        exit_code: field::<i64>(value, "exit_code")?,
        workload_id: field::<WorkloadId>(value, "workload_id")?,
        service_id: field::<Option<ServiceId>>(value, "service_id")?,
        replica_index: field::<Option<i64>>(value, "replica_index")?,
        restart: field::<bool>(value, "restart")?,
    }))
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceExited(exited) = event else {
        return None;
    };
    if exited.service_id.is_some() || exited.exit_code != 0 {
        return None;
    }
    let instance =
        attempt_ports().and_then(|ports| InstanceStorage::get(&ports, &exited.instance_id))?;
    let input = RecordTaskSucceeded {
        job_id: instance.data.job_id?,
        task_index: instance.data.task_index?,
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.record_task_succeeded(input)
        }) {
            Ok((RecordTaskSucceededOutcome::Recorded { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
