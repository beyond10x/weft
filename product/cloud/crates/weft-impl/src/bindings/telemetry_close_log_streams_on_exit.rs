//! `telemetry-close-log-streams-on-exit` (`ess/components.yaml`): `weft.workloads.InstanceExited`
//! -> `weft.observability.CloseInstanceLogStreams`, mapping `instance_id`.
//!
//! Removed from the synthesis input by `story:cloudspec-submission-job-exit`, which puts the
//! first `InstanceExited` on the path: the outcome `closed` carries one `LogStreamClosed` (ESS
//! 0.99.2 generates one member for `each: changed`), so closing the streams of an instance with
//! none open has no event to publish but a made-up one (OPEN (ESS) in `ess/components.yaml`, no
//! ESS story yet). The binding reads the instance's streams from the ports of the attempt
//! ([`super::attempt_ports`]) and does not react to an instance with no `Open` stream: closing
//! none changes no row. Retries every refusal and an unmet obligation, as declared. The held
//! form of `InstanceExited` is the one of `job-task-succeeded-on-exit`.

use serde_json::Value;
use weft_system::SystemEvent;
use weft_types::behaviour::LogStreamStorage;
use weft_types::observability::{CloseInstanceLogStreams, LogStreamState};

use super::job_task_succeeded_on_exit::{decode_exited, encode_exited};
use super::{attempt_ports, invoke, Entry, Invoked};
use crate::System;

/// The entry of the runner.
pub const ENTRY: Entry = Entry {
    id: "telemetry-close-log-streams-on-exit",
    drop: &[],
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
    let ports = attempt_ports()?;
    let open = LogStreamStorage::list(&ports).into_iter().any(|stream| {
        stream.data.instance_id == exited.instance_id && stream.state == LogStreamState::Open
    });
    if !open {
        return None;
    }
    let input = CloseInstanceLogStreams {
        instance_id: exited.instance_id.clone(),
    };
    Some(
        match invoke(system, |system| {
            system.weft_control.close_instance_log_streams(input)
        }) {
            Ok((_closed, published)) => Invoked::Accepted(published),
            Err(unmet) => Invoked::Unmet(unmet),
        },
    )
}
