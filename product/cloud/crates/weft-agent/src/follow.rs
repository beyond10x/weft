//! The node agent's work list (`ess/5-runtime/nodes.yaml`, view `weft.nodes.AssignmentsOfNode`):
//! once its session is open, the agent follows what is assigned to its node.
//!
//! Every [`PASS`] it reads its work list with its own `node_id` and takes each `Active`
//! assignment whose instance reads `Scheduled` in `weft.workloads.Instances`. An instance in
//! `Starting` or later was started, by this agent or by one before a restart, so reading the
//! view makes a restart and a seeded state work the same way. For each instance it takes:
//!
//! 1. `weft.workloads.RecordArtifactVerified` as the NodeAgent, with the instance's own
//!    `artifact_digest`: `Scheduled` -> `Starting` under its guards. The skeleton fetches no
//!    artifact and checks no signature: the snippet is the command of the instance's process.
//!    An answer `501` with `committed: true` took effect as well (a generated binding of an event
//!    it published is not implemented yet); only `starting` publishes, so it is `starting`.
//! 2. `{"started": <instance_id>, "pass": <n>}` in the report file.
//! 3. Each process through the `exec` port ([`Exec::run`]), on a thread of its own: `run` blocks
//!    until the exec ends, and the passes over the work list go on meanwhile.
//! 4. `weft.workloads.RecordInstanceRunning` as the NodeAgent, once every process of the instance
//!    is `running`, or has `exited` before the agent saw it `running` (it ran).
//! 5. The exit code and output of each process that `exited`, one line of the report file.
//!
//! After each pass it writes `{"pass": <n>}`, `n` counting from 1. An instance the agent took
//! once in this run is not taken again, whatever `RecordArtifactVerified` answered; a work list
//! or a command the control host did not answer is tried again in the next pass.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Scope;
use std::time::Duration;

use serde_json::{json, Value};
use weft_client::{Answer, Client};
use weft_platform::node::{Exec, ExecState, Process};

use crate::agent::NODE_AGENT;
use crate::report::Report;

/// The time between two passes over the work list.
pub const PASS: Duration = Duration::from_millis(500);

/// The work list view.
const WORK_LIST: &str = "weft.nodes.AssignmentsOfNode";

/// The instances view.
const INSTANCES: &str = "weft.workloads.Instances";

/// What the agent follows its work list with.
pub struct Follower<'a> {
    client: &'a Client,
    node_id: &'a str,
    exec: &'a (dyn Exec + Sync),
    report: &'a Mutex<Report>,
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
        }
    }

    /// Follows the work list until the report file cannot be written. The starts run on threads
    /// of this call's scope.
    ///
    /// # Errors
    ///
    /// The report file cannot be written.
    pub fn follow(&self) -> Result<(), String> {
        std::thread::scope(|scope| {
            let mut taken = HashSet::new();
            let mut pass = 0;
            loop {
                pass += 1;
                self.pass(scope, pass, &mut taken)?;
                self.write(|report| report.pass(pass))?;
                std::thread::sleep(PASS);
            }
        })
    }

    /// One pass over the work list: every instance it takes is started on a thread of `scope`.
    fn pass<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        pass: u64,
        taken: &mut HashSet<String>,
    ) -> Result<(), String> {
        let instances = match self.work() {
            Ok(instances) => instances,
            Err(error) => {
                eprintln!("weft-agent: pass {pass}: the work list: {error}");
                return Ok(());
            }
        };
        for instance in instances {
            let Some(instance_id) = instance["instance_id"].as_str().map(str::to_owned) else {
                continue;
            };
            if taken.contains(&instance_id) {
                continue;
            }
            let verified = match self.verify(&instance) {
                Ok(verified) => verified,
                Err(error) => {
                    eprintln!("weft-agent: RecordArtifactVerified {instance_id}: {error}");
                    continue;
                }
            };
            taken.insert(instance_id.clone());
            if !verified {
                continue;
            }
            let processes = processes(&instance);
            self.write(|report| report.started(&instance_id, pass))?;
            // An instance without a process has nothing to wait for: it runs as it starts.
            let start = Arc::new(Start {
                follower: self,
                instance_id,
                left: AtomicUsize::new(processes.len().max(1)),
            });
            if processes.is_empty() {
                start.ran();
            }
            for process in processes {
                let start = Arc::clone(&start);
                scope.spawn(move || start.run(&process));
            }
        }
        Ok(())
    }

    /// The `Scheduled` instances of the `Active` assignments of this node's work list, as rows
    /// of `weft.workloads.Instances`.
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
            .filter(|row| active.contains(&row["instance_id"]) && row["state"] == "Scheduled")
            .collect())
    }

    /// `RecordArtifactVerified` of `instance`: whether it moved the instance to `Starting`.
    fn verify(&self, instance: &Value) -> Result<bool, String> {
        let answer = self.command(
            "weft.workloads.RecordArtifactVerified",
            json!({
                "instance_id": instance["instance_id"],
                "node_id": self.node_id,
                "artifact_digest": instance["artifact_digest"],
            }),
        )?;
        let starting = answer.body["outcome"] == "starting"
            || (answer.status == 501 && answer.body["committed"] == true);
        if !starting {
            eprintln!(
                "weft-agent: RecordArtifactVerified {} answered {}: {}",
                instance["instance_id"], answer.status, answer.body
            );
        }
        Ok(starting)
    }

    fn command(&self, command: &str, input: Value) -> Result<Answer, String> {
        self.client
            .command(command, NODE_AGENT, json!({}), input)
            .map_err(|error| format!("the control host did not answer: {error}"))
    }

    fn write(&self, line: impl FnOnce(&mut Report) -> std::io::Result<()>) -> Result<(), String> {
        line(&mut self.report()).map_err(|error| format!("report: {error}"))
    }

    fn report(&self) -> MutexGuard<'_, Report> {
        self.report.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The start of one instance: its processes that have not run yet.
struct Start<'a> {
    follower: &'a Follower<'a>,
    instance_id: String,
    left: AtomicUsize,
}

impl Start<'_> {
    /// Runs `process` through the exec port to its end, and writes its exit to the report.
    fn run(&self, process: &Process) {
        let mut ran = false;
        let ended = self.follower.exec.run(process, &mut |state| {
            if !ran && matches!(state, ExecState::Running | ExecState::Exited) {
                ran = true;
                self.ran();
            }
        });
        match ended {
            Ok(exited) => {
                if !ran {
                    self.ran();
                }
                if let Err(error) = self
                    .follower
                    .write(|report| report.exited(&self.instance_id, &exited))
                {
                    eprintln!("weft-agent: {}: {error}", self.instance_id);
                }
            }
            Err(error) => eprintln!(
                "weft-agent: a process of {} through the exec port: {error}",
                self.instance_id
            ),
        }
    }

    /// One process of the instance ran; after the last one, `RecordInstanceRunning`.
    fn ran(&self) {
        let before = self
            .left
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if before != Ok(1) {
            return;
        }
        let command = "weft.workloads.RecordInstanceRunning";
        match self.follower.command(
            command,
            json!({"instance_id": self.instance_id, "node_id": self.follower.node_id}),
        ) {
            Ok(answer)
                if matches!(
                    answer.body["outcome"].as_str(),
                    Some("running-ready" | "running")
                ) => {}
            Ok(answer) => eprintln!(
                "weft-agent: {command} {} answered {}: {}",
                self.instance_id, answer.status, answer.body
            ),
            Err(error) => eprintln!("weft-agent: {command} {}: {error}", self.instance_id),
        }
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

/// The processes of an instance row: each one's command and environment.
fn processes(instance: &Value) -> Vec<Process> {
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
                    let mut started = Process::new(argv);
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
