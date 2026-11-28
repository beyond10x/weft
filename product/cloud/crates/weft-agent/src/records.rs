//! The node's own records (`weft.node`, the domain of the component weft-node, which this agent
//! is): the local process of each process it runs, its restarts and its stop.
//!
//! weft-node is reached in process, not over the network (`ess/components.yaml`), so the control
//! host's surface does not serve `weft.node`. The agent reaches its records through this port.
//! A follower given none keeps no records: each call is refused, the refusal is logged, and the
//! process runs either way.

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

/// Records that are not kept: every call refused.
pub(crate) struct Unkept;

/// The records a follower holds until it is given some.
pub(crate) static UNKEPT: Unkept = Unkept;

const NOT_KEPT: &str = "not-served: this agent keeps no weft.node records";

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
