//! The node's own records (`weft.node`): the local process of each process the agent runs, its
//! restarts and its stop.
//!
//! The control host keeps them (owner decision D-N1 = A, `story:node-records-on-control-host`):
//! `ess/components.yaml` places `weft.node` with weft-control, so `StartProcess`,
//! `RestartProcess` and `StopProcess` are routes of the control host, sent as the NodeAgent like
//! `RecordInstanceRunning`, and their events land in its one store and log; nothing relays them.
//! [`ControlRecords`] is the agent's production adapter of the port, which `agent::run_with`
//! gives its follower. A follower given none keeps no records: each call is refused, the refusal
//! is logged and reported, and the process runs either way.

use serde_json::{json, Value};
use weft_client::{Answer, Client};

use crate::agent::NODE_AGENT;

/// The `weft.node` commands the agent runs for the processes of a process workload.
pub trait NodeRecords {
    /// `weft.node.StartProcess` of `process` of `instance_id` under `assignment_id` on `node_id`:
    /// the identity of its `weft.node.LocalProcess`.
    ///
    /// # Errors
    ///
    /// The outcome of a refusal (`assignment-not-held`, `already-started`, …), or why the records
    /// could not be reached.
    fn start_process(
        &self,
        node_id: &str,
        assignment_id: &str,
        instance_id: &str,
        process: &str,
    ) -> Result<String, String>;

    /// `weft.node.RestartProcess` of `local_process_id` with reason `Exited`: the outcome
    /// (`restarted` or `backing-off`).
    ///
    /// # Errors
    ///
    /// As [`NodeRecords::start_process`].
    fn restart_process(
        &self,
        local_process_id: &str,
        node_id: &str,
        restarts_in_window: i64,
        backoff_seconds: i64,
    ) -> Result<String, String>;

    /// `weft.node.StopProcess` of `local_process_id`: the outcome (`stopped` or
    /// `already-stopped`).
    ///
    /// # Errors
    ///
    /// As [`NodeRecords::start_process`].
    fn stop_process(&self, local_process_id: &str, node_id: &str) -> Result<String, String>;
}

/// The records the control host keeps: each command sent to it over `client` as the NodeAgent,
/// with no caller attributes, the transport and caller of the agent's other records
/// (`RecordInstanceRunning`).
pub struct ControlRecords<'a> {
    client: &'a Client,
}

/// What a `501` whose command took effect answers: the command's effect and events were
/// committed and a binding of what it published could not be delivered, so the answer names no
/// outcome. Not sent again: a second send would make the command twice.
pub const COMMITTED: &str = "committed";

impl<'a> ControlRecords<'a> {
    /// The records of the control host of `client`.
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Sends `command` with `input`: its outcome when it is one of `taken`, with the answer's
    /// body; [`COMMITTED`] for a `501` whose command took effect.
    fn send(&self, command: &str, input: Value, taken: &[&str]) -> Result<(String, Value), String> {
        let answer = self
            .client
            .command(command, NODE_AGENT, json!({}), input)
            .map_err(|error| format!("{command}: the control host did not answer: {error}"))?;
        match answer.body["outcome"].as_str() {
            Some(outcome) if taken.contains(&outcome) => Ok((outcome.to_owned(), answer.body)),
            _ if committed(&answer) => Ok((COMMITTED.to_owned(), answer.body)),
            _ => Err(format!(
                "{command} answered {}: {}",
                answer.status, answer.body
            )),
        }
    }
}

/// An answer `501` whose command took effect.
fn committed(answer: &Answer) -> bool {
    answer.status == 501 && answer.body["committed"] == true
}

impl NodeRecords for ControlRecords<'_> {
    fn start_process(
        &self,
        node_id: &str,
        assignment_id: &str,
        instance_id: &str,
        process: &str,
    ) -> Result<String, String> {
        const COMMAND: &str = "weft.node.StartProcess";
        let (_, body) = self.send(
            COMMAND,
            json!({
                "node_id": node_id,
                "assignment_id": assignment_id,
                "instance_id": instance_id,
                "process": process,
            }),
            &["started"],
        )?;
        body["published"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|event| event["event"] == "weft.node.LocalProcessStarted")
            .and_then(|event| event["payload"]["local_process_id"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("{COMMAND} answered no local process: {body}"))
    }

    fn restart_process(
        &self,
        local_process_id: &str,
        node_id: &str,
        restarts_in_window: i64,
        backoff_seconds: i64,
    ) -> Result<String, String> {
        self.send(
            "weft.node.RestartProcess",
            json!({
                "local_process_id": local_process_id,
                "node_id": node_id,
                "reason": "Exited",
                "restarts_in_window": restarts_in_window,
                "backoff_seconds": backoff_seconds,
            }),
            &["restarted", "backing-off"],
        )
        .map(|(outcome, _)| outcome)
    }

    fn stop_process(&self, local_process_id: &str, node_id: &str) -> Result<String, String> {
        self.send(
            "weft.node.StopProcess",
            json!({"local_process_id": local_process_id, "node_id": node_id}),
            &["stopped", "already-stopped"],
        )
        .map(|(outcome, _)| outcome)
    }
}

/// Records that are not kept: every call refused.
pub(crate) struct Unkept;

/// The records a follower holds until it is given some.
pub(crate) static UNKEPT: Unkept = Unkept;

const NOT_KEPT: &str = "not-served: this follower was given no weft.node records";

impl NodeRecords for Unkept {
    fn start_process(&self, _: &str, _: &str, _: &str, _: &str) -> Result<String, String> {
        Err(NOT_KEPT.to_owned())
    }

    fn restart_process(&self, _: &str, _: &str, _: i64, _: i64) -> Result<String, String> {
        Err(NOT_KEPT.to_owned())
    }

    fn stop_process(&self, _: &str, _: &str) -> Result<String, String> {
        Err(NOT_KEPT.to_owned())
    }
}
