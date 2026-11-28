//! `r1-rollout-submit-job` (`ess/components.yaml`, block "R1 for a Job workload", the owner's
//! decision D-C2 = A): `weft.deployments.ReleaseStepStarted` where `kind == Rollout` and
//! `workload_id` is defined -> `weft.workloads.SubmitJob`, mapping `workload_id` and `scope_id`
//! from the event and `parallelism`, `gang` and `max_retries` from the workload's
//! `JobSettings`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2), and because ESS maps a binding only from its event (the OPEN (ESS) of the block). Drops
//! `not-a-job` and `workload-retired`; retries every other refusal.
//!
//! The workload is read from the ports of the attempt ([`super::attempt_ports`]):
//!
//! - a `Job` workload is sent its own `JobSettings`;
//! - a `Schedule` workload is not reacted to: its schedule starts its Jobs;
//! - any other workload, and a workload no row holds, is sent the counts of one task
//!   (parallelism 1, gang false, max_retries 0, the literals of the declared mapping), so
//!   `SubmitJob` answers by the workload (`no-workload`, `not-a-job`, ...) and never
//!   `bad-counts`.
//!
//! A repeated delivery of `ReleaseStepStarted` submits a second Job: no refusal of `SubmitJob`
//! reads the step. The invocation acts in the event's scope (the runner's caller of a hand-run
//! invocation, [`super`]), which `SubmitJob` compares with the mapped `scope_id`. The held form
//! of `ReleaseStepStarted` is the one of `r1-set-workload-replicas`.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::behaviour::WorkloadStorage;
use weft_types::deployments::StepKind;
use weft_types::workloads::{ShapeSettings, SubmitJob, SubmitJobOutcome};

use super::{attempt_ports, invoke, outcome_name, r1_set_workload_replicas, Entry, Invoked};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "r1-rollout-submit-job",
    drop: &["not-a-job", "workload-retired"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    (r1_set_workload_replicas::ENTRY.encode)(event)
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    (r1_set_workload_replicas::ENTRY.decode)(value)
}

/// The counts the workload's Job is submitted with: `None` for a Schedule workload.
fn counts(settings: Option<&ShapeSettings>) -> Option<(i64, bool, i64)> {
    match settings {
        Some(ShapeSettings::Job(job)) => Some((job.parallelism, job.gang, job.max_retries)),
        Some(ShapeSettings::Schedule(_)) => None,
        _ => Some((1, false, 0)),
    }
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::DeploymentsReleaseStepStarted(started) = event else {
        return None;
    };
    if started.kind != StepKind::Rollout {
        return None;
    }
    let workload_id = started.workload_id.as_ref()?;
    let settings = attempt_ports()
        .and_then(|ports| WorkloadStorage::get(&ports, workload_id))
        .map(|row| row.data.settings);
    let (parallelism, gang, max_retries) = counts(settings.as_ref())?;
    let input = SubmitJob {
        workload_id: workload_id.clone(),
        scope_id: started.scope_id.clone(),
        parallelism,
        gang,
        max_retries,
        schedule_id: None,
    };
    Some(
        match invoke(system, |system| system.weft_control.submit_job(input)) {
            Ok((SubmitJobOutcome::Submitted { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
