//! `edge-backend-exited` (`ess/components.yaml`): `weft.workloads.InstanceExited` ->
//! `weft.edge.RemoveBackend`, mapping `instance_id`, invoked through
//! `WeftEdge::remove_backend`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2) by `story:cloudspec-submission-job-exit`, which puts the first `InstanceExited` on the
//! path (an exit of a Job's task). Drops `no-backend` (the instance never was a backend, or a
//! repeated delivery); retries every other refusal. The held form of `InstanceExited` is the one
//! of `job-task-succeeded-on-exit`.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::edge::{RemoveBackend, RemoveBackendOutcome};

use super::job_task_succeeded_on_exit::{decode_exited, encode_exited};
use super::{invoke, outcome_name, Entry, Invoked};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "edge-backend-exited",
    drop: &["no-backend"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    encode_exited(event)
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    decode_exited(value)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceExited(exited) = event else {
        return None;
    };
    let input = RemoveBackend {
        instance_id: exited.instance_id.clone(),
    };
    Some(
        match invoke(system, |system| system.weft_edge.remove_backend(input)) {
            Ok((RemoveBackendOutcome::Removed { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
