//! `edge-backend-restore` (`ess/components.yaml`): `weft.workloads.InstanceReady` ->
//! `weft.edge.RestoreBackend`, mapping `instance_id`, invoked through
//! `WeftEdge::restore_backend`.
//!
//! Removed from the synthesis input for ESS
//! `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`
//! (E2). Drops `not-out-of-traffic` (the instance's first readiness, which `edge-backend-ready`
//! answers, or a repeated delivery); retries every other refusal.

use weft_system::SystemEvent;
use weft_types::edge::{RestoreBackend, RestoreBackendOutcome};

use super::edge_backend_ready::{decode_ready, encode_ready};
use super::{invoke, outcome_name, Entry, Invoked};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "edge-backend-restore",
    drop: &["not-out-of-traffic"],
    invoke: deliver,
    encode: encode_ready,
    decode: decode_ready,
};

fn deliver(system: &mut System, event: &SystemEvent) -> Option<Invoked> {
    let SystemEvent::WorkloadsInstanceReady(ready) = event else {
        return None;
    };
    let input = RestoreBackend {
        instance_id: ready.instance_id.clone(),
    };
    Some(
        match invoke(system, |system| system.weft_edge.restore_backend(input)) {
            Ok((RestoreBackendOutcome::Restored { .. }, published)) => Invoked::Accepted(published),
            Ok((outcome, _)) => Invoked::Refused(outcome_name(&outcome)),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
