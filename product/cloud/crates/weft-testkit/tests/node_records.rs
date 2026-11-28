//! The control host keeps the node's own records (`story:node-records-on-control-host`, owner
//! decision D-N1 = A): `weft.node` sits with weft-control, so `StartProcess`, `RestartProcess` and
//! `StopProcess` are routes of the control host like `RecordInstanceRunning`, and the node agent's
//! run sends them there through its production records ([`weft_agent::records::ControlRecords`]).
//! One store, no relay: the events of these commands are lines of the control host's log and
//! their rows are rows of its state file.
//!
//! The case runs a control host of the test build and the node agent's run
//! (`weft_agent::agent::run_with`) in a thread of this process, with a node-facts port of this
//! file, an `exec` port that runs nothing and the in-memory `workloads` port of weft-platform. A
//! process part is submitted, planned and applied; its instance is placed on the agent's node by
//! seed (placement of a `Daemon` is `story:slot-node-pool-placement`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_impl::context::{mint, timestamp};
use weft_impl::{Ports, Row};
use weft_platform::local::stub::{StubEvent, StubWorkloads};
use weft_platform::node::{
    Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation,
    IsolationLevel as PortIsolation, NodeFacts, Process, ProcessExit, WorkloadKey, Workloads,
};
use weft_platform::Refusal;
use weft_testkit::cloudspec;
use weft_testkit::steps::{self, NODE_AGENT};
use weft_testkit::ControlHost;
use weft_types::behaviour::{InstanceStorage, WorkloadStorage};
use weft_types::nodes::{AssignmentData, AssignmentSnapshot, AssignmentState, DesiredState};
use weft_types::primitives::Uuid;
use weft_types::shared::{
    ArtifactDigest, AssignmentId, InstanceId, IsolationLevel, NodeId, NodeSessionId,
    PlacementDecisionId, ProcessName, ScopeId, WorkloadId,
};
use weft_types::workloads::obligations::RecordProcessRestartedBehavior;
use weft_types::workloads::{
    DesiredRun, InstanceData, InstanceHealth, InstanceSnapshot, InstanceState,
    Process as SpecProcess, ProcessRole, RecordProcessRestarted, RecordProcessRestartedOutcome,
    RestartMode, RestartPolicy, RestartReason,
};

/// How long the case waits for the agent or the control host.
const LIMIT: Duration = Duration::from_secs(30);

/// A node that observes one cpu and `Container` isolation, and whose `exec` port runs nothing: a
/// process instance never reaches it.
struct Node;

impl NodeFacts for Node {
    fn observe(&self) -> Result<Facts, Refusal> {
        Ok(Facts {
            capacity: vec![Capacity {
                resource_class: "cpu".to_owned(),
                amount: 4,
            }],
            isolation: Isolation {
                levels: vec![PortIsolation::Container],
                cgroups_delegated: Some(true),
                egress_denied: Some(true),
                kvm_present: Some(false),
                gpu_passthrough: Some(false),
            },
        })
    }
}

impl Exec for Node {
    fn run(&self, _: &Process, _: &mut dyn FnMut(ExecState)) -> Result<Exited, ExecError> {
        Err(Refusal::NotServed {
            operation: "exec in a process test".to_owned(),
        }
        .into())
    }
}

/// A report file of this test process named `name`, absent.
fn report_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("node-records-{name}-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// The complete lines of the report file at `path`.
fn lines(path: &std::path::Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    complete
        .lines()
        .map(|line| serde_json::from_str(line).expect("a report line"))
        .collect()
}

/// Waits until `done` answers, at most [`LIMIT`]; `what` and `state` say what was missing.
fn wait<T>(what: &str, state: impl Fn() -> String, mut done: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = done() {
            return value;
        }
        assert!(
            start.elapsed() < LIMIT,
            "{what} after {LIMIT:?}: {}",
            state()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The `state` of `instance_id` in `weft.workloads.Instances`.
fn state(client: &Client, instance_id: &str) -> Value {
    client.view("weft.workloads.Instances", &[]).ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["instance_id"] == json!(instance_id))
        .map_or(Value::Null, |row| row["state"].clone())
}

/// The `Scheduled` instance of `workload_id` (read from the state file at `state`) on `node_id`
/// with an `Active` assignment, as `CreateInstance`, `PlaceInstance` and `Assign` would leave
/// them: the rows and the identities of the instance and the assignment.
fn placed(state: &std::path::Path, workload_id: &str, node_id: &str) -> (Vec<Row>, String, String) {
    let workload = {
        let ports = Ports::open(state).expect("the state file opens");
        WorkloadStorage::get(&ports, &WorkloadId(Uuid(workload_id.to_owned())))
            .expect("the applied workload")
            .data
    };
    let now = timestamp(time::OffsetDateTime::now_utc());
    let (instance_id, assignment_id) = (mint(), mint());
    let node_id = NodeId(Uuid(node_id.to_owned()));
    let rows = vec![
        Row::Instance(InstanceSnapshot {
            state: InstanceState::Scheduled,
            data: InstanceData {
                instance_id: InstanceId(Uuid(instance_id.clone())),
                workload_id: workload.workload_id.clone(),
                scope_id: workload.scope_id.clone(),
                scope_node_id: None,
                artifact_digest: workload.artifact_digest.clone(),
                minimum_isolation: workload.isolation,
                isolation: Some(IsolationLevel::None),
                node_id: Some(node_id.clone()),
                job_id: None,
                task_index: None,
                invocation_id: None,
                box_id: None,
                session_id: None,
                replica_index: None,
                service_id: None,
                replica_counted_out: false,
                warm_pool_id: None,
                restored_from: None,
                desired: DesiredRun::Run,
                ready: false,
                health: InstanceHealth::Unknown,
                processes: workload.processes.clone(),
                restart_policy: workload.restart_policy.clone(),
                restarts: 0,
                restarts_in_window: 0,
                backoff_seconds: None,
                exit_code: None,
                failure_reason: None,
                eviction_reason: None,
            },
        }),
        Row::Assignment(AssignmentSnapshot {
            state: AssignmentState::Active,
            data: AssignmentData {
                assignment_id: AssignmentId(Uuid(assignment_id.clone())),
                instance_id: InstanceId(Uuid(instance_id.clone())),
                node_id,
                session_id: NodeSessionId(Uuid(mint())),
                placement_decision_id: PlacementDecisionId(Uuid(mint())),
                min_isolation: IsolationLevel::None,
                desired: DesiredState::Run,
                snapshot_id: None,
                assigned_at: now.clone(),
                lease_renewed_at: now,
                ended_at: None,
            },
        }),
    ];
    (rows, instance_id, assignment_id)
}

/// The log lines of `kind` whose `member` is `value`.
fn of<'a>(log: &'a [Value], kind: &str, member: &str, value: &str) -> Vec<&'a Value> {
    log.iter()
        .filter(|line| line["kind"] == kind && line[member] == value)
        .collect()
}

/// Acceptance 1 of `story:node-records-on-control-host`. A process part (restart `always`, delay
/// 1 s) runs through the stub `workloads` port under the node agent's run; its exit 1 is restarted
/// by the agent and recorded with `weft.node.RestartProcess` on the control host:
/// `weft.node.ProcessRestarted` is a line of the control host's log, and its `LocalProcess` row
/// reads one restart for the reason `Exited` in `weft.node.LocalProcessesOfInstance`. Binding
/// r9-record-process-restarted records it on the instance in the same request
/// (`weft.workloads.RecordProcessRestarted`): `RestartProcess` answers 2xx, and the instance's row
/// reads one restart, one in the window and health `Starting`. The agent's report holds no
/// refused record and no `404`, and the control host answered no request `404`.
#[test]
fn the_control_host_records_the_restart_the_node_agent_makes() {
    let mut host = ControlHost::start();
    let client = host.client();
    let path = steps::run_path(&client);
    let mut caller = steps::path_caller(&path);
    caller["principal_id"] = json!(mint());
    let formation_id = steps::create_formation(&client, &caller, &path.scope_id);
    let environment_id = steps::create_environment(&client, &caller, &formation_id, &path.scope_id);
    cloudspec::register_process_artifact(&client, &caller, &path.scope_id);
    cloudspec::register_process_runtime(&client);
    let source_id = cloudspec::submit(
        &client,
        &caller,
        &formation_id,
        &cloudspec::submission_with_part("daemon", &cloudspec::process_part()),
    );
    let plan_id = cloudspec::plan(
        &client,
        &caller,
        &source_id,
        &path.scope_id,
        &environment_id,
    );
    cloudspec::ready(&client, &caller, &plan_id);
    let applied = cloudspec::apply(&client, &caller, &plan_id);
    assert_eq!(applied.status, 202, "{}", applied.body);
    let workload_id = applied.body["parts"][0]["workload_id"]
        .as_str()
        .expect("the applied part's workload")
        .to_owned();

    host.stop();
    let (rows, instance_id, assignment_id) =
        placed(&host.state_path(), &workload_id, &path.node.node_id);
    weft_testkit::seed(&host.state_path(), rows);
    let host = host.restart();
    let client = host.client();
    let seq = host.last_seq();

    let report = report_path("restart");
    let options = weft_agent::agent::Options::parse([
        "--control".to_owned(),
        format!("http://{}", host.address()),
        "--node-id".to_owned(),
        path.node.node_id.clone(),
        "--join-token".to_owned(),
        path.node.join_token.clone(),
        "--substrate".to_owned(),
        "/nonexistent/substrate.sock".to_owned(),
        "--report".to_owned(),
        report.to_string_lossy().into_owned(),
    ])
    .expect("the arguments parse");
    let stub: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    let workloads: &'static (dyn Workloads + Sync) = stub;
    std::thread::spawn(move || {
        weft_agent::agent::run_with(&options, &Node, &Node, Some(workloads))
    });

    wait(
        "the instance does not read Running",
        || format!("{} {:?}", state(&client, &instance_id), lines(&report)),
        || (state(&client, &instance_id) == json!("Running")).then_some(()),
    );
    let key = WorkloadKey::new(assignment_id, "main");
    stub.exit(&key, ProcessExit::Code(1));
    wait(
        "the agent does not restart the process",
        || format!("{:?}", stub.events()),
        || {
            stub.events()
                .iter()
                .any(|event| matches!(event, StubEvent::Restarting { .. }))
                .then_some(())
        },
    );
    let log = wait(
        "no weft.node.ProcessRestarted in the control host's log",
        || format!("{:?} {:?}", host.log_after(seq), lines(&report)),
        || {
            let log = host.log_after(seq);
            (!of(&log, "event", "name", "weft.node.ProcessRestarted").is_empty()).then_some(log)
        },
    );

    for (command, event) in [
        ("weft.node.StartProcess", "weft.node.LocalProcessStarted"),
        ("weft.node.RestartProcess", "weft.node.ProcessRestarted"),
    ] {
        let requests = of(&log, "request", "operation", command);
        assert_eq!(requests.len(), 1, "{command}: {log:?}");
        assert_eq!(requests[0]["actor"], json!(NODE_AGENT), "{command}");
        assert_eq!(
            of(&log, "event", "name", event).len(),
            1,
            "{event}: {log:?}"
        );
    }
    assert!(
        log.iter()
            .all(|line| line["kind"] != "request" || line["status"] != 404),
        "a request answered 404: {log:?}"
    );
    // The restart reaches the instance through binding r9-record-process-restarted in the same
    // request: `RestartProcess` answers 2xx, not the `501` of an unmet obligation.
    let restart = of(&log, "request", "operation", "weft.node.RestartProcess")[0];
    assert!(
        restart["status"]
            .as_u64()
            .is_some_and(|status| (200..300).contains(&status)),
        "RestartProcess answered {}: {log:?}",
        restart["status"]
    );
    // The lines of that request: its own event, then the two of the binding's command. (The
    // instance's first `InstanceHealthStarting` came with `RecordArtifactVerified`, earlier.)
    let following: Vec<&str> = log
        .iter()
        .skip_while(|line| line["seq"] != restart["seq"])
        .skip(1)
        .take_while(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str())
        .collect();
    assert_eq!(
        following,
        vec![
            "weft.node.ProcessRestarted",
            "weft.workloads.ProcessRestarted",
            "weft.workloads.InstanceHealthStarting",
        ],
        "{log:?}"
    );
    let instance = client.view("weft.workloads.Instances", &[]).ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["instance_id"] == json!(instance_id))
        .cloned()
        .expect("the instance's row");
    assert_eq!(instance["restarts"], json!(1), "{instance}");
    assert_eq!(instance["restarts_in_window"], json!(1), "{instance}");
    assert_eq!(instance["health"], json!("Starting"), "{instance}");

    let processes = client
        .view_as(
            "weft.node.LocalProcessesOfInstance",
            &[("instance_id", &instance_id)],
            NODE_AGENT,
            json!({}),
        )
        .ok_body(200);
    let processes = processes["rows"].as_array().expect("rows");
    assert_eq!(processes.len(), 1, "{processes:?}");
    assert_eq!(processes[0]["node_id"], json!(path.node.node_id));
    assert_eq!(processes[0]["process"], json!("main"));
    assert_eq!(processes[0]["restarts"], json!(1), "{processes:?}");
    assert_eq!(
        processes[0]["last_reason"],
        json!("Exited"),
        "{processes:?}"
    );
    assert_eq!(processes[0]["state"], json!("Running"), "{processes:?}");

    let report = lines(&report);
    let refused: Vec<&Value> = report
        .iter()
        .filter(|line| line.get("record").is_some() || line["status"] == 404)
        .collect();
    assert!(
        refused.is_empty(),
        "refused records in the report: {refused:?}"
    );
    assert!(
        report
            .iter()
            .any(|line| line["instance_id"] == json!(instance_id) && line["restarted"] == 1),
        "the report holds no restart: {report:?}"
    );
}

/// An instance in `state` on `node_id`, of one `Main` process `main`, under the restart policy
/// `mode` (initial backoff 1 s, at most 60 s, limit 2 in the window), health `Ready`.
fn instance_row(instance_id: &str, node_id: &str, state: InstanceState, mode: RestartMode) -> Row {
    Row::Instance(InstanceSnapshot {
        state,
        data: InstanceData {
            instance_id: InstanceId(Uuid(instance_id.to_owned())),
            workload_id: WorkloadId(Uuid(mint())),
            scope_id: ScopeId(Uuid(mint())),
            scope_node_id: None,
            artifact_digest: ArtifactDigest("sha256:weft-skeleton-process".to_owned()),
            minimum_isolation: IsolationLevel::None,
            isolation: Some(IsolationLevel::None),
            node_id: Some(NodeId(Uuid(node_id.to_owned()))),
            job_id: None,
            task_index: None,
            invocation_id: None,
            box_id: None,
            session_id: None,
            replica_index: None,
            service_id: None,
            replica_counted_out: false,
            warm_pool_id: None,
            restored_from: None,
            desired: DesiredRun::Run,
            ready: true,
            health: InstanceHealth::Ready,
            processes: vec![SpecProcess {
                name: ProcessName("main".to_owned()),
                role: ProcessRole::Main,
                artifact_digest: None,
                command: vec!["poold".to_owned()],
                environment: std::collections::BTreeMap::new(),
                checks: Vec::new(),
            }],
            restart_policy: RestartPolicy {
                mode,
                backoff_initial_seconds: 1,
                backoff_max_seconds: 60,
                backoff_limit: 2,
                backoff_window_seconds: 600,
            },
            restarts: 0,
            restarts_in_window: 0,
            backoff_seconds: None,
            exit_code: None,
            failure_reason: None,
            eviction_reason: None,
        },
    })
}

/// `weft.workloads.RecordProcessRestarted`, the command of binding r9-record-process-restarted,
/// answers each outcome of its contract in its selection precedence, on rows written through the
/// storage port: the input refusal, the unknown instance, the refusals on the row, then
/// `restarted` (health `Starting`, the larger count kept on a repeat), `backing-off` past the
/// limit of 2 in the window, `restarted-backing-off` after it, and `other-state` for an instance
/// that is not `Starting` or `Running`.
#[test]
fn record_process_restarted_answers_every_outcome_of_its_contract_in_order() {
    let dir = weft_testkit::TempDir::new();
    let state = dir.path().join("state.db");
    let node_id = mint();
    let (always, never, scheduled) = (mint(), mint(), mint());
    weft_testkit::seed(
        &state,
        vec![
            instance_row(
                &always,
                &node_id,
                InstanceState::Running,
                RestartMode::Always,
            ),
            instance_row(&never, &node_id, InstanceState::Running, RestartMode::Never),
            instance_row(
                &scheduled,
                &node_id,
                InstanceState::Scheduled,
                RestartMode::Always,
            ),
        ],
    );
    let mut ports = Ports::open(&state).expect("the state file opens");
    let mut record = |instance_id: &str, node: &str, process: &str, counts: [i64; 3]| {
        let outcome = ports
            .record_process_restarted(RecordProcessRestarted {
                instance_id: InstanceId(Uuid(instance_id.to_owned())),
                node_id: NodeId(Uuid(node.to_owned())),
                process: ProcessName(process.to_owned()),
                restarts: counts[0],
                reason: RestartReason::Exited,
                restarts_in_window: counts[1],
                backoff_seconds: counts[2],
            })
            .expect("no unmet obligation");
        let name = format!("{outcome:?}");
        let name = name.split([' ', '{']).next().unwrap_or_default().to_owned();
        (name, outcome)
    };
    let other_node = mint();
    for (instance_id, node, process, counts, want) in [
        (&always, &node_id, "main", [1, 0, 1], "InvalidCount"),
        (&always, &node_id, "main", [0, 1, 1], "InvalidCount"),
        (&mint(), &node_id, "main", [1, 1, 1], "NoSuchInstance"),
        (&always, &other_node, "main", [1, 1, 1], "WrongNode"),
        (&always, &node_id, "sidecar", [1, 1, 1], "NoSuchProcess"),
        (&never, &node_id, "main", [1, 1, 1], "RestartNever"),
        (&always, &node_id, "main", [1, 2, 1], "CountAboveTotal"),
        (&always, &node_id, "main", [1, 1, 0], "BackoffOutOfRange"),
        (&always, &node_id, "main", [1, 1, 61], "BackoffOutOfRange"),
        (&scheduled, &node_id, "main", [1, 1, 1], "OtherState"),
    ] {
        let (got, outcome) = record(instance_id, node, process, counts);
        assert_eq!(got, want, "{counts:?}: {outcome:?}");
    }
    let read = |ports: &Ports| {
        InstanceStorage::get(ports, &InstanceId(Uuid(always.clone())))
            .expect("the instance")
            .data
    };

    let (got, outcome) = record(&always, &node_id, "main", [2, 1, 1]);
    assert_eq!(got, "Restarted", "{outcome:?}");
    let RecordProcessRestartedOutcome::Restarted {
        process_restarted, ..
    } = outcome
    else {
        unreachable!()
    };
    assert_eq!(process_restarted.restarts, 2);
    // A repeated or late delivery with a smaller count keeps the larger one.
    let (got, outcome) = record(&always, &node_id, "main", [1, 1, 2]);
    assert_eq!(got, "Restarted", "{outcome:?}");
    let (got, outcome) = record(&always, &node_id, "main", [3, 3, 4]);
    assert_eq!(got, "BackingOff", "{outcome:?}");
    let (got, outcome) = record(&always, &node_id, "main", [4, 3, 8]);
    assert_eq!(got, "RestartedBackingOff", "{outcome:?}");
    let instance = read(&ports);
    assert_eq!(
        (
            instance.restarts,
            instance.restarts_in_window,
            instance.backoff_seconds,
            instance.health,
            instance.ready,
        ),
        (4, 3, Some(8), InstanceHealth::BackingOff, false)
    );
}
