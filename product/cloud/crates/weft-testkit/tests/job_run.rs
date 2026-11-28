//! A rollout step of a Job workload runs its Job (`story:cloudspec-submission-job-run`, the
//! owner's decision D-C2 = A): the hand-run bindings of the block "R1 for a Job workload" in
//! `ess/components.yaml` take the `ReleaseStepStarted` of a Job workload to `SubmitJob`
//! (`r1-rollout-submit-job`), the submitted Job outside a gang to `StartJob`
//! (`job-start-on-submit`), each task of the started Job to an instance (`job-task-instances`)
//! and the task's instance to the scheduler (`job-instance-scheduling-request`); the Scheduler
//! and R4 take it to the node, where the agent runs it confined. Its exit finishes the Job
//! (`story:cloudspec-submission-job-exit`): the agent records an instance of restart mode `Never`
//! whose processes exited 0 `Exited`, `job-task-succeeded-on-exit` takes its `InstanceExited` to
//! `RecordTaskSucceeded` and `job-complete-on-task` the `TaskSucceeded` to `CompleteJob`.
//!
//! The release the step belongs to is made by `weft.deployments.ApplyPlan` from a `Ready` plan
//! seeded through the storage port, as `release_steps.rs` seeds it (who computes a plan is not
//! this story). The end-to-end case runs the control host binary beside `weft-agent`; the others
//! run a control host in this process, whose bindings run with every request and whose
//! controllers do not run.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weft_client::{Client, Expect};
use weft_impl::bindings::ENTRIES;
use weft_impl::context::mint;
use weft_impl::{Ports, Row};
use weft_testkit::steps::{self, input, Path, DEPLOYER};
use weft_testkit::{ControlHost, InProcess, NodeAgent};
use weft_types::architecture::{
    ComponentWorkloadData, ComponentWorkloadSnapshot, ComponentWorkloadState,
};
use weft_types::behaviour::{
    InstanceStorage, JobStorage, LogStreamStorage, MetricSeriesStorage, WorkloadStorage,
};
use weft_types::deployments::{PlanData, PlanSnapshot, PlanState};
use weft_types::observability::{
    LogStreamData, LogStreamKind, LogStreamSnapshot, LogStreamState, MetricName, MetricSeriesData,
    MetricSeriesSnapshot, MetricSeriesState, SeriesSource,
};
use weft_types::primitives::{Duration as SpecDuration, Timestamp, Uuid};
use weft_types::shared::{
    ComponentId, ComponentWorkloadId, EnvironmentId, FormationId, FormationRevisionId, InstanceId,
    JobId, LogStreamId, MetricSeriesId, NodeId, PlanId, PrincipalId, ScheduleId, ScopeId,
    WorkloadId,
};
use weft_types::workloads::{
    InstanceSnapshot, InstanceState, JobData, JobSettings, JobSnapshot, JobState, ScheduleData,
    ScheduleSnapshot, ScheduleState, ShapeSettings, WorkloadState,
};

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";
const SCHEDULER: &str = "weft.actors.Scheduler";
const SUBMIT_JOB: &str = "weft.workloads.SubmitJob";
const START_JOB: &str = "weft.workloads.StartJob";
const RECORD_EXITED: &str = "weft.workloads.RecordInstanceExited";
const RECORD_TASK: &str = "weft.workloads.RecordTaskSucceeded";
const COMPLETE_JOB: &str = "weft.workloads.CompleteJob";

/// The bindings of the block "R1 for a Job workload", in the order of `ess/components.yaml`.
const BLOCK: &[&str] = &[
    "r1-rollout-submit-job",
    "job-start-on-submit",
    "job-task-instances",
    "job-instance-scheduling-request",
];

/// The bindings of the exit of a Job's task (`story:cloudspec-submission-job-exit`), in the order of
/// `ess/components.yaml`.
const EXIT: &[&str] = &["job-task-succeeded-on-exit", "job-complete-on-task"];

/// How long the step may take from `StartReleaseStep` to the Job reading `Succeeded`.
const RUN_LIMIT: Duration = Duration::from_secs(30);

/// What the Job workload's definitions made.
struct JobDefinitions {
    formation_id: String,
    revision_id: String,
    environment_id: String,
    workload_id: String,
}

/// The `DefineWorkload` input of the skeleton's snippet, shaped `Job` with `parallelism`: one
/// try per task (`max_retries` 0, restart `Never`).
fn job_workload(of: input::WorkloadOf<'_>, parallelism: i64) -> Value {
    let mut workload = input::define_workload(of);
    workload["shape"] = json!("Job");
    workload["settings"] = json!({
        "shape": "job",
        "value": {"parallelism": parallelism, "gang": false, "max_retries": 0, "backoff": "PT10S"},
    });
    workload["restart_policy"]["mode"] = json!("Never");
    workload
}

/// The workload definitions of the skeleton (`steps::run_definitions`) with a Job workload of
/// parallelism 1 in place of the Service: formation `site` pinned to the default cluster, its
/// component, a revision, environment `prod`, the artifact, a runtime of `Snippet`, the Job
/// workload and its assignment to the component.
fn define_job(client: &Client, path: &Path) -> JobDefinitions {
    let caller = steps::path_caller(path);
    let scope_id = path.scope_id.as_str();
    let formation_id = steps::create_formation(client, &caller, scope_id);
    steps::pin_formation(client, &caller, scope_id, &path.cluster_id, &formation_id);
    let component_id = steps::add_component(client, &caller, &formation_id, scope_id);
    let revision_id = steps::commit_revision(client, &caller, &formation_id, scope_id);
    let environment_id = steps::create_environment(client, &caller, &formation_id, scope_id);
    steps::register_artifact(client, &caller, scope_id);
    let runtime_id = steps::register_runtime(client, &["Snippet"]);
    let of = input::WorkloadOf {
        scope_id,
        formation_id: &formation_id,
        formation_revision_id: &revision_id,
        environment_id: &environment_id,
        runtime_id: &runtime_id,
    };
    let workload_id = steps::define_workload(client, &caller, job_workload(of, 1));
    steps::assign_workload(
        client,
        &caller,
        input::assign_workload(of, &component_id, &workload_id),
    );
    JobDefinitions {
        formation_id,
        revision_id,
        environment_id,
        workload_id,
    }
}

/// A `Ready` plan of `formation_id` and `revision_id` for `environment_id` in `scope_id`.
fn ready_plan(
    plan_id: &str,
    scope_id: &str,
    formation_id: &str,
    revision_id: &str,
    environment_id: &str,
) -> Row {
    Row::Plan(PlanSnapshot {
        state: PlanState::Ready,
        data: PlanData {
            plan_id: PlanId(Uuid(plan_id.to_owned())),
            scope_id: ScopeId(Uuid(scope_id.to_owned())),
            formation_id: FormationId(Uuid(formation_id.to_owned())),
            environment_id: EnvironmentId(Uuid(environment_id.to_owned())),
            formation_revision_id: FormationRevisionId(Uuid(revision_id.to_owned())),
            translation_id: None,
            source_id: None,
            live_release_id: None,
            diff: None,
            simulation: None,
            created_by: PrincipalId(Uuid(mint())),
            created_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
        },
    })
}

/// A caller acting in `scope_id` with a principal of its own.
fn caller(scope_id: &str) -> Value {
    json!({"tenant_id": mint(), "principal_id": mint(), "scope_id": scope_id})
}

/// `command` sent as `actor` with `caller`: the body, which answers `outcome`.
fn answers(
    client: &Client,
    actor: &str,
    caller: &Value,
    command: &str,
    input: &Value,
    outcome: &str,
) -> Value {
    let answer = client
        .command(command, actor, caller.clone(), input.clone())
        .expect("the control host answers");
    assert_eq!(
        answer.body["outcome"],
        json!(outcome),
        "{command} {input}: {} {}",
        answer.status,
        answer.body
    );
    answer.body
}

/// The text member `field` of the first published event's payload.
fn payload_str(body: &Value, field: &str) -> String {
    body["published"][0]["payload"][field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {body}"))
        .to_owned()
}

/// On a `Ready` plan of `plan_id` in the scope of `caller`: `ApplyPlan`, a rollout step of one
/// replica at position 1, `StartRelease`. The step's identity.
fn release_with_a_rollout_step(client: &Client, caller: &Value, plan_id: &str) -> String {
    let scope_id = caller["scope_id"].clone();
    let body = answers(
        client,
        DEPLOYER,
        caller,
        "weft.deployments.ApplyPlan",
        &json!({"plan_id": plan_id, "scope_id": scope_id, "strategy": "Rolling"}),
        "created",
    );
    let release_id = payload_str(&body, "release_id");
    let body = answers(
        client,
        RELEASE_CONTROLLER,
        caller,
        "weft.deployments.AddReleaseStep",
        &json!({"release_id": release_id, "position": 1, "kind": "Rollout", "component": "site", "replicas": 1}),
        "added",
    );
    let step_id = payload_str(&body, "step_id");
    answers(
        client,
        RELEASE_CONTROLLER,
        caller,
        "weft.deployments.StartRelease",
        &json!({"release_id": release_id}),
        "started",
    );
    step_id
}

/// `StartReleaseStep` of `step_id` naming `workload_id`, as ReleaseController.
fn start_step(client: &Client, caller: &Value, step_id: &str, workload_id: &str) -> Value {
    answers(
        client,
        RELEASE_CONTROLLER,
        caller,
        "weft.deployments.StartReleaseStep",
        &json!({"step_id": step_id, "job_id": null, "workload_id": workload_id}),
        "started",
    )
}

/// The binding lines of `log` of the binding `id`.
fn binding_lines<'a>(log: &'a [Value], id: &str) -> Vec<&'a Value> {
    log.iter()
        .filter(|line| line["kind"] == "binding" && line["binding"] == id)
        .collect()
}

/// The names of the event lines of `log`.
fn event_names(log: &[Value]) -> Vec<String> {
    log.iter()
        .filter(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str().map(str::to_owned))
        .collect()
}

/// The rows of the `weft.workloads.Instances` view.
fn instances(client: &Client) -> Vec<Value> {
    client.view("weft.workloads.Instances", &[]).ok_body(200)["rows"]
        .as_array()
        .expect("rows")
        .clone()
}

/// Statements 1 and 2 of `story:cloudspec-submission-job-run` and of
/// `story:cloudspec-submission-job-exit`: with the skeleton node joined, starting the rollout
/// step of a release whose workload is a Snippet Job of parallelism 1 runs its one instance
/// (task 0) on that node (a 2xx `RecordInstanceRunning`, the agent report's exit line), and
/// within 30 s the instance reads `Exited` with exit code 0 and the Job reads `Succeeded` with
/// `succeeded_tasks` 1: the agent sent `RecordInstanceExited` (2xx), and the log holds one
/// `InstanceExited`, `TaskSucceeded` and `JobSucceeded`. Each binding of the block, and both
/// bindings of the exit (`job-task-succeeded-on-exit`, `job-complete-on-task`), is `applied`;
/// `r1-set-workload-replicas` drops `not-a-service`, and `r1-submit-scheduling-request` has no
/// line for the Job's instance.
///
/// Before the exit story this case ended with the instance and the Job `Running`; an instance
/// of restart mode `Never` that exited 0 now ends both.
#[test]
fn a_rollout_step_of_a_snippet_job_runs_its_one_task_on_the_node() {
    let mut host = ControlHost::start();
    let path = steps::run_path(&host.client());
    let defs = define_job(&host.client(), &path);
    // The plan is a row no request writes: it is seeded while the host is stopped.
    host.stop();
    let plan_id = mint();
    weft_testkit::seed(
        &host.state_path(),
        vec![ready_plan(
            &plan_id,
            &path.scope_id,
            &defs.formation_id,
            &defs.revision_id,
            &defs.environment_id,
        )],
    );
    let host = host.restart();
    let client = host.client();
    let mut agent = NodeAgent::start(&host, &path.node.node_id, &path.node.join_token);
    let caller = caller(&path.scope_id);
    let step_id = release_with_a_rollout_step(&client, &caller, &plan_id);

    let before = host.last_seq();
    let started = Instant::now();
    start_step(&client, &caller, &step_id, &defs.workload_id);

    // 1. Within 30 s the Job reads Succeeded and the task's instance Exited.
    let (jobs, rows) = loop {
        let jobs = client.view("weft.workloads.Jobs", &[]).ok_body(200)["rows"]
            .as_array()
            .expect("rows")
            .clone();
        let rows = instances(&client);
        if jobs.iter().any(|job| job["state"] == json!("Succeeded"))
            && rows.iter().any(|row| row["state"] == json!("Exited"))
        {
            break (jobs, rows);
        }
        assert!(
            started.elapsed() < RUN_LIMIT,
            "no Succeeded Job and Exited instance {RUN_LIMIT:?} after StartReleaseStep: jobs \
             {jobs:?}; instances {rows:?}; agent report: {:?}; log: {:?}",
            agent.report(),
            host.log_after(before)
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(rows.len(), 1, "one instance: {rows:?}");
    let instance_id = rows[0]["instance_id"]
        .as_str()
        .expect("an instance_id")
        .to_owned();
    let exit = json!({"instance_id": instance_id, "exit_code": 0, "stdout": "weft\n"});
    agent.wait_for(RUN_LIMIT, |lines| lines.contains(&exit));

    let detail = client
        .view(
            "weft.workloads.InstanceDetail",
            &[("instance_id", instance_id.as_str())],
        )
        .ok_body(200);
    let detail = &detail["rows"][0];
    assert_eq!(
        (
            detail["workload_id"].clone(),
            detail["task_index"].clone(),
            detail["node_id"].clone(),
            detail["service_id"].clone(),
            detail["state"].clone(),
            detail["exit_code"].clone(),
        ),
        (
            json!(defs.workload_id),
            json!(0),
            json!(path.node.node_id),
            Value::Null,
            json!("Exited"),
            json!(0),
        ),
        "{detail}"
    );

    assert_eq!(jobs.len(), 1, "one Job: {jobs:?}");
    assert_eq!(
        (
            jobs[0]["job_id"].clone(),
            jobs[0]["parallelism"].clone(),
            jobs[0]["gang"].clone(),
            jobs[0]["succeeded_tasks"].clone(),
            jobs[0]["state"].clone(),
        ),
        (
            detail["job_id"].clone(),
            json!(1),
            json!(false),
            json!(1),
            json!("Succeeded")
        ),
        "{jobs:?}"
    );

    let log = host.log_after(before);
    let events = event_names(&log);
    for name in [
        "weft.workloads.JobSubmitted",
        "weft.workloads.JobStarted",
        "weft.workloads.InstanceRunning",
        "weft.workloads.InstanceExited",
        "weft.workloads.TaskSucceeded",
        "weft.workloads.JobSucceeded",
    ] {
        assert_eq!(
            events.iter().filter(|event| *event == name).count(),
            1,
            "one {name} line: {events:?}"
        );
    }
    let status = |line: &Value| line["status"].as_u64().expect("a status");
    for operation in [
        "weft.workloads.RecordInstanceRunning",
        "weft.workloads.RecordInstanceExited",
    ] {
        let requests: Vec<&Value> = log
            .iter()
            .filter(|line| line["kind"] == "request" && line["operation"] == operation)
            .collect();
        assert!(
            requests
                .iter()
                .any(|line| (200..300).contains(&status(line))),
            "no 2xx request line of {operation}: {requests:?}; log: {log:?}"
        );
    }

    // 2. The block's bindings and the exit's applied; r1-set-workload-replicas dropped
    // not-a-service; r1-submit-scheduling-request skipped the Job's instance.
    for id in BLOCK.iter().chain(EXIT) {
        let lines = binding_lines(&log, id);
        assert!(
            lines.iter().any(|line| line["result"] == "applied"),
            "no applied line of {id}: {lines:?}; log: {log:?}"
        );
        assert!(
            lines.iter().all(|line| line["result"] == "applied"),
            "{id}: {lines:?}"
        );
    }
    let replicas = binding_lines(&log, "r1-set-workload-replicas");
    assert!(
        replicas
            .iter()
            .any(|line| line["refusal"] == "not-a-service" && line["result"] == "dropped"),
        "{replicas:?}"
    );
    assert_eq!(
        binding_lines(&log, "r1-submit-scheduling-request"),
        Vec::<&Value>::new()
    );
    // The edge's reaction to the exit is answered once and holds nothing.
    let backend = binding_lines(&log, "edge-backend-exited");
    assert!(
        backend.len() == 1 && backend[0]["result"] != "retried",
        "{backend:?}; log: {log:?}"
    );
}

/// A control host in this process with the operator and tenant path and the skeleton
/// definitions (a Service workload), and a Job workload of parallelism 1 beside it.
struct World {
    host: InProcess,
    path: Path,
    service: String,
    job: String,
    formation_id: String,
    component_id: String,
    revision_id: String,
    environment_id: String,
}

impl World {
    fn new() -> Self {
        let host = InProcess::start();
        let path = steps::run_path(host.client());
        let defs = steps::run_definitions(host.client(), &path);
        let of = input::WorkloadOf {
            scope_id: &path.scope_id,
            formation_id: &defs.formation_id,
            formation_revision_id: &defs.revision_id,
            environment_id: &defs.environment_id,
            runtime_id: &defs.runtime_id,
        };
        let job = steps::define_workload(
            host.client(),
            &steps::path_caller(&path),
            job_workload(of, 1),
        );
        Self {
            path,
            service: defs.workload_id,
            job,
            formation_id: defs.formation_id,
            component_id: defs.component_id,
            revision_id: defs.revision_id,
            environment_id: defs.environment_id,
            host,
        }
    }

    fn ports(&self) -> Ports {
        self.host.ports()
    }

    fn client(&self) -> &Client {
        self.host.client()
    }

    fn caller(&self) -> Value {
        caller(&self.path.scope_id)
    }

    /// A release of a seeded `Ready` plan with a started release: its rollout step.
    fn rollout_step(&self) -> String {
        let plan_id = mint();
        self.ports().put_row(ready_plan(
            &plan_id,
            &self.path.scope_id,
            &self.formation_id,
            &self.revision_id,
            &self.environment_id,
        ));
        release_with_a_rollout_step(self.client(), &self.caller(), &plan_id)
    }

    /// An assignment of `workload_id` to the skeleton component in the skeleton environment,
    /// seeded beside the Service's (`AssignWorkload` answers `already-assigned` for a second
    /// workload of the component), as `release_steps.rs` seeds it.
    fn assign(&self, workload_id: &str) {
        self.ports()
            .put_row(Row::ComponentWorkload(ComponentWorkloadSnapshot {
                state: ComponentWorkloadState::Assigned,
                data: ComponentWorkloadData {
                    component_workload_id: ComponentWorkloadId(Uuid(mint())),
                    formation_id: FormationId(Uuid(self.formation_id.clone())),
                    formation_revision_id: FormationRevisionId(Uuid(self.revision_id.clone())),
                    environment_id: EnvironmentId(Uuid(self.environment_id.clone())),
                    component_id: ComponentId(Uuid(self.component_id.clone())),
                    workload_id: workload(workload_id),
                },
            }));
    }

    /// The Job workload's row, copied under a new identity with `settings` and `state`: the
    /// copy's identity.
    fn job_copy(&self, settings: JobSettings, state: WorkloadState) -> String {
        let mut ports = self.ports();
        let mut row = WorkloadStorage::get(&ports, &workload(&self.job)).expect("the Job workload");
        let id = mint();
        row.data.workload_id = workload(&id);
        row.data.settings = ShapeSettings::Job(settings);
        row.state = state;
        ports.put_row(Row::Workload(row));
        id
    }

    /// A Job of the Job workload in `state`, seeded: its identity.
    fn seeded_job(&self, parallelism: i64, gang: bool, state: JobState) -> String {
        let id = mint();
        self.ports().put_row(Row::Job(JobSnapshot {
            state,
            data: JobData {
                job_id: JobId(Uuid(id.clone())),
                workload_id: workload(&self.job),
                scope_id: ScopeId(Uuid(self.path.scope_id.clone())),
                parallelism,
                gang,
                max_retries: 0,
                retries_used: 0,
                succeeded_tasks: 0,
                outputs: Vec::new(),
                failure: None,
                schedule_id: None,
            },
        }));
        id
    }

    /// `command` as `actor` with `caller` is refused with `outcome` and `error`, and no Job,
    /// instance or workload row changes, and no event is logged.
    fn refuses_as(
        &self,
        actor: &str,
        caller: &Value,
        command: &str,
        input: &Value,
        outcome: &str,
        error: &str,
    ) {
        let before = rows(&self.ports());
        let seq = self.host.last_seq();
        let body = answers(self.client(), actor, caller, command, input, outcome);
        assert!(
            body.to_string().contains(&format!(".{error}\"")),
            "{command} {input}: no error {error} in {body}"
        );
        assert_eq!(rows(&self.ports()), before, "{command} {input} wrote a row");
        assert_eq!(
            event_names(&self.host.log_after(seq)),
            Vec::<String>::new(),
            "{command} {input} published"
        );
    }

    fn submit_refused(&self, input: &Value, outcome: &str, error: &str) {
        self.refuses_as(DEPLOYER, &self.caller(), SUBMIT_JOB, input, outcome, error);
    }

    fn start_refused(&self, input: &Value, outcome: &str, error: &str) {
        self.refuses_as(SCHEDULER, &json!({}), START_JOB, input, outcome, error);
    }
}

fn workload(id: &str) -> WorkloadId {
    WorkloadId(Uuid(id.to_owned()))
}

fn job_settings(parallelism: i64) -> JobSettings {
    JobSettings {
        parallelism,
        gang: false,
        max_retries: 0,
        backoff: SpecDuration("PT10S".to_owned()),
    }
}

fn submit(workload_id: &str, scope_id: &str, parallelism: i64, max_retries: i64) -> Value {
    json!({
        "workload_id": workload_id,
        "scope_id": scope_id,
        "parallelism": parallelism,
        "gang": false,
        "max_retries": max_retries,
        "schedule_id": null,
    })
}

/// Every Job, instance and workload row, as text: a refusal leaves it as it was.
fn rows(ports: &Ports) -> String {
    format!(
        "{:?} {:?} {:?}",
        JobStorage::list(ports),
        InstanceStorage::list(ports),
        WorkloadStorage::list(ports)
    )
}

/// Statement 3, `SubmitJob` sent directly: every refusal in the contract's order writes no row
/// and publishes nothing (a Service workload answers `ShapeMismatch`, parallelism 0
/// `InvalidAmount`); a Job workload's Job is submitted `Pending` with the workload's scope.
#[test]
fn submit_job_answers_each_outcome_in_order_and_a_refusal_writes_no_row() {
    let w = World::new();
    let scope = w.path.scope_id.clone();

    w.submit_refused(
        &submit(&w.job, &mint(), 1, 0),
        "not-callers-scope",
        "WorkloadNotFound",
    );
    w.submit_refused(&submit(&w.job, &scope, 0, 0), "bad-counts", "InvalidAmount");
    w.submit_refused(
        &submit(&w.job, &scope, 1, -1),
        "bad-counts",
        "InvalidAmount",
    );
    w.submit_refused(
        &submit(&mint(), &scope, 1, 0),
        "no-workload",
        "WorkloadNotFound",
    );
    let mut missing = submit(&w.job, &scope, 1, 0);
    missing["schedule_id"] = json!(mint());
    w.submit_refused(&missing, "schedule-missing", "NotFound");
    let other = mint();
    w.refuses_as(
        DEPLOYER,
        &caller(&other),
        SUBMIT_JOB,
        &submit(&w.job, &other, 1, 0),
        "workload-of-other-scope",
        "WorkloadNotFound",
    );
    let schedule = mint();
    w.ports().put_row(Row::Schedule(ScheduleSnapshot {
        state: ScheduleState::Active,
        data: ScheduleData {
            schedule_id: ScheduleId(Uuid(schedule.clone())),
            workload_id: workload(&w.service),
            scope_id: ScopeId(Uuid(scope.clone())),
            last_fired_at: None,
        },
    }));
    let mut foreign = submit(&w.job, &scope, 1, 0);
    foreign["schedule_id"] = json!(schedule);
    w.submit_refused(&foreign, "schedule-of-other-workload", "ForeignOwner");
    let retired = w.job_copy(job_settings(1), WorkloadState::Retired);
    w.submit_refused(
        &submit(&retired, &scope, 1, 0),
        "workload-retired",
        "WorkloadIsRetired",
    );
    w.submit_refused(
        &submit(&w.service, &scope, 1, 0),
        "not-a-job",
        "ShapeMismatch",
    );
    // 65 tasks of one cpu each, past the account's quota of 64.
    w.submit_refused(
        &submit(&w.job, &scope, 65, 0),
        "over-quota",
        "QuotaExceeded",
    );

    let seq = w.host.last_seq();
    let body = answers(
        w.client(),
        DEPLOYER,
        &w.caller(),
        SUBMIT_JOB,
        &json!({"workload_id": w.job, "scope_id": scope, "parallelism": 2, "gang": true, "max_retries": 3, "schedule_id": null}),
        "submitted",
    );
    let payload = &body["published"][0]["payload"];
    let job_id = payload_str(&body, "job_id");
    assert_eq!(
        payload,
        &json!({"job_id": job_id, "workload_id": w.job, "parallelism": 2, "gang": true}),
        "{body}"
    );
    let job = JobStorage::get(&w.ports(), &JobId(Uuid(job_id))).expect("the Job row");
    assert_eq!(job.state, JobState::Pending, "{job:?}");
    assert_eq!(
        (
            job.data.workload_id.clone(),
            job.data.scope_id.clone(),
            job.data.parallelism,
            job.data.gang,
            job.data.max_retries,
            job.data.retries_used,
            job.data.succeeded_tasks,
            job.data.outputs.clone(),
            job.data.failure.clone(),
            job.data.schedule_id.clone(),
        ),
        (
            workload(&w.job),
            ScopeId(Uuid(scope)),
            2,
            true,
            3,
            0,
            0,
            Vec::new(),
            None,
            None
        ),
        "{job:?}"
    );
    // A gang Job waits for its gang: job-start-on-submit does not react.
    let log = w.host.log_after(seq);
    assert_eq!(event_names(&log), vec!["weft.workloads.JobSubmitted"]);
    assert_eq!(
        binding_lines(&log, "job-start-on-submit"),
        Vec::<&Value>::new()
    );
}

/// Statement 3, `StartJob` sent directly: every refusal in the contract's order writes no row
/// (`bound_tasks` 0 answers `InvalidAmount`); a Pending Job starts `Running` once.
#[test]
fn start_job_answers_each_outcome_in_order_and_a_refusal_writes_no_row() {
    let w = World::new();
    let job = w.seeded_job(1, false, JobState::Pending);
    let gang = w.seeded_job(2, true, JobState::Pending);

    w.start_refused(
        &json!({"job_id": job, "bound_tasks": 0}),
        "none-bound",
        "InvalidAmount",
    );
    w.start_refused(
        &json!({"job_id": mint(), "bound_tasks": 1}),
        "no-such-job",
        "NotFound",
    );
    w.start_refused(
        &json!({"job_id": gang, "bound_tasks": 1}),
        "gang-incomplete",
        "GangIncomplete",
    );
    w.start_refused(
        &json!({"job_id": job, "bound_tasks": 2}),
        "too-many",
        "InvalidAmount",
    );

    let body = answers(
        w.client(),
        SCHEDULER,
        &json!({}),
        START_JOB,
        &json!({"job_id": job, "bound_tasks": 1}),
        "started",
    );
    assert_eq!(
        body["published"][0]["payload"],
        json!({"job_id": job, "bound_tasks": 1}),
        "{body}"
    );
    let row = JobStorage::get(&w.ports(), &JobId(Uuid(job.clone()))).expect("the Job row");
    assert_eq!(row.state, JobState::Running);
    w.start_refused(
        &json!({"job_id": job, "bound_tasks": 1}),
        "wrong-state",
        "StateConflict",
    );
}

/// The bindings in one process: the rollout step of the Job workload creates one Pending
/// instance of task 0 with its scheduling request, nothing is held; a repeated delivery of
/// `JobSubmitted`, `JobStarted` and the instance's `InstanceCreated` makes nothing new (dropped
/// `wrong-state`, no second instance, dropped `already-requested`).
#[test]
fn the_block_takes_a_job_rollout_step_to_one_requested_instance_and_a_repeat_makes_none() {
    let w = World::new();
    w.assign(&w.job);
    let step_id = w.rollout_step();
    let seq = w.host.last_seq();
    start_step(w.client(), &w.caller(), &step_id, &w.job);

    let log = w.host.log_after(seq);
    assert_eq!(
        event_names(&log),
        vec![
            "weft.deployments.ReleaseStepStarted",
            "weft.workloads.JobSubmitted",
            "weft.workloads.JobStarted",
            "weft.workloads.InstanceCreated",
            "weft.scheduling.SchedulingRequested",
        ],
        "{log:?}"
    );
    for id in BLOCK {
        let lines = binding_lines(&log, id);
        assert_eq!(lines.len(), 1, "{id}: {lines:?}");
        assert_eq!(lines[0]["result"], json!("applied"), "{id}: {lines:?}");
    }
    assert_eq!(w.host.host().held(), 0);

    let ports = w.ports();
    let jobs = JobStorage::list(&ports);
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = &jobs[0];
    assert_eq!(
        (
            job.state,
            job.data.parallelism,
            job.data.gang,
            job.data.max_retries
        ),
        (JobState::Running, 1, false, 0),
        "{job:?}"
    );
    let tasks: Vec<_> = InstanceStorage::list(&ports)
        .into_iter()
        .filter(|row| row.data.job_id.is_some())
        .collect();
    assert_eq!(tasks.len(), 1, "{tasks:?}");
    assert_eq!(
        (
            tasks[0].data.job_id.clone(),
            tasks[0].data.task_index,
            tasks[0].data.workload_id.clone()
        ),
        (Some(job.data.job_id.clone()), Some(0), workload(&w.job)),
        "{tasks:?}"
    );

    let seq = w.host.last_seq();
    let created = InstanceStorage::list(&ports)
        .into_iter()
        .find(|row| row.data.job_id.is_some())
        .expect("the task's instance");
    // The two events again, read from their held form by the codec of the binding that holds
    // each.
    let held = |id: &str, value: Value| {
        let entry = ENTRIES
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("no hand-run binding {id}"));
        (entry.decode)(&value).unwrap_or_else(|error| panic!("{id}: {error}"))
    };
    let job_id = &job.data.job_id.0 .0;
    w.host.host().replay(vec![
        held(
            "job-start-on-submit",
            json!({"job_id": job_id, "workload_id": w.job, "parallelism": 1, "gang": false}),
        ),
        held(
            "job-task-instances",
            json!({"job_id": job_id, "bound_tasks": 1}),
        ),
    ]);
    let log = w.host.log_after(seq);
    let start = binding_lines(&log, "job-start-on-submit");
    assert_eq!(
        start
            .iter()
            .map(|line| (line["refusal"].clone(), line["result"].clone()))
            .collect::<Vec<_>>(),
        vec![(json!("wrong-state"), json!("dropped"))],
        "{log:?}"
    );
    assert_eq!(event_names(&log), Vec::<String>::new(), "{log:?}");
    let after: Vec<_> = InstanceStorage::list(&ports)
        .into_iter()
        .filter(|row| row.data.job_id.is_some())
        .collect();
    assert_eq!(after.len(), 1, "a second instance of task 0: {after:?}");
    assert_eq!(after[0].data.instance_id, created.data.instance_id);
    assert_eq!(w.host.host().held(), 0);
}

/// `r1-rollout-submit-job` on the rollout step of a Service workload: `SubmitJob` answers
/// `not-a-job`, dropped; no Job is made and nothing is held.
#[test]
fn a_rollout_step_of_a_service_drops_not_a_job() {
    let w = World::new();
    let step_id = w.rollout_step();
    let seq = w.host.last_seq();
    start_step(w.client(), &w.caller(), &step_id, &w.service);
    let log = w.host.log_after(seq);
    let lines = binding_lines(&log, "r1-rollout-submit-job");
    assert_eq!(
        lines
            .iter()
            .map(|line| (line["refusal"].clone(), line["result"].clone()))
            .collect::<Vec<_>>(),
        vec![(json!("not-a-job"), json!("dropped"))],
        "{log:?}"
    );
    assert_eq!(JobStorage::list(&w.ports()), Vec::new());
    assert_eq!(w.host.host().held(), 0);
}

impl World {
    /// The Job workload's rollout step started, so its Job is `Running` with one `Pending`
    /// instance of task 0, and that instance moved to `Running` on the skeleton's node through
    /// the storage port (no controller runs in this process): the Job's and the instance's
    /// identities.
    fn running_task(&self) -> (String, String) {
        self.assign(&self.job);
        let step_id = self.rollout_step();
        start_step(self.client(), &self.caller(), &step_id, &self.job);
        let mut ports = self.ports();
        let mut task = InstanceStorage::list(&ports)
            .into_iter()
            .find(|row| row.data.job_id.is_some())
            .expect("the task's instance");
        task.state = InstanceState::Running;
        task.data.node_id = Some(NodeId(Uuid(self.path.node.node_id.clone())));
        let job_id = task.data.job_id.clone().expect("a Job").0 .0;
        let instance_id = task.data.instance_id.0 .0.clone();
        ports.put_row(Row::Instance(task));
        (job_id, instance_id)
    }

    /// The instance `instance_id` copied under a new identity, changed by `change`: the copy's
    /// identity.
    fn instance_copy(
        &self,
        instance_id: &str,
        change: impl FnOnce(&mut InstanceSnapshot),
    ) -> String {
        let mut ports = self.ports();
        let mut row = InstanceStorage::get(&ports, &InstanceId(Uuid(instance_id.to_owned())))
            .expect("the instance");
        let id = mint();
        row.data.instance_id = InstanceId(Uuid(id.clone()));
        change(&mut row);
        ports.put_row(Row::Instance(row));
        id
    }

    /// The Job `job_id` with `succeeded_tasks` set to `succeeded`.
    fn with_succeeded(&self, job_id: &str, succeeded: i64) {
        let mut ports = self.ports();
        let mut row = JobStorage::get(&ports, &JobId(Uuid(job_id.to_owned()))).expect("the Job");
        row.data.succeeded_tasks = succeeded;
        ports.put_row(Row::Job(row));
    }

    fn job_row(&self, job_id: &str) -> JobSnapshot {
        JobStorage::get(&self.ports(), &JobId(Uuid(job_id.to_owned()))).expect("the Job row")
    }

    /// `RecordInstanceExited` of `instance_id` on the skeleton's node with `exit_code`, as the
    /// NodeAgent: the answer's body, which answers `outcome`.
    fn exited(&self, instance_id: &str, exit_code: i64, outcome: &str) -> Value {
        answers(
            self.client(),
            steps::NODE_AGENT,
            &json!({}),
            RECORD_EXITED,
            &json!({"instance_id": instance_id, "node_id": self.path.node.node_id, "exit_code": exit_code}),
            outcome,
        )
    }

    fn complete_refused(&self, input: &Value, outcome: &str, error: &str) {
        self.refuses_as(SCHEDULER, &json!({}), COMPLETE_JOB, input, outcome, error);
    }
}

/// The `weft.workloads` events of the exit in `log`, in log order.
fn exit_events(log: &[Value]) -> Vec<String> {
    event_names(log)
        .into_iter()
        .filter(|name| {
            [
                "weft.workloads.InstanceExited",
                "weft.workloads.TaskSucceeded",
                "weft.workloads.JobSucceeded",
            ]
            .contains(&name.as_str())
        })
        .collect()
}

/// The refusal and result of each binding line of `id` in `log`.
fn results(log: &[Value], id: &str) -> Vec<(Value, Value)> {
    binding_lines(log, id)
        .iter()
        .map(|line| (line["refusal"].clone(), line["result"].clone()))
        .collect()
}

/// The bindings of the exit in one process: `RecordInstanceExited` of a Job's task instance with
/// exit code 0 answers `exited` (2xx), `job-task-succeeded-on-exit` records the task succeeded
/// and `job-complete-on-task` completes the Job: `InstanceExited`, `TaskSucceeded`,
/// `JobSucceeded` in that order, each binding applied once, nothing held; the Job reads
/// `Succeeded` with `succeeded_tasks` 1 and no output, the instance `Exited` with exit code 0. A
/// repeated delivery of the `InstanceExited` and the `TaskSucceeded` changes nothing
/// (`not-running` and `wrong-state`, each dropped).
#[test]
fn the_exit_of_a_job_task_records_its_task_succeeded_and_completes_the_job() {
    let w = World::new();
    let (job_id, instance_id) = w.running_task();
    let seq = w.host.last_seq();
    let client = w.client();
    let answer = client
        .command(
            RECORD_EXITED,
            steps::NODE_AGENT,
            json!({}),
            json!({"instance_id": instance_id, "node_id": w.path.node.node_id, "exit_code": 0}),
        )
        .expect("the control host answers");
    assert!(
        (200..300).contains(&answer.status) && answer.body["outcome"] == json!("exited"),
        "{} {}",
        answer.status,
        answer.body
    );

    let log = w.host.log_after(seq);
    assert_eq!(
        exit_events(&log),
        vec![
            "weft.workloads.InstanceExited",
            "weft.workloads.TaskSucceeded",
            "weft.workloads.JobSucceeded",
        ],
        "{log:?}"
    );
    for id in EXIT {
        assert_eq!(
            results(&log, id),
            vec![(Value::Null, json!("applied"))],
            "{id}: {log:?}"
        );
    }
    // The instance never was ready in this process, so it is no backend of the edge.
    assert_eq!(
        results(&log, "edge-backend-exited"),
        vec![(json!("no-backend"), json!("dropped"))],
        "{log:?}"
    );
    assert_eq!(w.host.host().held(), 0);
    let job = w.job_row(&job_id);
    assert_eq!(
        (
            job.state,
            job.data.succeeded_tasks,
            job.data.outputs.clone()
        ),
        (JobState::Succeeded, 1, Vec::new()),
        "{job:?}"
    );
    let instance = InstanceStorage::get(&w.ports(), &InstanceId(Uuid(instance_id.clone())))
        .expect("the instance");
    assert_eq!(
        (instance.state, instance.data.exit_code),
        (InstanceState::Exited, Some(0)),
        "{instance:?}"
    );

    // The two events again, read from their held form by the codec of the binding that holds
    // each.
    let held = |id: &str, value: Value| {
        let entry = ENTRIES
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("no hand-run binding {id}"));
        (entry.decode)(&value).unwrap_or_else(|error| panic!("{id}: {error}"))
    };
    let seq = w.host.last_seq();
    w.host.host().replay(vec![
        held(
            "job-task-succeeded-on-exit",
            json!({
                "instance_id": instance_id,
                "exit_code": 0,
                "workload_id": w.job,
                "service_id": [],
                "replica_index": [],
                "restart": false,
            }),
        ),
        held(
            "job-complete-on-task",
            json!({"job_id": job_id, "task_index": 0}),
        ),
    ]);
    let log = w.host.log_after(seq);
    assert_eq!(
        results(&log, "job-task-succeeded-on-exit"),
        vec![(json!("not-running"), json!("dropped"))],
        "{log:?}"
    );
    assert_eq!(
        results(&log, "job-complete-on-task"),
        vec![(json!("wrong-state"), json!("dropped"))],
        "{log:?}"
    );
    assert_eq!(event_names(&log), Vec::<String>::new(), "{log:?}");
    let again = w.job_row(&job_id);
    assert_eq!(again, job);
    assert_eq!(w.host.host().held(), 0);
}

/// `job-task-succeeded-on-exit` does not react to an exit other than 0, nor to an instance of no
/// Job: each is recorded `Exited`, no `TaskSucceeded` follows, and the Job stays `Running` with
/// no task succeeded.
#[test]
fn a_non_zero_exit_and_an_instance_of_no_job_succeed_no_task() {
    let w = World::new();
    let (job_id, instance_id) = w.running_task();
    let alone = w.instance_copy(&instance_id, |row| {
        row.data.job_id = None;
        row.data.task_index = None;
    });
    let seq = w.host.last_seq();
    w.exited(&instance_id, 3, "exited");
    w.exited(&alone, 0, "exited");
    let log = w.host.log_after(seq);
    assert_eq!(
        exit_events(&log),
        vec![
            "weft.workloads.InstanceExited",
            "weft.workloads.InstanceExited"
        ],
        "{log:?}"
    );
    for id in EXIT {
        assert_eq!(results(&log, id), Vec::new(), "{id}: {log:?}");
    }
    let job = w.job_row(&job_id);
    assert_eq!(
        (job.state, job.data.succeeded_tasks),
        (JobState::Running, 0),
        "{job:?}"
    );
    assert_eq!(w.host.host().held(), 0);
}

/// Statement 3 of `story:cloudspec-submission-job-exit`, each sent directly: `RecordInstanceExited`
/// from another node answers `WrongNode`, `RecordTaskSucceeded` of task index 1 of a
/// parallelism-1 Job `InvalidAmount`, `CompleteJob` of a Job with no succeeded task
/// `TasksOutstanding`; none changes a row or publishes.
#[test]
fn the_exit_commands_sent_directly_refuse_and_change_no_row() {
    let w = World::new();
    let (job_id, instance_id) = w.running_task();
    w.refuses_as(
        steps::NODE_AGENT,
        &json!({}),
        RECORD_EXITED,
        &json!({"instance_id": instance_id, "node_id": mint(), "exit_code": 0}),
        "wrong-node",
        "WrongNode",
    );
    w.refuses_as(
        steps::NODE_AGENT,
        &json!({}),
        RECORD_TASK,
        &json!({"job_id": job_id, "task_index": 1}),
        "index-out-of-range",
        "InvalidAmount",
    );
    w.complete_refused(
        &json!({"job_id": job_id, "outputs": []}),
        "outstanding",
        "TasksOutstanding",
    );
}

/// `CompleteJob` (`behaviours/complete_job.rs`) answers each outcome of its contract in its
/// order, and a refusal writes no row: `no-such-job`; `outstanding` (ahead of `wrong-state`: a
/// `Pending` Job with no task succeeded); `wrong-state` (a `Pending` Job with every task
/// succeeded, ahead of `unknown-output`); `unknown-output` (an output no artifact carries);
/// `succeeded` (a `Running` Job with every task succeeded and a registered output: `Succeeded`
/// with that output, `JobSucceeded` naming it); `wrong-state` again for the `Succeeded` Job.
#[test]
fn complete_job_answers_each_outcome_in_order_and_a_refusal_writes_no_row() {
    let w = World::new();
    let unknown = "sha256:no-such-artifact";
    let registered = input::ARTIFACT_DIGEST;

    w.complete_refused(
        &json!({"job_id": mint(), "outputs": []}),
        "no-such-job",
        "NotFound",
    );
    let pending = w.seeded_job(1, false, JobState::Pending);
    w.complete_refused(
        &json!({"job_id": pending, "outputs": [unknown]}),
        "outstanding",
        "TasksOutstanding",
    );
    w.with_succeeded(&pending, 1);
    w.complete_refused(
        &json!({"job_id": pending, "outputs": [unknown]}),
        "wrong-state",
        "StateConflict",
    );
    let running = w.seeded_job(2, false, JobState::Running);
    w.with_succeeded(&running, 1);
    w.complete_refused(
        &json!({"job_id": running, "outputs": []}),
        "outstanding",
        "TasksOutstanding",
    );
    w.with_succeeded(&running, 2);
    w.complete_refused(
        &json!({"job_id": running, "outputs": [registered, unknown]}),
        "unknown-output",
        "ArtifactNotFound",
    );

    let seq = w.host.last_seq();
    let body = answers(
        w.client(),
        SCHEDULER,
        &json!({}),
        COMPLETE_JOB,
        &json!({"job_id": running, "outputs": [registered]}),
        "succeeded",
    );
    assert_eq!(
        body["published"][0]["payload"],
        json!({"job_id": running, "outputs": [registered]}),
        "{body}"
    );
    assert_eq!(
        event_names(&w.host.log_after(seq)),
        vec!["weft.workloads.JobSucceeded"]
    );
    let row = w.job_row(&running);
    assert_eq!(
        (
            row.state,
            row.data.succeeded_tasks,
            row.data
                .outputs
                .iter()
                .map(|digest| digest.0.clone())
                .collect::<Vec<_>>()
        ),
        (JobState::Succeeded, 2, vec![registered.to_owned()]),
        "{row:?}"
    );
    w.complete_refused(
        &json!({"job_id": running, "outputs": [registered]}),
        "wrong-state",
        "StateConflict",
    );
}

/// The other reactions to `InstanceExited`, delivered by hand since this story put the event on
/// the path: `telemetry-close-log-streams-on-exit` closes the instance's `Open` log stream
/// (`closed_at` and `retained_until` set) and `telemetry-retire-series-on-exit` retires its
/// `Active` metric series, each applied once with its two events; a stream and a series of
/// another instance are left as they were. `edge-backend-exited` drops `no-backend` (the
/// instance never was ready in this process). An instance with no open stream and no active
/// series gets no line of either telemetry binding.
#[test]
fn the_exit_closes_the_instance_s_log_streams_and_retires_its_series() {
    let w = World::new();
    let (_job_id, instance_id) = w.running_task();
    let alone = w.instance_copy(&instance_id, |row| {
        row.data.job_id = None;
        row.data.task_index = None;
    });
    let node = NodeId(Uuid(w.path.node.node_id.clone()));
    let scope = ScopeId(Uuid(w.path.scope_id.clone()));
    let stream = |of: &str| {
        let id = mint();
        w.ports().put_row(Row::LogStream(LogStreamSnapshot {
            state: LogStreamState::Open,
            data: LogStreamData {
                log_stream_id: LogStreamId(Uuid(id.clone())),
                scope_id: scope.clone(),
                instance_id: InstanceId(Uuid(of.to_owned())),
                node_id: node.clone(),
                kind: LogStreamKind::Process,
                process: None,
                retention_days: 7,
                opened_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
                closed_at: None,
                retained_until: None,
            },
        }));
        id
    };
    let series = |of: &str| {
        let id = mint();
        w.ports().put_row(Row::MetricSeries(MetricSeriesSnapshot {
            state: MetricSeriesState::Active,
            data: MetricSeriesData {
                metric_series_id: MetricSeriesId(Uuid(id.clone())),
                name: MetricName("cpu_seconds".to_owned()),
                labels: std::collections::BTreeMap::new(),
                source: SeriesSource::System,
                node_id: node.clone(),
                instance_id: Some(InstanceId(Uuid(of.to_owned()))),
                scope_id: Some(scope.clone()),
                identity_document_id: None,
                registered_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
            },
        }));
        id
    };
    let (own_stream, own_series) = (stream(&instance_id), series(&instance_id));
    let (other_stream, other_series) = (stream(&alone), series(&alone));

    let seq = w.host.last_seq();
    w.exited(&instance_id, 0, "exited");
    let log = w.host.log_after(seq);
    for id in [
        "telemetry-close-log-streams-on-exit",
        "telemetry-retire-series-on-exit",
    ] {
        assert_eq!(
            results(&log, id),
            vec![(Value::Null, json!("applied"))],
            "{id}: {log:?}"
        );
    }
    assert_eq!(
        results(&log, "edge-backend-exited"),
        vec![(json!("no-backend"), json!("dropped"))],
        "{log:?}"
    );
    let events = event_names(&log);
    for name in [
        "weft.observability.InstanceLogStreamsClosed",
        "weft.observability.LogStreamClosed",
        "weft.observability.InstanceMetricSeriesRetired",
        "weft.observability.MetricSeriesRetired",
    ] {
        assert_eq!(
            events.iter().filter(|event| *event == name).count(),
            1,
            "one {name} line: {events:?}"
        );
    }
    assert_eq!(w.host.host().held(), 0);

    let ports = w.ports();
    let stream_row = |id: &str| {
        LogStreamStorage::get(&ports, &LogStreamId(Uuid(id.to_owned()))).expect("a stream")
    };
    let series_row = |id: &str| {
        MetricSeriesStorage::get(&ports, &MetricSeriesId(Uuid(id.to_owned()))).expect("a series")
    };
    let closed = stream_row(&own_stream);
    assert_eq!(closed.state, LogStreamState::Closed, "{closed:?}");
    assert!(
        closed.data.closed_at.is_some() && closed.data.retained_until.is_some(),
        "{closed:?}"
    );
    assert_eq!(series_row(&own_series).state, MetricSeriesState::Retired);
    assert_eq!(stream_row(&other_stream).state, LogStreamState::Open);
    assert_eq!(series_row(&other_series).state, MetricSeriesState::Active);

    // An instance with nothing to close or retire: no line of either telemetry binding.
    let bare = w.instance_copy(&instance_id, |row| {
        row.state = InstanceState::Running;
        row.data.job_id = None;
        row.data.task_index = None;
    });
    let seq = w.host.last_seq();
    w.exited(&bare, 0, "exited");
    let log = w.host.log_after(seq);
    for id in [
        "telemetry-close-log-streams-on-exit",
        "telemetry-retire-series-on-exit",
    ] {
        assert_eq!(results(&log, id), Vec::new(), "{id}: {log:?}");
    }
    assert_eq!(w.host.host().held(), 0);
}
