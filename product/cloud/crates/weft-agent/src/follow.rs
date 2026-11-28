//! The node agent's work list (`ess/5-runtime/nodes.yaml`, view `weft.nodes.AssignmentsOfNode`):
//! once its session is open, the agent follows what is assigned to its node.
//!
//! Every [`PASS`] it reads its work list with its own `node_id` and the `Scheduled` and
//! `Starting` instances of its `Active` assignments in `weft.workloads.Instances`. It takes each
//! instance that reads `Scheduled`, and one that reads `Starting` only when this run sent its
//! `RecordArtifactVerified` and took no answer (the command took effect and its answer was lost).
//! Any other instance in `Starting` was started by another run of the agent, one before a restart,
//! and is left as it is: this run starts nothing twice. For each instance it takes:
//!
//! 1. `weft.workloads.RecordArtifactVerified` as the NodeAgent, with the instance's own
//!    `artifact_digest`: `Scheduled` -> `Starting` under its guards. The skeleton fetches no
//!    artifact and checks no signature: the snippet is the command of the instance's process.
//!    Its answer is `starting`, whose `InstanceHealthStarting` the hand-run binding
//!    `edge-backend-restarting` delivers (`RecordBackendNotReady`, `not-ready` dropped on a first
//!    start). An answer `501` with `committed: true` is taken as `starting` too: it took effect,
//!    and a generated binding of an event it published reached a command no behaviour implements;
//!    only `starting` publishes.
//! 2. `{"started": <instance_id>, "pass": <n>}` in the report file.
//! 3. Each process through the `exec` port ([`Exec::run`]), on a thread of its own: `run` blocks
//!    until the exec ends, and the passes over the work list go on meanwhile. The limits of a
//!    process come from its instance ([`limits`]). A process of a `Service` replica goes through
//!    the `workloads` port instead when the follower has one ([`Follower::workloads`],
//!    `story:workload-environment-and-args`): [`Workloads::start`] under the key of its
//!    assignment and process name, with the process's command and environment as its instance
//!    names them, then [`Workloads::observe`] once a pass until it reads `running` (or `exited`:
//!    it ran). A refused start, or a start that ends `failed`, `stopped` or `unknown`, is one
//!    report line with its code ([`workload_refusal_code`]) and is not started again.
//! 4. `weft.workloads.RecordInstanceRunning` as the NodeAgent, once every process of the instance
//!    is `running`, or has `exited` before the agent saw it `running` (it ran).
//! 5. The exit code and output of each process that `exited`, one line of the report file.
//! 6. `weft.workloads.RecordInstanceExited` with exit code 0 as the NodeAgent
//!    (`story:cloudspec-submission-job-exit`), once every process of an instance whose restart
//!    mode is `Never` has exited 0 and its `RecordInstanceRunning` was answered. An instance of
//!    another restart mode is not reported (its owner restarts an exit: the skeleton `Service`),
//!    nor one with a process that ended otherwise (a non-zero exit, a signal, a refusal; recording
//!    a failure is `RecordInstanceFailed`, outside this story), nor one with no process (it has no
//!    exit code to report).
//!
//! A start the place refused before the process ran with `retriable: true` (the place says it
//! started nothing) is started again [`RETRY_AFTER`] passes later, while the instance reads
//! `Starting`, at most [`ATTEMPTS`] times in all. Any other refusal is not started again: a start
//! refused `Unavailable` may have started the process (the exec port's contract: "a start that is
//! not answered is refused `Unavailable`, never started again"), and the others will not change.
//! A refusal it does not start again (not retriable, or the last attempt) is one report line
//! `{"instance_id": <id>, "refusal": <code>}` ([`refusal_code`]).
//!
//! The time between passes and the passes before a refused start is made again are
//! [`Follower::pacing`], [`PASS`] and [`RETRY_AFTER`] unless a caller sets them.
//!
//! With several processes, a process refused for good closes the instance's start: the others run
//! to their ends and their exits are reported, but `RecordInstanceRunning` is never sent, since
//! one process of the instance never ran (recording the failure is `RecordInstanceFailed`, outside
//! this epic), and nothing is left waiting for the refused process. While a refused process waits
//! for its next attempt, `RecordInstanceRunning` waits with it; at most [`ATTEMPTS`] attempts
//! later the process ran or was refused for good.
//!
//! After each pass it writes `{"pass": <n>}`, `n` counting from 1. An instance the agent took
//! once in this run is not taken again, whatever `RecordArtifactVerified` answered. A work list
//! that is not answered is read again in the next pass; a `RecordArtifactVerified` that is not
//! answered is sent again while the instance reads `Scheduled`; a `RecordInstanceRunning` that is
//! not answered is sent again in each pass while the instance reads `Starting`; a `RecordInstanceExited`
//! that is not answered is sent again at the start of each pass, before the work list is read
//! (the instance reads `Running` and is no longer on it). A `5xx` answer without
//! `committed: true` counts as no answer.
//!
//! An instance whose workload has package kind `Process` (`weft.workloads.NodeInstanceRuns`,
//! read in a pass that takes an instance) is verified the same way and then runs through the
//! `workloads` port as a `Service` replica does, never through the exec port (without a
//! `workloads` port its start is refused `not-served`). Its specs add the workload's host user,
//! stop and the `Main` process's prestart steps; once a process ran, the agent holds it, restarts
//! it by the workload's `restart` and stops it when its assignment is no longer held
//! (`follow::supervise`, `story:slot-node-process`).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Scope;
use std::time::Duration;

use serde_json::{json, Value};
use weft_client::{Answer, Client};
use weft_platform::node::{
    Exec, ExecError, ExecState, Limits, Observation, Process, WorkloadError, WorkloadKey,
    WorkloadSpec, WorkloadState, Workloads,
};
use weft_platform::Refusal;

use crate::agent::NODE_AGENT;
use crate::records::{NodeRecords, UNKEPT};
use crate::report::Report;

mod supervise;

use supervise::Supervised;

/// The time between two passes over the work list, by default.
pub const PASS: Duration = Duration::from_millis(500);

/// How many times one process is started, the first time included.
const ATTEMPTS: u32 = 5;

/// The passes from a refused start to the next start of the same process, by default: two
/// seconds at [`PASS`], so a place out of sandboxes is not asked again at every pass.
pub const RETRY_AFTER: u64 = 4;

/// The work list view.
const WORK_LIST: &str = "weft.nodes.AssignmentsOfNode";

/// The instances view.
const INSTANCES: &str = "weft.workloads.Instances";

/// What the node runs with how: the host user, prestart, stop and restart of a process workload.
const RUNS: &str = "weft.workloads.NodeInstanceRuns";

/// The command that records an instance `Running`.
const RUNNING: &str = "weft.workloads.RecordInstanceRunning";

/// The command that records an instance `Exited`.
const EXITED: &str = "weft.workloads.RecordInstanceExited";

/// What the agent follows its work list with.
pub struct Follower<'a> {
    client: &'a Client,
    node_id: &'a str,
    exec: &'a (dyn Exec + Sync),
    /// The `workloads` port, which starts the processes of a `Service` replica when present.
    workloads: Option<&'a (dyn Workloads + Sync)>,
    report: &'a Mutex<Report>,
    /// The instances whose processes all ran and whose `RecordInstanceRunning` was not answered.
    unrecorded: Mutex<HashMap<String, Arc<Start>>>,
    /// The instances whose `RecordInstanceExited` is due and was not answered.
    unexited: Mutex<HashSet<String>>,
    /// The refused starts to make again.
    retries: Mutex<Vec<Retry>>,
    /// The time between two passes.
    pass: Duration,
    /// The passes from a refused start to the next start of the same process.
    retry_after: u64,
    /// The node's own records (`weft.node`) of the processes of process workloads.
    records: &'a (dyn NodeRecords + Sync),
    /// The processes of process workloads that ran and that this run still holds.
    supervised: Mutex<Vec<Supervised>>,
}

/// The instances of this run that [`Follower::pass`] reads and writes.
#[derive(Default)]
struct Taken {
    /// Taken once in this run: not taken again.
    done: HashSet<String>,
    /// `RecordArtifactVerified` sent and not answered: `Starting` means it took effect.
    verifying: HashSet<String>,
}

/// What `RecordArtifactVerified` answered.
enum Verified {
    /// The instance is `Starting`.
    Starting,
    /// A refusal: the instance did not move.
    Refused,
    /// No answer: the command may or may not have taken effect.
    Unanswered(String),
}

/// The start of one instance: its processes that have not run yet, and what its exit waits for.
struct Start {
    instance_id: String,
    left: AtomicUsize,
    /// Its restart mode is `Never`: an exit 0 of every process is recorded.
    never: bool,
    /// How many processes it has.
    processes: usize,
    end: Mutex<End>,
}

/// What [`Follower::record_exit_when_due`] waits for.
#[derive(Default)]
struct End {
    /// `RecordInstanceRunning` was answered.
    running: bool,
    /// The processes that exited 0.
    exited: usize,
    /// A process ended otherwise: another exit code, a signal, a refusal for good.
    failed: bool,
    /// `RecordInstanceExited` was sent once.
    sent: bool,
}

/// A refused start of `process` to make again in pass `due`, as attempt `attempt`.
struct Retry {
    start: Arc<Start>,
    process: Process,
    attempt: u32,
    due: u64,
}

impl<'a> Follower<'a> {
    /// A follower of `node_id`'s work list on the control host of `client`, starting processes
    /// through `exec` and writing to `report`.
    pub fn new(
        client: &'a Client,
        node_id: &'a str,
        exec: &'a (dyn Exec + Sync),
        report: &'a Mutex<Report>,
    ) -> Self {
        Self {
            client,
            node_id,
            exec,
            workloads: None,
            report,
            unrecorded: Mutex::new(HashMap::new()),
            unexited: Mutex::new(HashSet::new()),
            retries: Mutex::new(Vec::new()),
            pass: PASS,
            retry_after: RETRY_AFTER,
            records: &UNKEPT,
            supervised: Mutex::new(Vec::new()),
        }
    }

    /// The same follower with `records` as the node's own records (`weft.node`). Without them,
    /// each `StartProcess`, `RestartProcess` and `StopProcess` is refused, logged and reported,
    /// and the process runs either way.
    #[must_use]
    pub fn records(mut self, records: &'a (dyn NodeRecords + Sync)) -> Self {
        self.records = records;
        self
    }

    /// The same follower with `pass` between two passes and `retry_after` passes from a refused
    /// start to the next start of the same process (defaults [`PASS`] and [`RETRY_AFTER`]).
    #[must_use]
    pub fn pacing(mut self, pass: Duration, retry_after: u64) -> Self {
        self.pass = pass;
        self.retry_after = retry_after;
        self
    }

    /// The same follower with the `workloads` port: the processes of a `Service` replica and of
    /// a process workload start through it, every other process through the `exec` port. Without
    /// it, the start of a process workload is refused `not-served`.
    #[must_use]
    pub fn workloads(mut self, workloads: &'a (dyn Workloads + Sync)) -> Self {
        self.workloads = Some(workloads);
        self
    }

    /// Follows the work list until the report file cannot be written. The starts run on threads
    /// of this call's scope.
    ///
    /// # Errors
    ///
    /// The report file cannot be written.
    pub fn follow(&self) -> Result<(), String> {
        std::thread::scope(|scope| {
            let mut taken = Taken::default();
            let mut pass = 0;
            loop {
                pass += 1;
                self.pass(scope, pass, &mut taken)?;
                self.write(|report| report.pass(pass))?;
                std::thread::sleep(self.pass);
            }
        })
    }

    /// One pass over the work list: the `RecordInstanceExited` not answered are sent again, then
    /// the work list is read, the `RecordInstanceRunning` not answered are sent again, the
    /// refused starts that are due are made again, and every instance it takes is started; each
    /// start on a thread of `scope`.
    fn pass<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        taken: &mut Taken,
    ) -> Result<(), String> {
        self.record_unexited();
        let (instances, assignments) = match self.work() {
            Ok(work) => work,
            Err(error) => {
                eprintln!("weft-agent: pass {pass}: the work list: {error}");
                return Ok(());
            }
        };
        self.record_unrecorded(&instances);
        self.retry_due(scope, pass, &instances);
        // How the workload of each instance on the node runs, read once in a pass that takes an
        // instance.
        let mut runs: Option<HashMap<String, Value>> = None;
        for instance in &instances {
            let Some(instance_id) = instance["instance_id"].as_str().map(str::to_owned) else {
                continue;
            };
            if taken.done.contains(&instance_id) {
                continue;
            }
            let takeable = match instance["state"].as_str() {
                Some("Scheduled") => true,
                Some("Starting") => taken.verifying.contains(&instance_id),
                _ => false,
            };
            if !takeable {
                continue;
            }
            if runs.is_none() {
                match self.runs() {
                    Ok(read) => runs = Some(read),
                    Err(error) => {
                        eprintln!("weft-agent: pass {pass}: {error}");
                        break;
                    }
                }
            }
            let run = runs
                .as_ref()
                .and_then(|runs| runs.get(&instance_id))
                .cloned()
                .unwrap_or(Value::Null);
            match instance["state"].as_str() {
                Some("Scheduled") => {
                    taken.verifying.insert(instance_id.clone());
                    match self.verify(instance) {
                        Verified::Starting => {}
                        Verified::Refused => {
                            taken.verifying.remove(&instance_id);
                            taken.done.insert(instance_id);
                            continue;
                        }
                        Verified::Unanswered(error) => {
                            eprintln!("weft-agent: RecordArtifactVerified {instance_id}: {error}");
                            continue;
                        }
                    }
                }
                // This run's `RecordArtifactVerified` took effect; its answer was lost.
                Some("Starting") if taken.verifying.contains(&instance_id) => {}
                // Started by another run of the agent: left as it is.
                _ => continue,
            }
            taken.verifying.remove(&instance_id);
            taken.done.insert(instance_id.clone());
            self.start(scope, pass, instance, &run, instance_id)?;
        }
        self.tend(&assignments)
    }

    /// Starts the processes of `instance`, whose workload runs as `run` (its row of
    /// `weft.workloads.NodeInstanceRuns`, `null` when absent), each on a thread of `scope`.
    fn start<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        instance: &Value,
        run: &Value,
        instance_id: String,
    ) -> Result<(), String> {
        let process_workload = supervise::is_process(run);
        if process_workload || !instance["service_id"].is_null() {
            if let Some(workloads) = self.workloads {
                return self.start_workloads(scope, pass, instance, run, instance_id, workloads);
            }
        }
        if process_workload {
            // A process workload runs on the host as itself, never as an exec in a sandbox.
            self.write(|report| report.started(&instance_id, pass))?;
            eprintln!(
                "weft-agent: {instance_id} is a process workload and no workloads port runs it"
            );
            return self.write(|report| report.refused(&instance_id, "not-served"));
        }
        let processes = processes(instance);
        self.write(|report| report.started(&instance_id, pass))?;
        // An instance without a process has nothing to wait for: it runs as it starts.
        let start = Arc::new(Start {
            instance_id,
            left: AtomicUsize::new(processes.len().max(1)),
            never: instance["restart_policy"]["mode"] == "Never",
            processes: processes.len(),
            end: Mutex::new(End::default()),
        });
        if processes.is_empty() {
            self.ran(&start);
        }
        for process in processes {
            let start = Arc::clone(&start);
            scope.spawn(move || self.run(start, process, 1, pass));
        }
        Ok(())
    }

    /// Starts the processes of `instance` (a `Service` replica, or a process workload as `run`
    /// describes it) through `workloads`, each supervised to `running` on a thread of `scope`.
    /// A process of a process workload that ran is then held ([`Follower::tend`]).
    fn start_workloads<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        instance: &Value,
        run: &Value,
        instance_id: String,
        workloads: &'scope (dyn Workloads + Sync),
    ) -> Result<(), String> {
        let specs = workload_specs(instance, run);
        self.write(|report| report.started(&instance_id, pass))?;
        let start = Arc::new(Start {
            instance_id,
            left: AtomicUsize::new(specs.len().max(1)),
            never: instance["restart_policy"]["mode"] == "Never",
            processes: specs.len(),
            end: Mutex::new(End::default()),
        });
        if specs.is_empty() {
            self.ran(&start);
        }
        for (key, spec) in specs {
            let start = Arc::clone(&start);
            let held = Supervised::new(&start, instance, run, key.clone());
            scope.spawn(move || self.supervise(workloads, &start, &key, &spec, held));
        }
        Ok(())
    }

    /// Starts `spec` under `key` through `workloads` and observes it once a pass until it ran
    /// (`running`, or `exited` before the agent saw it `running`); a refusal, or an end in
    /// `failed`, `stopped` or `unknown`, is one report line (a failed start also one line with
    /// its failure), the instance is not recorded `Running`, and the process ended otherwise
    /// than with exit 0 ([`Follower::ended`]). A process that ran is `held` from then on
    /// ([`Follower::tend`]): its exits are seen there.
    fn supervise(
        &self,
        workloads: &(dyn Workloads + Sync),
        start: &Arc<Start>,
        key: &WorkloadKey,
        spec: &WorkloadSpec,
        held: Supervised,
    ) {
        let instance_id = &start.instance_id;
        let mut answer = workloads.start(key, spec);
        loop {
            let code = match answer {
                Ok(
                    observation @ Observation {
                        state: WorkloadState::Running | WorkloadState::Exited,
                        ..
                    },
                ) => {
                    self.ran(start);
                    if let Err(error) = self.hold(held, &observation) {
                        eprintln!("weft-agent: {instance_id}: {error}");
                    }
                    return;
                }
                Ok(Observation {
                    state: WorkloadState::Starting | WorkloadState::Restarting,
                    ..
                }) => {
                    std::thread::sleep(self.pass);
                    answer = workloads.observe(key);
                    continue;
                }
                Ok(Observation { state, failure, .. }) => {
                    if let Some(failure) = &failure {
                        if let Err(error) = self.write(|report| {
                            report.process_failed(instance_id, &key.process, failure)
                        }) {
                            eprintln!("weft-agent: {instance_id}: {error}");
                        }
                    }
                    format!("workload-{state:?}").to_lowercase()
                }
                Err(error) => workload_refusal_code(&error),
            };
            eprintln!("weft-agent: the workload {key} of {instance_id} did not run: {code}");
            if let Err(error) = self.write(|report| report.refused(instance_id, &code)) {
                eprintln!("weft-agent: {instance_id}: {error}");
            }
            self.ended(start, false);
            return;
        }
    }

    /// The `Scheduled` and `Starting` instances of the `Active` assignments of this node's work
    /// list, as rows of `weft.workloads.Instances`, each with the `assignment_id` of its
    /// assignment, and those assignments, as rows of the work list.
    fn work(&self) -> Result<(Vec<Value>, Vec<Value>), String> {
        let assigned = rows(
            self.client
                .view(WORK_LIST, &[("node_id", self.node_id)])
                .map_err(|error| format!("{WORK_LIST}: {error}"))?,
            WORK_LIST,
        )?;
        let assignments: Vec<Value> = assigned
            .into_iter()
            .filter(|row| row["state"] == "Active" && row["node_id"] == self.node_id)
            .collect();
        // Each instance with the assignment that holds it: the key of its workloads.
        let active: HashMap<Value, Value> = assignments
            .iter()
            .map(|row| (row["instance_id"].clone(), row["assignment_id"].clone()))
            .collect();
        if active.is_empty() {
            return Ok((Vec::new(), assignments));
        }
        let instances = rows(
            self.client
                .view(INSTANCES, &[])
                .map_err(|error| format!("{INSTANCES}: {error}"))?,
            INSTANCES,
        )?;
        let instances = instances
            .into_iter()
            .filter(|row| row["state"] == "Scheduled" || row["state"] == "Starting")
            .filter_map(|mut row| {
                let assignment_id = active.get(&row["instance_id"])?.clone();
                row["assignment_id"] = assignment_id;
                Some(row)
            })
            .collect();
        Ok((instances, assignments))
    }

    /// How the workload of each instance on this node runs, by instance: the rows of
    /// `weft.workloads.NodeInstanceRuns`.
    fn runs(&self) -> Result<HashMap<String, Value>, String> {
        let runs = rows(
            self.client
                .view(RUNS, &[("node_id", self.node_id)])
                .map_err(|error| format!("{RUNS}: {error}"))?,
            RUNS,
        )?;
        Ok(runs
            .into_iter()
            .filter_map(|row| Some((row["instance_id"].as_str()?.to_owned(), row)))
            .collect())
    }

    /// `RecordArtifactVerified` of `instance`.
    fn verify(&self, instance: &Value) -> Verified {
        let answer = match self.command(
            "weft.workloads.RecordArtifactVerified",
            json!({
                "instance_id": instance["instance_id"],
                "node_id": self.node_id,
                "artifact_digest": instance["artifact_digest"],
            }),
        ) {
            Ok(answer) => answer,
            Err(error) => return Verified::Unanswered(error),
        };
        if answer.body["outcome"] == "starting" || committed(&answer) {
            return Verified::Starting;
        }
        if answer.status >= 500 {
            return Verified::Unanswered(format!("answered {}: {}", answer.status, answer.body));
        }
        eprintln!(
            "weft-agent: RecordArtifactVerified {} answered {}: {}",
            instance["instance_id"], answer.status, answer.body
        );
        Verified::Refused
    }

    /// Sends `RecordInstanceRunning` again for each instance whose last one was not answered and
    /// that reads `Starting`; an instance that reads otherwise, or is gone from the work list, is
    /// no longer followed.
    fn record_unrecorded(&self, instances: &[Value]) {
        let unrecorded: Vec<Arc<Start>> = {
            let mut unrecorded = lock(&self.unrecorded);
            unrecorded.retain(|instance_id, _| reads_starting(instances, instance_id));
            unrecorded.values().cloned().collect()
        };
        for start in unrecorded {
            if self.record_running(&start.instance_id) {
                lock(&self.unrecorded).remove(&start.instance_id);
                self.running_answered(&start);
            }
        }
    }

    /// Sends `RecordInstanceExited` again for each instance whose last one was not answered.
    fn record_unexited(&self) {
        let unexited: Vec<String> = lock(&self.unexited).iter().cloned().collect();
        for instance_id in unexited {
            if self.record_exited(&instance_id) {
                lock(&self.unexited).remove(&instance_id);
            }
        }
    }

    /// Makes again each refused start due in `pass`, on a thread of `scope`, while its instance
    /// reads `Starting`; a start whose instance reads otherwise is dropped.
    fn retry_due<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        instances: &[Value],
    ) {
        let due: Vec<Retry> = {
            let mut retries = lock(&self.retries);
            let (due, later) = retries.drain(..).partition(|retry| retry.due <= pass);
            *retries = later;
            due
        };
        for retry in due {
            if !reads_starting(instances, &retry.start.instance_id) {
                eprintln!(
                    "weft-agent: {} no longer reads Starting: its refused process is not started again",
                    retry.start.instance_id
                );
                continue;
            }
            scope.spawn(move || self.run(retry.start, retry.process, retry.attempt, pass));
        }
    }

    /// Runs `process` of `start` through the exec port to its end, as attempt `attempt` made in
    /// pass `pass`, and writes its exit to the report. A refusal before the process ran is
    /// started again or reported.
    fn run(&self, start: Arc<Start>, process: Process, attempt: u32, pass: u64) {
        let mut ran = false;
        let ended = self.exec.run(&process, &mut |state| {
            if !ran && matches!(state, ExecState::Running | ExecState::Exited) {
                ran = true;
                self.ran(&start);
            }
        });
        let instance_id = &start.instance_id;
        match ended {
            Ok(exited) => {
                if !ran {
                    self.ran(&start);
                }
                if let Err(error) = self.write(|report| report.exited(instance_id, &exited)) {
                    eprintln!("weft-agent: {instance_id}: {error}");
                }
                self.ended(&start, exited.code == Some(0));
            }
            Err(error) if ran => {
                eprintln!("weft-agent: a process of {instance_id} through the exec port: {error}");
                self.ended(&start, false);
            }
            Err(error) if again(&error) && attempt < ATTEMPTS => {
                let due = pass + self.retry_after;
                eprintln!(
                    "weft-agent: a process of {instance_id} refused (attempt {attempt} of {ATTEMPTS}), started again in pass {due}: {error}"
                );
                lock(&self.retries).push(Retry {
                    start,
                    process,
                    attempt: attempt + 1,
                    due,
                });
            }
            Err(error) => {
                eprintln!(
                    "weft-agent: a process of {instance_id} refused (attempt {attempt} of {ATTEMPTS}), not started again: {error}"
                );
                let code = refusal_code(&error);
                if let Err(error) = self.write(|report| report.refused(instance_id, &code)) {
                    eprintln!("weft-agent: {instance_id}: {error}");
                }
                self.ended(&start, false);
            }
        }
    }

    /// One process of `start` ran; after the last one, `RecordInstanceRunning`, followed again in
    /// the next passes when it is not answered.
    fn ran(&self, start: &Arc<Start>) {
        let before = start
            .left
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if before != Ok(1) {
            return;
        }
        if self.record_running(&start.instance_id) {
            self.running_answered(start);
        } else {
            lock(&self.unrecorded).insert(start.instance_id.clone(), Arc::clone(start));
        }
    }

    /// `RecordInstanceRunning` of `start` was answered.
    fn running_answered(&self, start: &Start) {
        lock(&start.end).running = true;
        self.record_exit_when_due(start);
    }

    /// A process of `start` ended for good: it exited 0 (`zero`), or it ended otherwise.
    fn ended(&self, start: &Start, zero: bool) {
        {
            let mut end = lock(&start.end);
            if zero {
                end.exited += 1;
            } else {
                end.failed = true;
            }
        }
        self.record_exit_when_due(start);
    }

    /// `RecordInstanceExited` of `start`, once: when its restart mode is `Never`, its
    /// `RecordInstanceRunning` was answered and every one of its processes (at least one) exited
    /// 0. Followed again in the next passes when it is not answered.
    fn record_exit_when_due(&self, start: &Start) {
        {
            let mut end = lock(&start.end);
            let due = start.never
                && start.processes > 0
                && end.running
                && !end.failed
                && end.exited == start.processes
                && !end.sent;
            if !due {
                return;
            }
            end.sent = true;
        }
        if !self.record_exited(&start.instance_id) {
            lock(&self.unexited).insert(start.instance_id.clone());
        }
    }

    /// `RecordInstanceExited` of `instance_id` with exit code 0: whether the control host
    /// answered it. A refusal is an answer (the instance does not read `Running` any more: it is
    /// not sent again).
    fn record_exited(&self, instance_id: &str) -> bool {
        match self.command(
            EXITED,
            json!({"instance_id": instance_id, "node_id": self.node_id, "exit_code": 0}),
        ) {
            Ok(answer)
                if matches!(
                    answer.body["outcome"].as_str(),
                    Some("exited" | "exited-restart")
                ) || committed(&answer) =>
            {
                true
            }
            Ok(answer) => {
                eprintln!(
                    "weft-agent: {EXITED} {instance_id} answered {}: {}",
                    answer.status, answer.body
                );
                answer.status < 500
            }
            Err(error) => {
                eprintln!("weft-agent: {EXITED} {instance_id}: {error}");
                false
            }
        }
    }

    /// `RecordInstanceRunning` of `instance_id`: whether the control host answered it.
    fn record_running(&self, instance_id: &str) -> bool {
        match self.command(
            RUNNING,
            json!({"instance_id": instance_id, "node_id": self.node_id}),
        ) {
            Ok(answer)
                if matches!(
                    answer.body["outcome"].as_str(),
                    Some("running-ready" | "running")
                ) || committed(&answer) =>
            {
                true
            }
            Ok(answer) => {
                eprintln!(
                    "weft-agent: {RUNNING} {instance_id} answered {}: {}",
                    answer.status, answer.body
                );
                answer.status < 500
            }
            Err(error) => {
                eprintln!("weft-agent: {RUNNING} {instance_id}: {error}");
                false
            }
        }
    }

    fn command(&self, command: &str, input: Value) -> Result<Answer, String> {
        self.client
            .command(command, NODE_AGENT, json!({}), input)
            .map_err(|error| format!("the control host did not answer: {error}"))
    }

    fn write(&self, line: impl FnOnce(&mut Report) -> std::io::Result<()>) -> Result<(), String> {
        line(&mut lock(self.report)).map_err(|error| format!("report: {error}"))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An answer `501` whose command took effect (a binding of its event is not implemented).
fn committed(answer: &Answer) -> bool {
    answer.status == 501 && answer.body["committed"] == true
}

/// Whether `instance_id` reads `Starting` in `instances`.
fn reads_starting(instances: &[Value], instance_id: &str) -> bool {
    instances
        .iter()
        .any(|row| row["instance_id"] == instance_id && row["state"] == "Starting")
}

/// Whether a start refused with `error` is made again: the place refused it with `retriable:
/// true`, so it started nothing and the same request may succeed later. `Unavailable` is not:
/// the place did not answer and may have started the process.
fn again(error: &ExecError) -> bool {
    matches!(
        error,
        ExecError::Refused {
            retriable: true,
            ..
        }
    )
}

/// The code of a refusal in the report: the place's own code (`exec.sandbox-unavailable`), or,
/// for a refusal of the port itself, its name.
fn refusal_code(error: &ExecError) -> String {
    match error {
        ExecError::Refused { code, .. }
        | ExecError::Ended {
            refusal: Some(code),
            ..
        } => code.clone(),
        ExecError::Ended { refusal: None, .. } => "ended".to_owned(),
        ExecError::EmptyCommand => "empty-command".to_owned(),
        ExecError::HostPath(_) => "host-path".to_owned(),
        ExecError::NoExit(_) => "no-exit".to_owned(),
        ExecError::Shared(Refusal::Unavailable { .. }) => "unavailable".to_owned(),
        ExecError::Shared(Refusal::NotServed { .. }) => "not-served".to_owned(),
        ExecError::Shared(Refusal::Invalid { .. }) => "invalid".to_owned(),
    }
}

/// The rows of a view's answer.
fn rows(answer: Answer, view: &str) -> Result<Vec<Value>, String> {
    if answer.status != 200 {
        return Err(format!(
            "{view} answered {}: {}",
            answer.status, answer.body
        ));
    }
    match answer.body["rows"].as_array() {
        Some(rows) => Ok(rows.clone()),
        None => Err(format!("{view}: no rows in {}", answer.body)),
    }
}

/// The limits of every process of `instance`. A process of a `Service` replica (its instance
/// names a `service_id`) runs until it is stopped, so it gets the longest `timeout` and
/// `cpu_time` the exec port accepts ([`Limits::LONGEST`], 24 hours); any other instance (a
/// snippet's invocation, a job's task) runs to its end under the port's default limits.
///
/// An exec is not a long-lived workload: substrate has none on the host driver (substrate gap 1,
/// cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 10), so a replica still ends after 24 hours. Running it until it is
/// stopped is `story:ports-node-workloads-local` (the `workloads` port).
fn limits(instance: &Value) -> Limits {
    if instance["service_id"].is_null() {
        Limits::default()
    } else {
        Limits {
            timeout: Limits::LONGEST,
            cpu_time: Limits::LONGEST,
            ..Limits::default()
        }
    }
}

/// The processes of an instance row: each one's command and environment, under the instance's
/// [`limits`].
fn processes(instance: &Value) -> Vec<Process> {
    let limits = limits(instance);
    process_rows(instance)
        .iter()
        .map(|process| {
            let mut started = Process::new(command(process)).limits(limits);
            for (name, value) in environment(process) {
                started = started.env(name, value);
            }
            started
        })
        .collect()
}

/// The processes of an instance row as workloads of the `workloads` port: each under the key of
/// the instance's assignment and the process's name, with its command and environment, and what
/// `run` (the instance's row of `weft.workloads.NodeInstanceRuns`) names of its workload: the
/// host user, the stop, and for the `Main` process the prestart steps
/// ([`supervise::run_spec`]). The port's defaults for what `run` does not name; never a restart
/// by the place (the agent decides restarts, `follow::supervise`).
fn workload_specs(instance: &Value, run: &Value) -> Vec<(WorkloadKey, WorkloadSpec)> {
    let assignment = instance["assignment_id"].as_str().unwrap_or_default();
    process_rows(instance)
        .iter()
        .map(|process| {
            let key = WorkloadKey::new(assignment, process["name"].as_str().unwrap_or_default());
            let mut spec = WorkloadSpec::new(command(process));
            for (name, value) in environment(process) {
                spec = spec.env(name, value);
            }
            (key, supervise::run_spec(spec, process, run))
        })
        .collect()
}

/// The `processes` of an instance row.
fn process_rows(instance: &Value) -> &[Value] {
    instance["processes"].as_array().map_or(&[], Vec::as_slice)
}

/// A process's `command`: its program and its arguments.
fn command(process: &Value) -> Vec<String> {
    process["command"]
        .as_array()
        .map(|argv| {
            argv.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A process's `environment`: each variable with its text value.
fn environment(process: &Value) -> Vec<(String, String)> {
    process["environment"]
        .as_object()
        .map(|environment| {
            environment
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// The code of a refusal of the `workloads` port in the report, named as [`refusal_code`] names
/// the refusals every port shares.
fn workload_refusal_code(error: &WorkloadError) -> String {
    match error {
        WorkloadError::UnknownWorkload(_) => "unknown-workload",
        WorkloadError::Shared(Refusal::Unavailable { .. }) => "unavailable",
        WorkloadError::Shared(Refusal::NotServed { .. }) => "not-served",
        WorkloadError::Shared(Refusal::Invalid { .. }) => "invalid",
    }
    .to_owned()
}
