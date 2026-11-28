//! A Service workload's environment text values and arguments reach its instance at start, or
//! the placement is refused by name (`story:workload-environment-and-args`).
//!
//! Each case runs a control host of the test build (its controllers place the replica) and the
//! node agent's run in a thread of this process, with stub ports: an `exec` port and a
//! `node-facts` port of this file, and the in-memory `workloads` port of weft-platform. The
//! agent's thread holds its session until the test process ends.
//!
//! - With a `workloads` port the replica is started through it: the stub's start record holds
//!   the process's arguments and environment as the workload's document names them.
//! - Without one the replica runs through the `exec` port, as a snippet does: the exec request
//!   holds them.
//! - A runtime that declares it delivers no environment (or no arguments,
//!   `weft.workloads.RuntimeInputs`) makes `PlaceInstance` refuse the replica by that name, and
//!   no `PlacementDecision` exists.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_platform::local::stub::{StubEvent, StubWorkloads};
use weft_platform::node::{
    Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel, NodeFacts,
    Process, Workloads,
};
use weft_platform::Refusal;
use weft_testkit::steps::{self, input, Path};
use weft_testkit::ControlHost;

/// How long a case waits for the agent or the controllers.
const LIMIT: Duration = Duration::from_secs(30);

/// The program of the workload's one process.
const PROGRAM: &str = "/srv/site/bin/site";

/// Its two arguments.
const ARGUMENTS: [&str; 2] = ["--listen=8080", "--mode=live"];

/// Its two environment variables, with text values.
const ENVIRONMENT: [(&str, &str); 2] = [("SITE_NAME", "weft"), ("SITE_GREETING", "hello world")];

/// The process's command: the program and its two arguments.
fn argv() -> Vec<String> {
    std::iter::once(PROGRAM)
        .chain(ARGUMENTS)
        .map(str::to_owned)
        .collect()
}

/// The process's environment.
fn environment() -> BTreeMap<String, String> {
    ENVIRONMENT
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// A stub of the node: what it observes about itself (one cpu, `Container` isolation) and an
/// `exec` port that records each process and runs it to `exited` 0.
struct Node {
    execs: Mutex<Vec<Process>>,
}

impl Node {
    fn leaked() -> &'static Self {
        Box::leak(Box::new(Self {
            execs: Mutex::new(Vec::new()),
        }))
    }

    fn execs(&self) -> Vec<Process> {
        self.execs.lock().expect("the execs").clone()
    }
}

impl NodeFacts for Node {
    fn observe(&self) -> Result<Facts, Refusal> {
        Ok(Facts {
            capacity: vec![Capacity {
                resource_class: "cpu".to_owned(),
                amount: 4,
            }],
            isolation: Isolation {
                levels: vec![IsolationLevel::Container],
                cgroups_delegated: Some(true),
                egress_denied: Some(true),
                kvm_present: Some(false),
                gpu_passthrough: Some(false),
            },
        })
    }
}

impl Exec for Node {
    fn run(
        &self,
        process: &Process,
        on_state: &mut dyn FnMut(ExecState),
    ) -> Result<Exited, ExecError> {
        self.execs.lock().expect("the execs").push(process.clone());
        on_state(ExecState::Accepted);
        on_state(ExecState::Running);
        on_state(ExecState::Exited);
        Ok(Exited {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            truncated: false,
        })
    }
}

/// A report file of this test process named `name`, absent.
fn report_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "workload-inputs-{name}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// Runs the node agent of `path`'s node against `host` in a thread of its own, through `node`
/// and, when given, `workloads`.
fn start_agent(
    host: &ControlHost,
    path: &Path,
    node: &'static Node,
    workloads: Option<&'static StubWorkloads>,
    name: &str,
) {
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
        report_path(name).to_string_lossy().into_owned(),
    ])
    .expect("the arguments parse");
    let workloads = workloads.map(|stub| stub as &'static (dyn Workloads + Sync));
    std::thread::spawn(move || weft_agent::agent::run_with(&options, node, node, workloads));
}

/// The identities the definitions made.
struct Defined {
    formation_id: String,
    service_id: String,
}

/// The workload definitions of `path`'s account, as `steps::run_definitions` makes them, but
/// with the one process of [`argv`] and [`environment`] and on `runtime_id` (a new runtime of
/// `Snippet` when absent): formation `site` pinned to the default cluster, its component, a
/// revision, environment `prod`, the `Snippet` artifact, the workload shaped `Service`, its
/// assignment and its service of one replica.
fn define(client: &Client, path: &Path, runtime_id: Option<&str>) -> Defined {
    let caller = steps::path_caller(path);
    let scope_id = path.scope_id.as_str();
    let formation_id = steps::create_formation(client, &caller, scope_id);
    steps::pin_formation(client, &caller, scope_id, &path.cluster_id, &formation_id);
    let component_id = steps::add_component(client, &caller, &formation_id, scope_id);
    let revision_id = steps::commit_revision(client, &caller, &formation_id, scope_id);
    let environment_id = steps::create_environment(client, &caller, &formation_id, scope_id);
    steps::register_artifact(client, &caller, scope_id);
    let runtime_id = runtime_id.map_or_else(
        || steps::register_runtime(client, &["Snippet"]),
        str::to_owned,
    );
    let of = input::WorkloadOf {
        scope_id,
        formation_id: &formation_id,
        formation_revision_id: &revision_id,
        environment_id: &environment_id,
        runtime_id: &runtime_id,
    };
    let mut workload = input::define_workload(of);
    workload["processes"] = json!([{
        "name": "main",
        "role": "Main",
        "command": argv(),
        "environment": environment(),
        "checks": [],
    }]);
    let workload_id = steps::define_workload(client, &caller, workload);
    steps::assign_workload(
        client,
        &caller,
        input::assign_workload(of, &component_id, &workload_id),
    );
    let service_id = steps::define_service(client, &caller, &workload_id, scope_id);
    Defined {
        formation_id,
        service_id,
    }
}

/// The rows of `view`.
fn rows(client: &Client, view: &str) -> Vec<Value> {
    client.view(view, &[]).ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .clone()
}

/// The row of `weft.workloads.Instances` of the service `service_id`, once there is one.
fn instance_of(client: &Client, service_id: &str) -> Option<Value> {
    rows(client, "weft.workloads.Instances")
        .into_iter()
        .find(|row| row["service_id"] == json!(service_id))
}

/// Waits until `done` answers, at most [`LIMIT`]; `what` says what was waited for.
fn wait<T>(what: &str, mut done: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = done() {
            return value;
        }
        assert!(start.elapsed() < LIMIT, "{what} after {LIMIT:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Statement 1, through the `workloads` port: the replica's instance reads `Running`, and the
/// stub port's one start record holds exactly the process's two arguments after its program and
/// its two environment variables. Nothing ran through the `exec` port.
#[test]
fn a_service_replica_starts_through_the_workloads_port_with_its_environment_and_arguments() {
    let host = ControlHost::start();
    let client = host.client();
    let path = steps::run_path(&client);
    let node = Node::leaked();
    let workloads: &'static StubWorkloads = Box::leak(Box::new(StubWorkloads::new()));
    start_agent(&host, &path, node, Some(workloads), "port");
    let defined = define(&client, &path, None);

    let row = wait("the replica's instance does not read Running", || {
        instance_of(&client, &defined.service_id).filter(|row| row["state"] == json!("Running"))
    });
    let started: Vec<_> = workloads
        .events()
        .into_iter()
        .filter_map(|event| match event {
            StubEvent::Started { key, spec } => Some((key, spec)),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 1, "{:?}", workloads.events());
    let (key, spec) = &started[0];
    assert_eq!(key.process, "main", "{key:?}");
    assert_eq!(spec.argv, argv(), "{spec:?}");
    assert_eq!(spec.env, environment(), "{spec:?}");
    assert_eq!(node.execs(), Vec::<Process>::new(), "{row}");
}

/// Statement 1, through the `exec` port: without a `workloads` port the replica runs as a
/// snippet does, and the exec request holds exactly the process's command and environment.
#[test]
fn without_a_workloads_port_the_replica_s_exec_request_holds_its_environment_and_arguments() {
    let host = ControlHost::start();
    let client = host.client();
    let path = steps::run_path(&client);
    let node = Node::leaked();
    start_agent(&host, &path, node, None, "exec");
    let defined = define(&client, &path, None);

    wait("the replica's instance does not read Running", || {
        instance_of(&client, &defined.service_id).filter(|row| row["state"] == json!("Running"))
    });
    let execs = node.execs();
    assert_eq!(execs.len(), 1, "{execs:?}");
    assert_eq!(execs[0].argv, argv(), "{:?}", execs[0]);
    assert_eq!(execs[0].env, environment(), "{:?}", execs[0]);
}

/// The rows a control host is seeded with: a runtime `snippet` of `Snippet` and its declared
/// inputs: it delivers environment variables when `environment`, arguments when `arguments`.
fn runtime_rows(runtime_id: &str, environment: bool, arguments: bool) -> Vec<weft_impl::Row> {
    use weft_types::primitives::Uuid;
    use weft_types::shared::RuntimeId;
    use weft_types::workloads::{
        PackageKind, RuntimeData, RuntimeInputsData, RuntimeInputsSnapshot, RuntimeInputsState,
        RuntimeSnapshot, RuntimeState,
    };
    let id = RuntimeId(Uuid(runtime_id.to_owned()));
    vec![
        weft_impl::Row::Runtime(RuntimeSnapshot {
            state: RuntimeState::Registered,
            data: RuntimeData {
                runtime_id: id.clone(),
                name: "snippet".to_owned(),
                version: "1".to_owned(),
                package_kinds: vec![PackageKind::Snippet],
                minimum_isolation: weft_types::shared::IsolationLevel::Container,
            },
        }),
        weft_impl::Row::RuntimeInputs(RuntimeInputsSnapshot {
            state: RuntimeInputsState::Declared,
            data: RuntimeInputsData {
                runtime_id: id,
                environment,
                arguments,
            },
        }),
    ]
}

/// The answer of `PlaceInstance` as the Scheduler for the pending request of the replica of
/// `defined` on `path`'s node at `Container`, once the node reads `Ready` with an open session:
/// sent again while the answer is a refusal of the node not being ready yet.
fn place(client: &Client, path: &Path, defined: &Defined) -> Value {
    let instance_id = wait("the replica has no instance", || {
        instance_of(client, &defined.service_id)
            .and_then(|row| row["instance_id"].as_str().map(str::to_owned))
    });
    let request_id = wait("the replica's instance has no pending request", || {
        rows(client, "weft.scheduling.PendingRequests")
            .into_iter()
            .find(|row| row["instance_id"] == json!(instance_id))
            .and_then(|row| row["request_id"].as_str().map(str::to_owned))
    });
    let node = wait("the node is not listed", || {
        rows(client, "weft.infrastructure.Nodes")
            .into_iter()
            .find(|row| row["node_id"] == json!(path.node.node_id))
    });
    let start = Instant::now();
    loop {
        let decided_at = (time::OffsetDateTime::now_utc() - time::Duration::seconds(1))
            .replace_nanosecond(0)
            .expect("zero nanoseconds is valid")
            .format(&time::format_description::well_known::Rfc3339)
            .expect("UTC time formats");
        let answer = client
            .command(
                "weft.scheduling.PlaceInstance",
                "weft.actors.Scheduler",
                json!({}),
                json!({
                    "request_id": request_id,
                    "tenant_id": path.tenant_id,
                    "scope_id": path.scope_id,
                    "cluster_id": path.cluster_id,
                    "formation_id": defined.formation_id,
                    "node_id": path.node.node_id,
                    "node_pool_id": node["node_pool_id"],
                    "zone": node["zone"],
                    "isolation": "Container",
                    "bindings": [],
                    "cluster_node_pool_id": null,
                    "topology_group_id": null,
                    "peer_groups": [],
                    "decided_at": decided_at,
                }),
            )
            .expect("the control host answers");
        let waiting = matches!(
            answer.body["outcome"].as_str(),
            Some("node-not-schedulable" | "node-not-connected" | "isolation-not-offered")
        );
        if !waiting {
            return answer.body;
        }
        assert!(
            start.elapsed() < LIMIT,
            "the node is not ready for a placement after {LIMIT:?}: {}",
            answer.body
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The `PlacementDecision` rows of the replica of `defined`; none before it has an instance.
fn decisions_of(client: &Client, defined: &Defined) -> Vec<Value> {
    let Some(instance_id) = instance_of(client, &defined.service_id)
        .and_then(|row| row["instance_id"].as_str().map(str::to_owned))
    else {
        return Vec::new();
    };
    rows(client, "weft.scheduling.PlacementDecisions")
        .into_iter()
        .filter(|row| row["instance_id"] == json!(instance_id))
        .collect()
}

/// Statement 2: the workload's runtime declares that it delivers no environment, so
/// `PlaceInstance` refuses the replica `environment-not-delivered`
/// (`weft.scheduling.EnvironmentNotDelivered`), and no `PlacementDecision` of it exists.
#[test]
fn a_placement_whose_runtime_delivers_no_environment_is_refused_naming_environment() {
    let runtime_id = weft_impl::context::mint();
    let host = ControlHost::start_seeded(runtime_rows(&runtime_id, false, true));
    let client = host.client();
    let path = steps::run_path(&client);
    start_agent(&host, &path, Node::leaked(), None, "no-environment");
    let defined = define(&client, &path, Some(&runtime_id));

    let body = place(&client, &path, &defined);
    assert_eq!(
        (body["outcome"].clone(), body["error"].clone()),
        (
            json!("environment-not-delivered"),
            json!("weft.scheduling.EnvironmentNotDelivered")
        ),
        "{body}"
    );
    assert_eq!(decisions_of(&client, &defined), Vec::<Value>::new());
}

/// Outcome 2, the other input: a runtime that delivers environment variables and no arguments
/// refuses the replica `arguments-not-delivered`, and no `PlacementDecision` of it exists.
#[test]
fn a_placement_whose_runtime_delivers_no_arguments_is_refused_naming_arguments() {
    let runtime_id = weft_impl::context::mint();
    let host = ControlHost::start_seeded(runtime_rows(&runtime_id, true, false));
    let client = host.client();
    let path = steps::run_path(&client);
    start_agent(&host, &path, Node::leaked(), None, "no-arguments");
    let defined = define(&client, &path, Some(&runtime_id));

    let body = place(&client, &path, &defined);
    assert_eq!(
        (body["outcome"].clone(), body["error"].clone()),
        (
            json!("arguments-not-delivered"),
            json!("weft.scheduling.ArgumentsNotDelivered")
        ),
        "{body}"
    );
    assert_eq!(decisions_of(&client, &defined), Vec::<Value>::new());
}

/// A runtime that declares it delivers both places the same replica: the refusals answer only
/// what the runtime declares it does not deliver.
#[test]
fn a_placement_whose_runtime_delivers_both_is_placed() {
    let runtime_id = weft_impl::context::mint();
    let host = ControlHost::start_seeded(runtime_rows(&runtime_id, true, true));
    let client = host.client();
    let path = steps::run_path(&client);
    start_agent(&host, &path, Node::leaked(), None, "both");
    let defined = define(&client, &path, Some(&runtime_id));

    wait("the replica has no PlacementDecision", || {
        (!decisions_of(&client, &defined).is_empty()).then_some(())
    });
}
