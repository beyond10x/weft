//! `job-instance-scheduling-request` (`ess/components.yaml`, block "R1 for a Job workload", the
//! owner's decision D-C2 = A): `weft.workloads.InstanceCreated` where `job_id` is defined ->
//! `weft.scheduling.SubmitSchedulingRequest`, the mapping of `r1-submit-scheduling-request`
//! (which skips every Job instance): `instance_id`, `scope_id`, `identity`, `claims`,
//! `minimum_isolation` (as `min_isolation`), `zones` and `priority`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `already-requested` (a repeated delivery); retries every other refusal.
//!
//! An instance of a gang Job is left to the control plane, which submits it with its gang: the
//! binding reads the Job's `gang` from the ports of the attempt ([`super::attempt_ports`]) and
//! does not react to it. Nor does it react to an instance whose Job no row holds: it cannot
//! tell that Job's gang (`CreateInstance` refuses `no-job`, so no instance it created names
//! one). The held form of `InstanceCreated` is its codec's, as for
//! `r1-submit-scheduling-request`.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::behaviour::JobStorage;
use weft_types::scheduling::{SubmitSchedulingRequest, SubmitSchedulingRequestOutcome};
use weft_types::workloads::InstanceCreated;

use super::{attempt_ports, invoke, outcome_name, Entry, Invoked};
use crate::storage::codec::Codec;
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "job-instance-scheduling-request",
    drop: &["already-requested"],
    invoke: deliver,
    encode,
    decode,
};

fn encode(event: &SystemEvent) -> Option<Value> {
    match event {
        SystemEvent::WorkloadsInstanceCreated(created) => Some(created.encode()),
        _ => None,
    }
}

fn decode(value: &Value) -> Result<SystemEvent, String> {
    InstanceCreated::decode(value).map(SystemEvent::WorkloadsInstanceCreated)
}

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceCreated(created) = event else {
        return None;
    };
    let job_id = created.job_id.as_ref()?;
    let job = attempt_ports().and_then(|ports| JobStorage::get(&ports, job_id))?;
    if job.data.gang {
        return None;
    }
    let input = SubmitSchedulingRequest {
        instance_id: created.instance_id.clone(),
        scope_id: created.scope_id.clone(),
        claims: created.claims.clone(),
        min_isolation: created.minimum_isolation,
        zones: created.zones.clone(),
        priority: created.priority.clone(),
        gang_id: None,
        identity: Some(created.identity.clone()),
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.submit_scheduling_request(input)
        }) {
            Ok((SubmitSchedulingRequestOutcome::Requested { .. }, published)) => {
                Invoked::Accepted(published)
            }
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
