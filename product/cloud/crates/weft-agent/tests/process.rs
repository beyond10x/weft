//! `process` (`story:slot-node-process`): an instance of a workload of package kind `Process` runs
//! through the `workloads` port, here the in-memory stub of `weft_platform::local::stub`. The
//! agent passes the prestart steps, the host user and the stop to the port, records the instance
//! `Running` once the port reads it running, decides each restart itself after the workload's
//! delay and records it with `weft.node.RestartProcess`, keeps the process down on an exit code
//! the workload names, and stops it with the workload's grace and kill scope when its assignment
//! is ordered stopped or released. No substrate daemon runs: the port is the stub. The node's
//! own records (`weft.node`, reached in process) are weft-impl's behaviours over a state file of
//! the test ([`LocalRecords`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_agent::follow::Follower;
use weft_agent::records::NodeRecords;
use weft_agent::report::Report;
use weft_client::{Client, Expect};
use weft_impl::{Ports, Row};
use weft_platform::local::stub::{StubEvent, StubWorkloads};
use weft_platform::node::{
    Exec, ExecError, ExecState, Exited, KillScope, PrestartStep as PortPrestart, Process,
    ProcessExit, RestartRule, StartFailure, StopRule, WorkloadKey, WorkloadState, Workloads,
};
use weft_platform::Refusal;
use weft_testkit::cloudspec::{self, ProcessWorld};
use weft_testkit::ControlHost;
use weft_types::behaviour::{Generated, WorkloadStorage};
use weft_types::node::obligations::{
    RestartProcessBehavior, StartProcessBehavior, StopProcessBehavior,
};
use weft_types::node::{
    RestartProcess, RestartProcessOutcome, StartProcess, StartProcessOutcome, StopProcess,
    StopProcessOutcome,
};
use weft_types::obligation::UnmetObligation;
use weft_types::primitives::Uuid;
use weft_types::shared::{
    AssignmentId, CloudId, InstanceId, IsolationLevel, LocalProcessId, NodeId, ProcessName,
};
use weft_types::workloads::{
    KillScope as SpecKill, PrestartStep, ProcessRestart, RestartMode, RestartReason, Stop,
};

use crate::seeds::{self, HostProcess, Placed};

/// How long a test waits for what the agent does.
const LIMIT: Duration = Duration::from_secs(20);

/// The time between two passes of the follower in these tests.
const PASS: Duration = Duration::from_millis(20);

/// The scheduler, which orders an assignment stopped (`weft.nodes.Stop`) and releases one whose
/// lease ended.
const SCHEDULER: &str = "weft.actors.Scheduler";

/// An exec port that runs nothing and counts its calls: a process instance never reaches it, so
/// a refusal in the report is the `workloads` port's, not this port's.
#[derive(Default)]
struct NoExec(AtomicUsize);

impl NoExec {
    fn calls(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Exec for NoExec {
    fn run(&self, _: &Process, _: &mut dyn FnMut(ExecState)) -> Result<Exited, ExecError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Refusal::NotServed {
            operation: "exec in a process test".to_owned(),
        }
        .into())
    }
}

/// A report file of this test process named `name`, absent.
fn report_path(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("process-{name}-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// The node's own records (`weft.node`) over a state file of this test: weft-impl's `weft.node`
/// behaviours, with the guards the specification declares, over the same rows the control host
/// was seeded with (the assignment and instance `StartProcess` reads). Each call is kept with the
/// outcome it got.
struct LocalRecords {
    node: Mutex<Generated<Ports>>,
    calls: Mutex<Vec<(&'static str, String)>>,
}

impl LocalRecords {
    /// Records over `rows`, in a state file of this test process named `name`.
    fn leaked(name: &str, rows: Vec<Row>) -> &'static Self {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "process-records-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        weft_testkit::seed(&path, rows);
        let ports = Ports::open(&path).expect("the records' state file opens");
        Box::leak(Box::new(Self {
            node: Mutex::new(Generated::new(ports)),
            calls: Mutex::default(),
        }))
    }

    /// Each call so far, with the outcome it got (`Started`, `Restarted`, `AssignmentNotHeld`, …).
    fn calls(&self) -> Vec<(&'static str, String)> {
        self.calls.lock().expect("the calls").clone()
    }

    /// Keeps `command` with the name of `outcome`: `Ok(answer)` when it is `taken`.
    fn keep<T: std::fmt::Debug>(
        &self,
        command: &'static str,
        outcome: Result<T, UnmetObligation>,
        taken: impl FnOnce(&T) -> Option<String>,
    ) -> Result<String, String> {
        let outcome = outcome.map_err(|unmet| format!("{unmet:?}"))?;
        let debug = format!("{outcome:?}");
        let name = debug
            .split([' ', '{', '('])
            .next()
            .unwrap_or_default()
            .to_owned();
        self.calls.lock().expect("the calls").push((command, name));
        taken(&outcome).ok_or(debug)
    }

    fn node(&self) -> std::sync::MutexGuard<'_, Generated<Ports>> {
        self.node.lock().expect("the records")
    }
}

impl NodeRecords for LocalRecords {
    fn start_process(
        &self,
        node_id: &str,
        assignment_id: &str,
        instance_id: &str,
        process: &str,
    ) -> Result<String, String> {
        let outcome = self.node().start_process(StartProcess {
            node_id: NodeId(Uuid(node_id.to_owned())),
            assignment_id: AssignmentId(Uuid(assignment_id.to_owned())),
            instance_id: InstanceId(Uuid(instance_id.to_owned())),
            process: ProcessName(process.to_owned()),
        });
        self.keep("StartProcess", outcome, |outcome| match outcome {
            StartProcessOutcome::Started {
                local_process_started,
            } => Some(local_process_started.local_process_id.0 .0.clone()),
            _ => None,
        })
    }

    fn restart_process(
        &self,
        local_process_id: &str,
        node_id: &str,
        restarts_in_window: i64,
        backoff_seconds: i64,
    ) -> Result<String, String> {
        let outcome = self.node().restart_process(RestartProcess {
            local_process_id: LocalProcessId(Uuid(local_process_id.to_owned())),
            node_id: NodeId(Uuid(node_id.to_owned())),
            reason: RestartReason::Exited,
            restarts_in_window,
            backoff_seconds,
        });
        self.keep("RestartProcess", outcome, |outcome| match outcome {
            RestartProcessOutcome::Restarted { .. } => Some("restarted".to_owned()),
            RestartProcessOutcome::BackingOff { .. } => Some("backing-off".to_owned()),
            _ => None,
        })
    }

    fn stop_process(&self, local_process_id: &str, node_id: &str) -> Result<String, String> {
        let outcome = self.node().stop_process(StopProcess {
            local_process_id: LocalProcessId(Uuid(local_process_id.to_owned())),
            node_id: NodeId(Uuid(node_id.to_owned())),
        });
        self.keep("StopProcess", outcome, |outcome| match outcome {
            StopProcessOutcome::Stopped { .. } => Some("stopped".to_owned()),
            StopProcessOutcome::AlreadyStopped { .. } => Some("already-stopped".to_owned()),
            _ => None,
        })
    }
}

/// Runs a follower of `node_id` on the control host at `host` on a thread of its own, with
/// `workloads` as its `workloads` port and `records` as its records (none: the follower's
/// defaults) and an exec port that counts its calls, writing to the report named `name`: the
/// report's path and the exec port.
fn follow(
    host: &ControlHost,
    node_id: &str,
    workloads: Option<&'static StubWorkloads>,
    records: Option<&'static LocalRecords>,
    name: &str,
) -> (PathBuf, &'static NoExec) {
    let report = report_path(name);
    let client: &'static Client = Box::leak(Box::new(Client::new(host.address())));
    let node_id: &'static str = Box::leak(node_id.to_owned().into_boxed_str());
    let open: &'static Mutex<Report> = Box::leak(Box::new(Mutex::new(
        Report::open(&report).expect("the report file opens"),
    )));
    let exec: &'static NoExec = Box::leak(Box::default());
    std::thread::spawn(move || {
        let mut follower = Follower::new(client, node_id, exec, open).pacing(PASS, 4);
        if let Some(workloads) = workloads {
            follower = follower.workloads(workloads);
        }
        if let Some(records) = records {
            follower = follower.records(records);
        }
        follower.follow()
    });
    (report, exec)
}

/// The calls of `command` in `records`, each with its outcome.
fn calls(records: &LocalRecords, command: &str) -> Vec<String> {
    records
        .calls()
        .into_iter()
        .filter(|(called, _)| *called == command)
        .map(|(_, outcome)| outcome)
        .collect()
}

/// The complete lines of the report file at `path`.
fn lines(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    complete
        .lines()
        .map(|line| serde_json::from_str(line).expect("a report line"))
        .collect()
}

/// Waits until `done` holds, at most [`LIMIT`]; `what` and `state` name what was missing.
fn wait_until(what: &str, state: impl Fn() -> String, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < LIMIT,
            "{what} after {LIMIT:?}: {}",
            state()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The `state` of `instance_id` in `weft.workloads.Instances`.
fn state(client: &Client, instance_id: &str) -> Value {
    let answer = client.view("weft.workloads.Instances", &[]).ok_body(200);
    answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["instance_id"] == json!(instance_id))
        .map(|row| row["state"].clone())
        .unwrap_or(Value::Null)
}

/// How many of `events` are `Restarting`.
fn restarts(events: &[StubEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, StubEvent::Restarting { .. }))
        .count()
}

/// How many of `events` are `Command`: the starts of the command, the first one included.
fn commands(events: &[StubEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, StubEvent::Command { .. }))
        .count()
}

/// The stop rules of the `Stopped` events.
fn stops(events: &[StubEvent]) -> Vec<StopRule> {
    events
        .iter()
        .filter_map(|event| match event {
            StubEvent::Stopped { rule, .. } => Some(*rule),
            _ => None,
        })
        .collect()
}

/// The request lines of `operation` in `log`.
fn requests<'a>(log: &'a [Value], operation: &str) -> Vec<&'a Value> {
    log.iter()
        .filter(|line| line["kind"] == "request" && line["operation"] == operation)
        .collect()
}

/// The workload key of the `main` process of `placed`.
fn key(placed: &Placed) -> WorkloadKey {
    WorkloadKey::new(placed.assignment_id.clone(), "main")
}

/// Acceptance of `story:slot-node-process`. A submission whose architecture has one `kind:
/// process` part with `environment`, one `prestart` step, `stop: {grace: 2s, kill: group}` and
/// `restart: {policy: always, delay: 1s, keep_down_on_exit: [2]}` is planned and applied; its
/// workload's instance, placed on a node by seed (placement is `story:slot-node-pool-placement`),
/// reaches `Running` through the stub `workloads` port after the prestart step ran. An exit 1 is
/// restarted after 1 s and recorded with `weft.node.RestartProcess`; an exit 2 keeps it down; a
/// stop reaches the port with grace 2 s and kill `Group`.
#[test]
fn a_planned_process_part_runs_restarts_after_its_delay_stays_down_on_its_code_and_stops_with_its_rule(
) {
    let mut host = ControlHost::start();
    let client = host.client();
    let world = ProcessWorld::new(&client);
    let applied = world.apply(
        &client,
        &cloudspec::submission_with_part("daemon", &cloudspec::process_part()),
    );
    assert_eq!(applied.status, 202, "{}", applied.body);
    let workload_id = applied.body["parts"][0]["workload_id"]
        .as_str()
        .expect("the applied part's workload")
        .to_owned();

    host.stop();
    let workload = {
        let ports = Ports::open(&host.state_path()).expect("the state file opens");
        WorkloadStorage::get(
            &ports,
            &weft_types::shared::WorkloadId(Uuid(workload_id.clone())),
        )
        .expect("the applied workload")
        .data
    };
    let mut rows = Vec::new();
    let placed = seeds::assigned(
        &mut rows,
        &CloudId(Uuid(weft_impl::context::mint())),
        &workload,
        IsolationLevel::None,
    );
    let records = LocalRecords::leaked("acceptance", rows.clone());
    weft_testkit::seed(&host.state_path(), rows);
    let host = host.restart();
    let client = host.client();
    let seq = host.last_seq();

    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let (report, exec) = follow(
        &host,
        &placed.node_id,
        Some(stub),
        Some(records),
        "acceptance",
    );
    let key = key(&placed);

    // Running through the port, after the prestart step ran.
    wait_until(
        "the instance does not read Running",
        || {
            format!(
                "{} {:?} {:?}",
                state(&client, &placed.instance_id),
                stub.events(),
                lines(&report)
            )
        },
        || state(&client, &placed.instance_id) == json!("Running"),
    );
    let events = stub.events();
    let StubEvent::Started { key: started, spec } = &events[0] else {
        panic!("the first event is no start: {events:?}");
    };
    assert_eq!(started, &key);
    assert_eq!(spec.argv, vec!["poold", "serve"]);
    assert_eq!(
        spec.env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
        vec![("POOLD_LOG", "info"), ("POOLD_SLOTS", "8")]
    );
    assert_eq!(spec.run_as.as_deref(), Some("deployer"));
    assert_eq!(
        spec.prestart,
        vec![PortPrestart::new(["mkdir", "-p", "/srv/pool"]).privileged(true)]
    );
    assert_eq!(
        spec.stop,
        StopRule::new(Duration::from_secs(2), KillScope::Group)
    );
    assert_eq!(
        spec.restart,
        RestartRule::default(),
        "the agent decides restarts itself and passes Never"
    );
    assert_eq!(
        events[1..3],
        [
            StubEvent::Prestart {
                key: key.clone(),
                step: 0,
                privileged: true
            },
            StubEvent::Command { key: key.clone() },
        ],
        "{events:?}"
    );

    // Exit 1: restarted after the delay of 1 s, and reported.
    let exited = Instant::now();
    stub.exit(&key, ProcessExit::Code(1));
    wait_until(
        "no restart",
        || format!("{:?}", stub.events()),
        || restarts(&stub.events()) == 1,
    );
    let waited = exited.elapsed();
    assert!(
        waited >= Duration::from_secs(1),
        "restarted {waited:?} after the exit, before the delay of 1 s"
    );
    wait_until(
        "no RestartProcess",
        || format!("{:?}", records.calls()),
        || !calls(records, "RestartProcess").is_empty(),
    );
    assert_eq!(
        records.calls(),
        vec![
            ("StartProcess", "Started".to_owned()),
            ("RestartProcess", "Restarted".to_owned()),
        ]
    );
    wait_until(
        "the command does not run again",
        || format!("{:?}", stub.events()),
        || commands(&stub.events()) == 2,
    );
    assert_eq!(
        stub.events()
            .iter()
            .filter(|event| matches!(event, StubEvent::Prestart { .. }))
            .count(),
        2,
        "the prestart step runs again before the command"
    );

    // Exit 2: kept down, past the delay.
    stub.exit(&key, ProcessExit::Code(2));
    wait_until(
        "no line keeps it down",
        || format!("{:?}", lines(&report)),
        || {
            lines(&report).iter().any(|line| {
                line["instance_id"] == json!(placed.instance_id) && line["kept_down"] == true
            })
        },
    );
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(restarts(&stub.events()), 1, "{:?}", stub.events());
    assert!(
        requests(&host.log_after(seq), "weft.workloads.RecordInstanceExited").is_empty(),
        "an exit kept down under Always is not recorded Exited"
    );
    assert_eq!(
        stub.observe(&key).expect("the stub holds it").state,
        WorkloadState::Exited
    );

    // A stop reaches the port with grace 2 s and kill Group.
    let ordered = client
        .command(
            "weft.nodes.Stop",
            SCHEDULER,
            json!({}),
            json!({"assignment_id": placed.assignment_id}),
        )
        .expect("the control host answers");
    assert_eq!(
        ordered.body["outcome"],
        json!("ordered"),
        "{}",
        ordered.body
    );
    wait_until(
        "no stop",
        || format!("{:?}", stub.events()),
        || !stops(&stub.events()).is_empty(),
    );
    assert_eq!(
        stops(&stub.events()),
        vec![StopRule::new(Duration::from_secs(2), KillScope::Group)]
    );
    wait_until(
        "no StopProcess",
        || format!("{:?}", records.calls()),
        || !calls(records, "StopProcess").is_empty(),
    );
    assert_eq!(calls(records, "StopProcess"), vec!["Stopped"]);
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// One prestart step running `mkdir -p /srv/pool`, privileged as `privileged` says.
fn one_step(privileged: bool) -> Option<Vec<PrestartStep>> {
    Some(vec![PrestartStep {
        command: vec!["mkdir".to_owned(), "-p".to_owned(), "/srv/pool".to_owned()],
        privileged: Some(privileged),
    }])
}

/// The report lines of `instance_id` that carry `field`.
fn with(report: &Path, instance_id: &str, field: &str) -> Vec<Value> {
    lines(report)
        .into_iter()
        .filter(|line| line["instance_id"] == json!(instance_id) && line.get(field).is_some())
        .collect()
}

/// Waits until the report holds a `pass` line numbered `passes` higher than the highest one
/// now: the agent has looked at its work list that many more times.
fn passes_later(report: &Path, passes: u64) {
    let pass = |lines: &[Value]| {
        lines
            .iter()
            .filter(|line| line.get("started").is_none())
            .filter_map(|line| line["pass"].as_u64())
            .max()
            .unwrap_or(0)
    };
    let target = pass(&lines(report)) + passes;
    wait_until(
        "the agent does not pass over its work list",
        || format!("{:?}", lines(report)),
        || pass(&lines(report)) >= target,
    );
}

/// Outcome 2: a prestart step that exits non-zero stops the start. The later steps and the
/// command do not run, the failure is one report line, and the instance never reads `Running`;
/// `RecordInstanceRunning` is never sent.
#[test]
fn a_failing_prestart_step_stops_the_start_and_the_instance_never_runs() {
    let (rows, placed) = seeds::process_on_one_node(
        HostProcess {
            prestart: one_step(false),
            ..HostProcess::default()
        },
        RestartMode::Always,
    );
    let host = ControlHost::start_seeded(rows);
    let seq = host.last_seq();
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let key = key(&placed);
    stub.fail_start(
        &key,
        StartFailure::Prestart {
            step: 0,
            exit: ProcessExit::Code(1),
        },
    );
    let (report, exec) = follow(&host, &placed.node_id, Some(stub), None, "prestart-fails");
    wait_until(
        "no failure line",
        || format!("{:?}", lines(&report)),
        || !with(&report, &placed.instance_id, "failed").is_empty(),
    );
    assert_eq!(
        with(&report, &placed.instance_id, "failed"),
        vec![json!({
            "instance_id": placed.instance_id,
            "process": "main",
            "failed": "prestart",
            "step": 0,
            "exit_code": 1,
        })]
    );
    passes_later(&report, 3);
    let events = stub.events();
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, StubEvent::Command { .. })),
        "{events:?}"
    );
    assert!(
        events.contains(&StubEvent::Prestart {
            key,
            step: 0,
            privileged: false
        }),
        "{events:?}"
    );
    assert_eq!(
        state(&host.client(), &placed.instance_id),
        json!("Starting")
    );
    assert_eq!(
        requests(&host.log_after(seq), "weft.workloads.RecordInstanceRunning").len(),
        0
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// Outcome 2: a privileged prestart step goes to the port as such; a port that cannot run one
/// refuses the start `NotServed`, which the report names, and nothing starts.
#[test]
fn a_privileged_step_on_a_port_that_cannot_run_it_is_refused_not_served() {
    let (rows, placed) = seeds::process_on_one_node(
        HostProcess {
            prestart: one_step(true),
            ..HostProcess::default()
        },
        RestartMode::Never,
    );
    let host = ControlHost::start_seeded(rows);
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::without_privileged()));
    let (report, exec) = follow(&host, &placed.node_id, Some(stub), None, "privileged");
    wait_until(
        "no refusal line",
        || format!("{:?}", lines(&report)),
        || !with(&report, &placed.instance_id, "refusal").is_empty(),
    );
    assert_eq!(
        with(&report, &placed.instance_id, "refusal"),
        vec![json!({"instance_id": placed.instance_id, "refusal": "not-served"})]
    );
    assert_eq!(stub.events(), Vec::new());
    passes_later(&report, 2);
    assert_eq!(
        state(&host.client(), &placed.instance_id),
        json!("Starting")
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// Default deny: an agent given no `workloads` port starts no process workload; its start is
/// refused `NotServed` and reported.
#[test]
fn without_a_workloads_port_a_process_instance_is_refused_not_served() {
    let (rows, placed) = seeds::process_on_one_node(HostProcess::default(), RestartMode::Never);
    let host = ControlHost::start_seeded(rows);
    let (report, exec) = follow(&host, &placed.node_id, None, None, "no-port");
    wait_until(
        "no refusal line",
        || format!("{:?}", lines(&report)),
        || !with(&report, &placed.instance_id, "refusal").is_empty(),
    );
    assert_eq!(
        with(&report, &placed.instance_id, "refusal"),
        vec![json!({"instance_id": placed.instance_id, "refusal": "not-served"})]
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// Outcome 4 under `OnFailure` with no delay: an exit 1 is restarted at once and reported, with
/// the policy's initial backoff as the wait (`RestartProcess` refuses a wait below it); an exit 0
/// keeps the process down.
#[test]
fn on_failure_restarts_an_exit_1_and_keeps_an_exit_0_down() {
    let (rows, placed) = seeds::process_on_one_node(
        HostProcess {
            restart: Some(ProcessRestart {
                policy: RestartMode::OnFailure,
                delay: Some("0s".to_owned()),
                keep_down_on_exit: None,
            }),
            ..HostProcess::default()
        },
        RestartMode::OnFailure,
    );
    let records = LocalRecords::leaked("on-failure", rows.clone());
    let host = ControlHost::start_seeded(rows);
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let key = key(&placed);
    let (report, exec) = follow(
        &host,
        &placed.node_id,
        Some(stub),
        Some(records),
        "on-failure",
    );
    wait_until(
        "the instance does not read Running",
        || format!("{:?} {:?}", stub.events(), lines(&report)),
        || state(&host.client(), &placed.instance_id) == json!("Running"),
    );

    stub.exit(&key, ProcessExit::Code(1));
    wait_until(
        "the command does not run again",
        || format!("{:?}", stub.events()),
        || commands(&stub.events()) == 2,
    );
    wait_until(
        "no restarted line",
        || format!("{:?}", lines(&report)),
        || !with(&report, &placed.instance_id, "restarted").is_empty(),
    );
    wait_until(
        "no RestartProcess",
        || format!("{:?}", records.calls()),
        || !calls(records, "RestartProcess").is_empty(),
    );
    assert_eq!(calls(records, "RestartProcess"), vec!["Restarted"]);

    stub.exit(&key, ProcessExit::Code(0));
    wait_until(
        "no line keeps it down",
        || format!("{:?}", lines(&report)),
        || {
            with(&report, &placed.instance_id, "kept_down")
                .iter()
                .any(|line| line["kept_down"] == true)
        },
    );
    passes_later(&report, 5);
    assert_eq!(restarts(&stub.events()), 1, "{:?}", stub.events());
    assert_eq!(
        with(&report, &placed.instance_id, "kept_down"),
        vec![
            json!({"instance_id": placed.instance_id, "process": "main", "exit_code": 1, "kept_down": false}),
            json!({"instance_id": placed.instance_id, "process": "main", "exit_code": 0, "kept_down": true}),
        ]
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// An exit seen through the `workloads` port goes the exec path's exit way
/// (`story:cloudspec-submission-job-exit`): a process of an instance of restart mode `Never`
/// that exits 0 is kept down, not restarted, and the instance is recorded with
/// `RecordInstanceExited` once and reads `Exited`.
#[test]
fn an_exit_0_under_never_is_kept_down_and_recorded_exited() {
    let (rows, placed) = seeds::process_on_one_node(HostProcess::default(), RestartMode::Never);
    let host = ControlHost::start_seeded(rows);
    let seq = host.last_seq();
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let key = key(&placed);
    let (report, exec) = follow(&host, &placed.node_id, Some(stub), None, "never-exit");
    wait_until(
        "the instance does not read Running",
        || format!("{:?} {:?}", stub.events(), lines(&report)),
        || state(&host.client(), &placed.instance_id) == json!("Running"),
    );
    stub.exit(&key, ProcessExit::Code(0));
    wait_until(
        "the instance does not read Exited",
        || format!("{:?} {:?}", lines(&report), host.log_after(seq)),
        || state(&host.client(), &placed.instance_id) == json!("Exited"),
    );
    passes_later(&report, 3);
    let log = host.log_after(seq);
    assert_eq!(
        requests(&log, "weft.workloads.RecordInstanceExited").len(),
        1,
        "{log:?}"
    );
    assert_eq!(restarts(&stub.events()), 0, "{:?}", stub.events());
    assert_eq!(
        with(&report, &placed.instance_id, "kept_down"),
        vec![
            json!({"instance_id": placed.instance_id, "process": "main", "exit_code": 0, "kept_down": true})
        ]
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}

/// Outcome 3 and the per-node decision: the assignment of a running process is released (its
/// lease ended, as when its node leaves the pool), and the agent stops the process through the
/// port with the workload's grace (`5m`) and kill scope (`Mixed`), and starts it nowhere else.
#[test]
fn a_released_assignment_stops_its_process_with_the_workloads_grace_and_kill_scope() {
    let (mut rows, placed) = seeds::process_on_one_node(
        HostProcess {
            stop: Some(Stop {
                grace: Some("5m".to_owned()),
                kill: Some(SpecKill::Mixed),
            }),
            ..HostProcess::default()
        },
        RestartMode::Never,
    );
    for row in &mut rows {
        if let Row::Assignment(assignment) = row {
            assignment.data.lease_renewed_at =
                weft_types::primitives::Timestamp("2026-01-01T00:00:00Z".to_owned());
        }
    }
    let records = LocalRecords::leaked("released", rows.clone());
    let host = ControlHost::start_seeded(rows);
    let client = host.client();
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let (report, exec) = follow(
        &host,
        &placed.node_id,
        Some(stub),
        Some(records),
        "released",
    );
    wait_until(
        "the instance does not read Running",
        || format!("{:?} {:?}", stub.events(), lines(&report)),
        || state(&client, &placed.instance_id) == json!("Running"),
    );
    let released = client
        .command(
            "weft.nodes.ReleaseAssignment",
            SCHEDULER,
            json!({}),
            json!({"assignment_id": placed.assignment_id}),
        )
        .expect("the control host answers");
    // The release takes effect; delivering its `AssignmentReleased` on the control host then
    // reaches `weft.node.CloseAssignmentEndpointSessions`, which no behaviour implements, so the
    // answer is `committed: true` with that refusal rather than `released`.
    assert!(
        released.body["outcome"] == json!("released") || released.body["committed"] == json!(true),
        "{}",
        released.body
    );
    wait_until(
        "no stop",
        || format!("{:?}", stub.events()),
        || !stops(&stub.events()).is_empty(),
    );
    assert_eq!(
        stops(&stub.events()),
        vec![StopRule::new(Duration::from_secs(300), KillScope::Mixed)]
    );
    wait_until(
        "no stopped line",
        || format!("{:?}", lines(&report)),
        || !with(&report, &placed.instance_id, "stopped").is_empty(),
    );
    passes_later(&report, 3);
    let started = stub
        .events()
        .iter()
        .filter(|event| matches!(event, StubEvent::Started { .. }))
        .count();
    assert_eq!(started, 1, "{:?}", stub.events());
    assert_eq!(stops(&stub.events()).len(), 1, "{:?}", stub.events());
    assert_eq!(
        records.calls(),
        vec![
            ("StartProcess", "Started".to_owned()),
            ("StopProcess", "Stopped".to_owned()),
        ]
    );
    assert_eq!(exec.calls(), 0, "a process instance reached the exec port");
}
