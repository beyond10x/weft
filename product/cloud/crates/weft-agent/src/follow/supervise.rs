//! The processes of a process workload (`story:slot-node-process`): an instance whose workload
//! has package kind `Process` starts through the `workloads` port as a `Service` replica does
//! (`Follower::start_workloads`), and the agent goes on holding each of its processes once it ran.
//!
//! - Start: each process's spec adds what the workload names ([`run_spec`]): its host user
//!   (`run_as`), its stop (`grace`, `kill`), and for the `Main` process its prestart steps in
//!   list order, a privileged step passed as such. The port runs the steps before the command; a
//!   step that fails ends the start in `failed`, a report line with the failure, and the instance
//!   does not reach `Running`.
//! - A process that ran is recorded in the node's own records with `weft.node.StartProcess`
//!   ([`NodeRecords`](crate::records::NodeRecords)) and observed once a pass. Every process
//!   started through the port is held so, a `Service` replica's too: its workload names no
//!   `restart`, so an exit keeps it down.
//! - An exit that keeps a process down (by the workload's `restart`, or under `Never`) ends it
//!   for good and counts in its instance's exit bookkeeping as an exit through the exec port
//!   does (`Follower::ended`): an instance of restart mode `Never` whose processes all exited 0
//!   is recorded with `RecordInstanceExited`.
//! - Restart: the agent passes the port the restart rule `Never` and decides itself, by the
//!   workload's `restart`: after an exit it keeps the process down (`policy`, or a code in
//!   `keep_down_on_exit`), or waits `delay`, calls [`Workloads::restart`] and records the restart
//!   with `weft.node.RestartProcess` (reason `Exited`, the restarts in the policy's window, and
//!   the delay as the wait, within the policy's backoff bounds). Each exit and restart is a report
//!   line.
//! - Stop: a process whose assignment no longer reads `Active` on this node (released, ended, or
//!   gone from the work list) or is ordered stopped (`desired: Stop`) is stopped through the port
//!   with the workload's grace and kill scope, recorded with `weft.node.StopProcess`, reported,
//!   and no longer held: it is started nowhere else.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use weft_platform::node::{
    KillScope, Observation, PrestartStep, ProcessExit, RestartMode, RestartRule, StopRule,
    WorkloadError, WorkloadKey, WorkloadSpec, WorkloadState,
};

use super::{lock, Follower, Start};

/// Whether the row of `weft.workloads.NodeInstanceRuns` names a workload of package kind
/// `Process`.
pub(super) fn is_process(run: &Value) -> bool {
    run["package_kind"] == "Process"
}

/// `spec` of `process` (a row of an instance's `processes`) with what `run`, the instance's row
/// of `weft.workloads.NodeInstanceRuns`, names of its workload: the host user, the stop, and for
/// the `Main` process the prestart steps. A `run` without them leaves the port's defaults.
pub(super) fn run_spec(mut spec: WorkloadSpec, process: &Value, run: &Value) -> WorkloadSpec {
    if let Some(user) = run["run_as"]["user"].as_str() {
        spec = spec.run_as(user);
    }
    if !run["stop"].is_null() {
        spec = spec.stop(stop_rule(&run["stop"]));
    }
    if process["role"] == "Main" {
        for step in run["prestart"].as_array().into_iter().flatten() {
            spec = spec.prestart(
                PrestartStep::new(strings(&step["command"])).privileged(step["privileged"] == true),
            );
        }
    }
    spec
}

/// Where a held process is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Started again; not yet seen `running` since.
    Starting,
    /// Seen `running`.
    Up,
    /// Ended on its own; started again at this instant.
    Due(Instant),
    /// Ended on its own and kept down.
    Down,
    /// A start again that did not reach `running`.
    Failed,
}

/// The bounds of the instance's restart policy (`weft.workloads.RestartPolicy`) that a recorded
/// restart must keep.
#[derive(Debug, Clone, Copy)]
struct Backoff {
    initial: i64,
    max: i64,
    window: Duration,
}

/// One process started through the `workloads` port that ran and that this run holds.
pub(super) struct Supervised {
    instance_id: String,
    /// The start of its instance, whose exit bookkeeping an end of this process counts in.
    start: Arc<Start>,
    process: String,
    key: WorkloadKey,
    stop: StopRule,
    /// The workload's own restart rule, which the agent applies.
    restart: RestartRule,
    backoff: Backoff,
    phase: Phase,
    /// Its `weft.node.LocalProcess`, once `StartProcess` was answered.
    local_process_id: Option<String>,
    /// The restarts so far, and the instants of those within the policy's window.
    restarts: u32,
    restarted_at: Vec<Instant>,
}

impl Supervised {
    /// The process under `key` of `instance`, started as `start`, before it ran. Its stop and
    /// restart are the workload's as `run` names them; a workload that names no restart (a
    /// `Service` replica) is never restarted by the agent.
    pub(super) fn new(start: &Arc<Start>, instance: &Value, run: &Value, key: WorkloadKey) -> Self {
        Self {
            instance_id: start.instance_id.clone(),
            start: Arc::clone(start),
            process: key.process.clone(),
            key,
            stop: stop_rule(&run["stop"]),
            restart: restart_rule(&run["restart"]),
            backoff: backoff(&instance["restart_policy"]),
            phase: Phase::Starting,
            local_process_id: None,
            restarts: 0,
            restarted_at: Vec::new(),
        }
    }
}

impl Follower<'_> {
    /// Holds `process` from its first `observation` that ran (`running`, or `exited` before the
    /// agent saw it `running`): recorded with `weft.node.StartProcess`, and an exit already seen
    /// handled as [`Follower::tend`] handles one.
    pub(super) fn hold(
        &self,
        mut process: Supervised,
        observation: &Observation,
    ) -> Result<(), String> {
        self.local_process(&mut process);
        self.observed(&mut process, observation)?;
        lock(&self.supervised).push(process);
        Ok(())
    }

    /// One pass over the held processes: each whose assignment is no longer held, or is ordered
    /// stopped, is stopped and dropped; every other one is observed. `assignments` are this
    /// node's `Active` assignments.
    pub(super) fn tend(&self, assignments: &[Value]) -> Result<(), String> {
        let held = std::mem::take(&mut *lock(&self.supervised));
        let mut kept = Vec::with_capacity(held.len());
        for mut process in held {
            let assignment = assignments
                .iter()
                .find(|row| row["assignment_id"] == process.key.assignment.as_str());
            if assignment.is_none_or(|row| row["desired"] == "Stop") {
                if !self.stopped(&process)? {
                    kept.push(process);
                }
                continue;
            }
            let Some(workloads) = self.workloads else {
                kept.push(process);
                continue;
            };
            match workloads.observe(&process.key) {
                Ok(observation) => self.observed(&mut process, &observation)?,
                Err(error) => eprintln!("weft-agent: observe {}: {error}", process.key),
            }
            kept.push(process);
        }
        lock(&self.supervised).extend(kept);
        Ok(())
    }

    /// What `observation` of `process` asks of the agent.
    fn observed(&self, process: &mut Supervised, observation: &Observation) -> Result<(), String> {
        match observation.state {
            WorkloadState::Running => process.phase = Phase::Up,
            WorkloadState::Exited => match process.phase {
                Phase::Starting | Phase::Up => {
                    let exit = observation.last_exit.unwrap_or(ProcessExit::Signal);
                    let kept_down = !process.restart.restarts_after(exit);
                    self.write(|report| {
                        report.process_exited(
                            &process.instance_id,
                            &process.process,
                            exit,
                            kept_down,
                        )
                    })?;
                    if kept_down {
                        // Ended for good: counted as the exec path counts an exit, so an
                        // instance of restart mode `Never` whose processes all exited 0 is
                        // recorded `Exited`.
                        process.phase = Phase::Down;
                        self.ended(&process.start, exit == ProcessExit::Code(0));
                    } else {
                        process.phase = Phase::Due(Instant::now() + process.restart.delay);
                        self.restart_if_due(process)?;
                    }
                }
                Phase::Due(_) => self.restart_if_due(process)?,
                Phase::Down | Phase::Failed => {}
            },
            WorkloadState::Failed if process.phase != Phase::Failed => {
                process.phase = Phase::Failed;
                if let Some(failure) = &observation.failure {
                    self.write(|report| {
                        report.process_failed(&process.instance_id, &process.process, failure)
                    })?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Starts `process` again through the port when its delay passed, and records the restart
    /// with `weft.node.RestartProcess`.
    fn restart_if_due(&self, process: &mut Supervised) -> Result<(), String> {
        let Phase::Due(at) = process.phase else {
            return Ok(());
        };
        let Some(workloads) = self.workloads else {
            return Ok(());
        };
        if Instant::now() < at {
            return Ok(());
        }
        if let Err(error) = workloads.restart(&process.key) {
            eprintln!("weft-agent: restart {}: {error}", process.key);
            return Ok(());
        }
        process.phase = Phase::Starting;
        process.restarts += 1;
        let now = Instant::now();
        process
            .restarted_at
            .retain(|then| now.duration_since(*then) < process.backoff.window);
        process.restarted_at.push(now);
        self.write(|report| {
            report.process_restarted(&process.instance_id, &process.process, process.restarts)
        })?;
        self.record_restart(process);
        Ok(())
    }

    /// `weft.node.RestartProcess` of `process`, after `StartProcess` when its local process is
    /// not recorded yet. A refusal is logged; the process runs either way.
    fn record_restart(&self, process: &mut Supervised) {
        let Some(local_process_id) = self.local_process(process) else {
            return;
        };
        let in_window = i64::try_from(process.restarted_at.len()).unwrap_or(i64::MAX);
        let delay = i64::try_from(process.restart.delay.as_secs()).unwrap_or(i64::MAX);
        let wait = delay.clamp(process.backoff.initial, process.backoff.max);
        if let Err(refusal) =
            self.records
                .restart_process(&local_process_id, self.node_id, in_window, wait)
        {
            eprintln!("weft-agent: RestartProcess {}: {refusal}", process.key);
        }
    }

    /// The local process of `process`: recorded with `weft.node.StartProcess` the first time it
    /// is asked for; `None` while that is refused.
    fn local_process(&self, process: &mut Supervised) -> Option<String> {
        if process.local_process_id.is_none() {
            match self.records.start_process(
                self.node_id,
                &process.key.assignment,
                &process.instance_id,
                &process.process,
            ) {
                Ok(id) => process.local_process_id = Some(id),
                Err(refusal) => eprintln!("weft-agent: StartProcess {}: {refusal}", process.key),
            }
        }
        process.local_process_id.clone()
    }

    /// Stops `process` through the port with its stop rule, records and reports the stop:
    /// whether it is no longer held (a refusal other than an unknown workload keeps it, to stop
    /// again in the next pass).
    fn stopped(&self, process: &Supervised) -> Result<bool, String> {
        let end = match self
            .workloads
            .map(|workloads| workloads.stop(&process.key, &process.stop))
        {
            Some(Ok(observation)) => observation.stopped,
            Some(Err(WorkloadError::UnknownWorkload(_))) | None => None,
            Some(Err(error)) => {
                eprintln!("weft-agent: stop {}: {error}", process.key);
                return Ok(false);
            }
        };
        if let Some(local_process_id) = &process.local_process_id {
            if let Err(refusal) = self.records.stop_process(local_process_id, self.node_id) {
                eprintln!("weft-agent: StopProcess {}: {refusal}", process.key);
            }
        }
        self.write(|report| report.process_stopped(&process.instance_id, &process.process, end))?;
        Ok(true)
    }
}

/// The texts of a JSON list.
fn strings(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// A duration `<n>s`, `<n>m` or `<n>h` (`n` a whole number), as a workload states `stop.grace`
/// and `restart.delay`.
fn duration(text: &str) -> Option<Duration> {
    let (number, scale) = if let Some(number) = text.strip_suffix('s') {
        (number, 1)
    } else if let Some(number) = text.strip_suffix('m') {
        (number, 60)
    } else {
        (text.strip_suffix('h')?, 3600)
    };
    number
        .parse::<u64>()
        .ok()?
        .checked_mul(scale)
        .map(Duration::from_secs)
}

/// The stop rule of a workload's `stop` (`weft.workloads.Stop`): its grace and kill scope, each
/// the port's default when absent or not understood.
fn stop_rule(stop: &Value) -> StopRule {
    let default = StopRule::default();
    let grace = stop["grace"]
        .as_str()
        .and_then(duration)
        .unwrap_or(default.grace);
    let kill = match stop["kill"].as_str() {
        Some("Main") => KillScope::Main,
        Some("Mixed") => KillScope::Mixed,
        Some("Group") => KillScope::Group,
        _ => default.kill,
    };
    StopRule::new(grace, kill)
}

/// The restart rule of a workload's `restart` (`weft.workloads.ProcessRestart`): its policy,
/// delay and the exit codes that keep the process down. Absent: `Never`.
fn restart_rule(restart: &Value) -> RestartRule {
    let policy = match restart["policy"].as_str() {
        Some("Always") => RestartMode::Always,
        Some("OnFailure") => RestartMode::OnFailure,
        _ => RestartMode::Never,
    };
    let delay = restart["delay"]
        .as_str()
        .and_then(duration)
        .unwrap_or(Duration::ZERO);
    RestartRule::new(policy, delay).keep_down_on_exit(
        restart["keep_down_on_exit"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .filter_map(|code| u8::try_from(code).ok()),
    )
}

/// The backoff bounds of an instance's `restart_policy`.
fn backoff(policy: &Value) -> Backoff {
    let initial = policy["backoff_initial_seconds"].as_i64().unwrap_or(1);
    Backoff {
        initial,
        max: policy["backoff_max_seconds"]
            .as_i64()
            .unwrap_or(initial)
            .max(initial),
        window: Duration::from_secs(policy["backoff_window_seconds"].as_u64().unwrap_or(600)),
    }
}
