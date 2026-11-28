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
//!    An answer `501` with `committed: true` took effect as well (a generated binding of an event
//!    it published is not implemented yet); only `starting` publishes, so it is `starting`.
//! 2. `{"started": <instance_id>, "pass": <n>}` in the report file.
//! 3. Each process through the `exec` port ([`Exec::run`]), on a thread of its own: `run` blocks
//!    until the exec ends, and the passes over the work list go on meanwhile. The limits of a
//!    process come from its instance ([`limits`]).
//! 4. `weft.workloads.RecordInstanceRunning` as the NodeAgent, once every process of the instance
//!    is `running`, or has `exited` before the agent saw it `running` (it ran).
//! 5. The exit code and output of each process that `exited`, one line of the report file.
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
//! not answered is sent again in each pass while the instance reads `Starting`. A `5xx` answer
//! without `committed: true` counts as no answer.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Scope;
use std::time::Duration;

use serde_json::{json, Value};
use weft_client::{Answer, Client};
use weft_platform::node::{Exec, ExecError, ExecState, Limits, Process};
use weft_platform::Refusal;

use crate::agent::NODE_AGENT;
use crate::report::Report;

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

/// The command that records an instance `Running`.
const RUNNING: &str = "weft.workloads.RecordInstanceRunning";

/// What the agent follows its work list with.
pub struct Follower<'a> {
    client: &'a Client,
    node_id: &'a str,
    exec: &'a (dyn Exec + Sync),
    report: &'a Mutex<Report>,
    /// The instances whose processes all ran and whose `RecordInstanceRunning` was not answered.
    unrecorded: Mutex<HashSet<String>>,
    /// The refused starts to make again.
    retries: Mutex<Vec<Retry>>,
    /// The time between two passes.
    pass: Duration,
    /// The passes from a refused start to the next start of the same process.
    retry_after: u64,
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

/// The start of one instance: its processes that have not run yet.
struct Start {
    instance_id: String,
    left: AtomicUsize,
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
            report,
            unrecorded: Mutex::new(HashSet::new()),
            retries: Mutex::new(Vec::new()),
            pass: PASS,
            retry_after: RETRY_AFTER,
        }
    }

    /// The same follower with `pass` between two passes and `retry_after` passes from a refused
    /// start to the next start of the same process (defaults [`PASS`] and [`RETRY_AFTER`]).
    #[must_use]
    pub fn pacing(mut self, pass: Duration, retry_after: u64) -> Self {
        self.pass = pass;
        self.retry_after = retry_after;
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

    /// One pass over the work list: the `RecordInstanceRunning` not answered are sent again, the
    /// refused starts that are due are made again, and every instance it takes is started; each
    /// start on a thread of `scope`.
    fn pass<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        taken: &mut Taken,
    ) -> Result<(), String> {
        let instances = match self.work() {
            Ok(instances) => instances,
            Err(error) => {
                eprintln!("weft-agent: pass {pass}: the work list: {error}");
                return Ok(());
            }
        };
        self.record_unrecorded(&instances);
        self.retry_due(scope, pass, &instances);
        for instance in &instances {
            let Some(instance_id) = instance["instance_id"].as_str().map(str::to_owned) else {
                continue;
            };
            if taken.done.contains(&instance_id) {
                continue;
            }
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
            self.start(scope, pass, instance, instance_id)?;
        }
        Ok(())
    }

    /// Starts the processes of `instance`, each on a thread of `scope`.
    fn start<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        instance: &Value,
        instance_id: String,
    ) -> Result<(), String> {
        let processes = processes(instance);
        self.write(|report| report.started(&instance_id, pass))?;
        // An instance without a process has nothing to wait for: it runs as it starts.
        let start = Arc::new(Start {
            instance_id,
            left: AtomicUsize::new(processes.len().max(1)),
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

    /// The `Scheduled` and `Starting` instances of the `Active` assignments of this node's work
    /// list, as rows of `weft.workloads.Instances`.
    fn work(&self) -> Result<Vec<Value>, String> {
        let assigned = rows(
            self.client
                .view(WORK_LIST, &[("node_id", self.node_id)])
                .map_err(|error| format!("{WORK_LIST}: {error}"))?,
            WORK_LIST,
        )?;
        let active: HashSet<Value> = assigned
            .iter()
            .filter(|row| row["state"] == "Active" && row["node_id"] == self.node_id)
            .map(|row| row["instance_id"].clone())
            .collect();
        if active.is_empty() {
            return Ok(Vec::new());
        }
        let instances = rows(
            self.client
                .view(INSTANCES, &[])
                .map_err(|error| format!("{INSTANCES}: {error}"))?,
            INSTANCES,
        )?;
        Ok(instances
            .into_iter()
            .filter(|row| {
                active.contains(&row["instance_id"])
                    && (row["state"] == "Scheduled" || row["state"] == "Starting")
            })
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
        let unrecorded: Vec<String> = {
            let mut unrecorded = lock(&self.unrecorded);
            unrecorded.retain(|instance_id| reads_starting(instances, instance_id));
            unrecorded.iter().cloned().collect()
        };
        for instance_id in unrecorded {
            if self.record_running(&instance_id) {
                lock(&self.unrecorded).remove(&instance_id);
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
            }
            Err(error) if ran => {
                eprintln!("weft-agent: a process of {instance_id} through the exec port: {error}")
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
            }
        }
    }

    /// One process of `start` ran; after the last one, `RecordInstanceRunning`, followed again in
    /// the next passes when it is not answered.
    fn ran(&self, start: &Start) {
        let before = start
            .left
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if before != Ok(1) {
            return;
        }
        if !self.record_running(&start.instance_id) {
            lock(&self.unrecorded).insert(start.instance_id.clone());
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
    instance["processes"]
        .as_array()
        .map(|processes| {
            processes
                .iter()
                .map(|process| {
                    let argv = process["command"]
                        .as_array()
                        .map(|argv| {
                            argv.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let mut started = Process::new(argv).limits(limits);
                    if let Some(environment) = process["environment"].as_object() {
                        for (name, value) in environment {
                            if let Some(value) = value.as_str() {
                                started = started.env(name.clone(), value);
                            }
                        }
                    }
                    started
                })
                .collect()
        })
        .unwrap_or_default()
}
