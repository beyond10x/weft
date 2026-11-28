//! The `snippet` golden runs to a finished Job on the local node through the cloudspec path
//! (`story:cloudspec-submission-snippet-run`, outcome 3 of `epic:cloudspec-submission`): the
//! control host binary beside `weft-agent`, the operator and tenant path, the node joined, a
//! formation pinned to the cluster with one environment, the runtime `snippet` and the snippet
//! artifact; then `POST /cloudspec/submissions` with the committed golden, `PlanFromSource`,
//! `RecordPlanResult` with no violation, `POST /cloudspec/plans/<plan_id>/apply`, and a wait for
//! the Job. Finished is the Job `Succeeded` (D-C2 = A). No network, no cloudspec binary: the
//! golden is the committed copy under `tests/fixtures/cloudspec/`.
//!
//! The skeleton test (`main.rs`, module `skeleton`) keeps its path without plan or release.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_impl::context::mint;
use weft_testkit::steps::{self, DEPLOYER};
use weft_testkit::{ControlHost, NodeAgent};

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";

/// How long the path may take from its first command to the Job reading `Succeeded`.
const RUN_LIMIT: Duration = Duration::from_secs(30);

/// The event lines of the path, in their order (short names).
const EVENTS: &[&str] = &[
    "SourceSubmitted",
    "PlanCreated",
    "PlanSimulated",
    "ReleaseCreated",
    "WorkloadDefined",
    "WorkloadAssigned",
    "ReleaseStepStarted",
    "JobSubmitted",
    "JobStarted",
    "InstanceCreated",
    "InstanceExited",
    "TaskSucceeded",
    "JobSucceeded",
];

/// The operations that each have a 2xx request line: the submission route's command,
/// `PlanFromSource`, `RecordPlanResult`, the apply route's `ApplyPlan`, and the node's reports.
const REQUESTS: &[&str] = &[
    "weft.deployments.SubmitDeploymentSource",
    "weft.deployments.PlanFromSource",
    "weft.deployments.RecordPlanResult",
    "weft.deployments.ApplyPlan",
    "weft.workloads.RecordArtifactVerified",
    "weft.workloads.RecordInstanceRunning",
    "weft.workloads.RecordInstanceExited",
];

/// The bindings that each have an `applied` line.
const APPLIED: &[&str] = &[
    "r1-rollout-submit-job",
    "job-start-on-submit",
    "job-task-instances",
    "job-instance-scheduling-request",
    "r4-bind-placement",
    "r4-assign",
    "r4-schedule-instance",
    "r4-record-placement-assigned",
    "job-task-succeeded-on-exit",
    "job-complete-on-task",
];

/// The bytes of the committed `snippet` golden.
fn golden() -> Vec<u8> {
    let path = weft_testkit::repo_root().join("tests/fixtures/cloudspec/snippet.submission.json");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The text member `field` of the first published event's payload.
fn payload(body: &Value, field: &str) -> String {
    body["published"][0]["payload"][field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {body}"))
        .to_owned()
}

/// The rows of the view `view`.
fn rows(client: &Client, view: &str) -> Vec<Value> {
    client.view(view, &[]).ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .clone()
}

/// The short names (after the last `.`) of the event lines of `log`.
fn events(log: &[Value]) -> Vec<String> {
    log.iter()
        .filter(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str())
        .map(|name| name.rsplit('.').next().unwrap_or(name).to_owned())
        .collect()
}

/// Whether `wanted` appear in `names` in this order (others between them).
fn in_order(names: &[String], wanted: &[&str]) -> bool {
    let mut rest = names.iter();
    wanted.iter().all(|name| rest.any(|seen| seen == name))
}

/// Statements 1 to 5 of the story's acceptance: the committed `snippet` golden, submitted,
/// planned with `PlanFromSource`, made `Ready` and applied, reads one Job `Succeeded` with its
/// instance `Exited` 0 on the local node within 30 s of the first command.
#[test]
fn the_snippet_golden_submitted_planned_and_applied_runs_to_a_succeeded_job() {
    let host = ControlHost::start();
    let client = host.client();
    let before = host.last_seq();

    let first_command = Instant::now();
    let path = steps::run_path(&client);
    let mut agent = NodeAgent::start(&host, &path.node.node_id, &path.node.join_token);
    let mut caller = steps::path_caller(&path);
    caller["principal_id"] = json!(mint());
    let scope_id = path.scope_id.as_str();
    let formation_id = steps::create_formation(&client, &caller, scope_id);
    steps::pin_formation(&client, &caller, scope_id, &path.cluster_id, &formation_id);
    let environment_id = steps::create_environment(&client, &caller, &formation_id, scope_id);
    steps::register_runtime(&client, &["Snippet"]);
    steps::register_artifact(&client, &caller, scope_id);

    let submitted = client
        .post_bytes_as(
            &format!("/cloudspec/submissions?formation_id={formation_id}"),
            &golden(),
            DEPLOYER,
            caller.clone(),
        )
        .expect("the control host answers");
    assert_eq!(submitted.status, 202, "{}", submitted.body);
    let source_id = payload(&submitted.body, "source_id");

    let planned = client
        .command(
            "weft.deployments.PlanFromSource",
            DEPLOYER,
            caller.clone(),
            json!({"source_id": source_id, "scope_id": scope_id, "environment_id": environment_id}),
        )
        .expect("the control host answers");
    assert_eq!(
        planned.body["outcome"],
        json!("planned"),
        "{}",
        planned.body
    );
    let plan_id = payload(&planned.body, "plan_id");

    let passed = client
        .command(
            "weft.deployments.RecordPlanResult",
            RELEASE_CONTROLLER,
            caller.clone(),
            json!({
                "plan_id": plan_id,
                "live_release_id": null,
                "diff": {"added": ["hello"], "changed": [], "removed": []},
                "simulation": {"reachability": [], "violations": []},
            }),
        )
        .expect("the control host answers");
    assert_eq!(passed.body["outcome"], json!("passed"), "{}", passed.body);

    let applied = client
        .post_bytes_as(
            &format!("/cloudspec/plans/{plan_id}/apply"),
            json!({"strategy": "Replace"}).to_string().as_bytes(),
            DEPLOYER,
            caller.clone(),
        )
        .expect("the control host answers");
    assert_eq!(applied.status, 202, "{}", applied.body);

    // 1. Within 30 s of the first command the Job reads Succeeded and its instance Exited.
    let (jobs, instances) = loop {
        let jobs = rows(&client, "weft.workloads.Jobs");
        let instances = rows(&client, "weft.workloads.Instances");
        if jobs.iter().any(|job| job["state"] == json!("Succeeded"))
            && instances.iter().any(|row| row["state"] == json!("Exited"))
        {
            break (jobs, instances);
        }
        assert!(
            first_command.elapsed() < RUN_LIMIT,
            "no Succeeded Job and Exited instance {RUN_LIMIT:?} after the first command: jobs \
             {jobs:?}; instances {instances:?}; agent report: {:?}; log: {:?}",
            agent.report(),
            host.log_after(before)
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let elapsed = first_command.elapsed();
    eprintln!("cloudspec snippet run: the Job read Succeeded {elapsed:?} after the first command");

    assert_eq!(jobs.len(), 1, "one Job: {jobs:?}");
    assert_eq!(
        (jobs[0]["state"].clone(), jobs[0]["succeeded_tasks"].clone()),
        (json!("Succeeded"), json!(1)),
        "{jobs:?}"
    );
    assert_eq!(instances.len(), 1, "one instance: {instances:?}");
    let instance_id = instances[0]["instance_id"]
        .as_str()
        .expect("an instance_id")
        .to_owned();
    let detail = client
        .view(
            "weft.workloads.InstanceDetail",
            &[("instance_id", instance_id.as_str())],
        )
        .ok_body(200);
    let detail = &detail["rows"][0];
    assert_eq!(
        (
            detail["state"].clone(),
            detail["exit_code"].clone(),
            detail["node_id"].clone(),
            detail["job_id"].clone(),
        ),
        (
            json!("Exited"),
            json!(0),
            json!(path.node.node_id),
            jobs[0]["job_id"].clone(),
        ),
        "{detail}"
    );
    let exit = json!({"instance_id": instance_id, "exit_code": 0, "stdout": "hello\n"});
    agent.wait_for(RUN_LIMIT, |lines| lines.contains(&exit));

    let log = host.log_after(before);

    // 2. The event lines of the path, in order.
    let names = events(&log);
    assert!(in_order(&names, EVENTS), "{names:?}");

    // 3. A 2xx request line for each operation of the path; none with status 501.
    let status = |line: &Value| line["status"].as_u64().expect("a status");
    for operation in REQUESTS {
        let requests: Vec<&Value> = log
            .iter()
            .filter(|line| line["kind"] == "request" && line["operation"] == *operation)
            .collect();
        assert!(
            requests
                .iter()
                .any(|line| (200..300).contains(&status(line))),
            "no 2xx request line of {operation}: {requests:?}; log: {log:?}"
        );
    }
    let unmet: Vec<&Value> = log
        .iter()
        .filter(|line| line["kind"] == "request" && line["status"] == json!(501))
        .collect();
    assert!(unmet.is_empty(), "request lines answered 501: {unmet:?}");

    // 4. The bindings of the run applied; r1-set-workload-replicas dropped not-a-service.
    let bindings = |id: &str| -> Vec<&Value> {
        log.iter()
            .filter(|line| line["kind"] == "binding" && line["binding"] == id)
            .collect()
    };
    for id in APPLIED {
        assert!(
            bindings(id).iter().any(|line| line["result"] == "applied"),
            "no applied line of {id}: {:?}; log: {log:?}",
            bindings(id)
        );
    }
    assert!(
        bindings("r1-set-workload-replicas")
            .iter()
            .any(|line| line["refusal"] == "not-a-service" && line["result"] == "dropped"),
        "{:?}; log: {log:?}",
        bindings("r1-set-workload-replicas")
    );

    // 5. The run's time, within the bound.
    assert!(elapsed < RUN_LIMIT, "{elapsed:?}");
}
