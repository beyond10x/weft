//! Applying the plan of a cloudspec source defines, assigns and rolls out its snippet parts
//! (`story:cloudspec-submission-apply-workloads`, owner decision D-C1 = A): the control host takes
//! `POST /cloudspec/plans/<plan_id>/apply` and, in one unit, applies the plan, adds one component
//! per snippet part, defines its workload with weft's fixed snippet profile, assigns it to the
//! plan's revision and environment, adds a rollout step per part, starts the release and its
//! first step. A refusal of any command commits nothing. The goldens are the committed copies of
//! `tests/fixtures/cloudspec/`; no network, no cloudspec binary.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use weft_client::Client;
use weft_impl::context::mint;
use weft_impl::{Ports, Row};
use weft_testkit::steps::{self, DEPLOYER};
use weft_testkit::ControlHost;
use weft_types::behaviour::{
    ComponentStorage, ComponentWorkloadStorage, PlanStorage, ReleaseStepStorage, ReleaseStorage,
    WorkloadStorage,
};
use weft_types::deployments::{
    PlanData, PlanSnapshot, PlanState, ReleaseState, ReleaseStepState, StepKind,
};
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared::{
    EnvironmentId, FormationId, FormationRevisionId, IsolationLevel, PlanId, PrincipalId, ScopeId,
};
use weft_types::workloads::{PackageKind, ProcessRole, RestartMode, ShapeKind, ShapeSettings};

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";
const PLAN_FROM_SOURCE: &str = "weft.deployments.PlanFromSource";
const RECORD_PLAN_RESULT: &str = "weft.deployments.RecordPlanResult";

/// The bytes of one committed golden, by its short name.
fn golden(name: &str) -> Vec<u8> {
    let path =
        weft_testkit::repo_root().join(format!("tests/fixtures/cloudspec/{name}.submission.json"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A control host with the operator and tenant path (its quota of 64 cpu, the priority class
/// `standard`), formation `site` of the path's scope, environment `prod` of it, the skeleton's
/// `Snippet` artifact and, with `runtime`, a registered runtime `snippet` of `Snippet`.
struct World {
    host: ControlHost,
    client: Client,
    caller: Value,
    scope_id: String,
    formation_id: String,
    environment_id: String,
    runtime_id: Option<String>,
}

impl World {
    fn new(runtime: bool) -> Self {
        let host = ControlHost::start();
        let client = host.client();
        let path = steps::run_path(&client);
        let mut caller = steps::path_caller(&path);
        caller["principal_id"] = json!(mint());
        let scope_id = path.scope_id.clone();
        let formation_id = steps::create_formation(&client, &caller, &scope_id);
        let environment_id = steps::create_environment(&client, &caller, &formation_id, &scope_id);
        steps::register_artifact(&client, &caller, &scope_id);
        let runtime_id = runtime.then(|| steps::register_runtime(&client, &["Snippet"]));
        Self {
            host,
            client,
            caller,
            scope_id,
            formation_id,
            environment_id,
            runtime_id,
        }
    }

    /// Posts `bytes` to `/cloudspec/submissions` for the formation: the source's identity.
    fn submit(&self, bytes: &[u8]) -> String {
        let answer = self
            .client
            .post_bytes_as(
                &format!("/cloudspec/submissions?formation_id={}", self.formation_id),
                bytes,
                DEPLOYER,
                self.caller.clone(),
            )
            .expect("the control host answers");
        assert_eq!(answer.status, 202, "{}", answer.body);
        payload(&answer.body, "source_id")
    }

    /// `PlanFromSource` of `source_id` for the environment: the plan's identity, `Simulating`.
    fn plan(&self, source_id: &str) -> String {
        let answer = self
            .client
            .command(
                PLAN_FROM_SOURCE,
                DEPLOYER,
                self.caller.clone(),
                json!({
                    "source_id": source_id,
                    "scope_id": self.scope_id,
                    "environment_id": self.environment_id,
                }),
            )
            .expect("the control host answers");
        assert_eq!(answer.body["outcome"], json!("planned"), "{}", answer.body);
        payload(&answer.body, "plan_id")
    }

    /// `RecordPlanResult` of `plan_id` with no violation: the plan is `Ready`.
    fn ready(&self, plan_id: &str) {
        let answer = self
            .client
            .command(
                RECORD_PLAN_RESULT,
                RELEASE_CONTROLLER,
                self.caller.clone(),
                json!({
                    "plan_id": plan_id,
                    "live_release_id": null,
                    "diff": {"added": ["hello"], "changed": [], "removed": []},
                    "simulation": {"reachability": [], "violations": []},
                }),
            )
            .expect("the control host answers");
        assert_eq!(answer.body["outcome"], json!("passed"), "{}", answer.body);
    }

    /// A `Ready` plan of `bytes` submitted and planned for the environment.
    fn ready_plan(&self, bytes: &[u8]) -> String {
        let plan_id = self.plan(&self.submit(bytes));
        self.ready(&plan_id);
        plan_id
    }

    /// `POST /cloudspec/plans/<plan_id>/apply` with strategy `Replace`, as the Deployer.
    fn apply(&self, plan_id: &str) -> weft_client::Answer {
        self.client
            .post_bytes_as(
                &format!("/cloudspec/plans/{plan_id}/apply"),
                json!({"strategy": "Replace"}).to_string().as_bytes(),
                DEPLOYER,
                self.caller.clone(),
            )
            .expect("the control host answers")
    }

    /// The same world on a control host restarted after `rows` were written into its state file.
    fn reseeded(mut self, rows: Vec<Row>) -> Self {
        self.host.stop();
        weft_testkit::seed(&self.host.state_path(), rows);
        let host = self.host.restart();
        Self {
            client: host.client(),
            host,
            ..self
        }
    }

    /// Stops the host: its rows, through the storage port.
    fn rows(&mut self) -> Ports {
        self.host.stop();
        Ports::open(&self.host.state_path()).expect("the state file opens")
    }
}

/// The text member `field` of the first published event's payload.
fn payload(body: &Value, field: &str) -> String {
    body["published"][0]["payload"][field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {body}"))
        .to_owned()
}

/// The names of the event lines of `lines`.
fn events(lines: &[Value]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str().map(str::to_owned))
        .collect()
}

/// No release, step, component, workload or assignment is held.
fn nothing_written(ports: &Ports, what: &str) {
    assert_eq!(ReleaseStorage::list(ports), Vec::new(), "{what}: a release");
    assert_eq!(
        ReleaseStepStorage::list(ports),
        Vec::new(),
        "{what}: a step"
    );
    assert_eq!(
        ComponentStorage::list(ports),
        Vec::new(),
        "{what}: a component"
    );
    assert_eq!(
        WorkloadStorage::list(ports),
        Vec::new(),
        "{what}: a workload"
    );
    assert_eq!(
        ComponentWorkloadStorage::list(ports),
        Vec::new(),
        "{what}: an assignment"
    );
}

/// The canonical JSON of `value`: object keys sorted, no white space (as weft-cloudspec writes
/// the text a digest is taken of).
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<&String, String> = map
                .iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            let members: Vec<String> = sorted
                .into_iter()
                .map(|(key, value)| format!("{}:{value}", Value::String(key.clone())))
                .collect();
            format!("{{{}}}", members.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        scalar => scalar.to_string(),
    }
}

/// `sha256:<hex>` of `text`, by the host's `sha256sum`.
fn sha256(text: &str) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(text.as_bytes())
        .expect("sha256sum reads");
    let output = child.wait_with_output().expect("sha256sum ends");
    assert!(output.status.success(), "sha256sum: {:?}", output.status);
    let hex = String::from_utf8(output.stdout).expect("UTF-8");
    format!(
        "sha256:{}",
        hex.split_whitespace().next().expect("a digest")
    )
}

/// The snippet golden with a second snippet part `bye`, which its coverage marks `refused`, and
/// the architecture digest of the copy.
fn with_refused_bye() -> Vec<u8> {
    let mut doc: Value = serde_json::from_slice(&golden("snippet")).expect("JSON");
    doc["architecture"]["parts"]["bye"] = json!({
        "block": "workloads",
        "command": ["sh", "-c", "echo bye"],
        "environment": {},
        "in": "jobs",
        "kind": "snippet",
    });
    let entries = doc["coverage"]["steps"][0]["entries"]
        .as_array_mut()
        .expect("coverage entries");
    entries.push(json!({
        "note": "a snippet this generator does not map (bye)",
        "object": "parts.bye",
        "status": "refused",
    }));
    doc["digests"]["architecture"] = json!(sha256(&canonical(&doc["architecture"])));
    serde_json::to_vec_pretty(&doc).expect("bytes")
}

/// Whether `wanted` appear in `names` in this order (others between them).
fn in_order(names: &[String], wanted: &[&str]) -> bool {
    let mut rest = names.iter();
    wanted.iter().all(|name| rest.any(|seen| seen == name))
}

/// Acceptance 1: the `Ready` plan of the submitted snippet golden is applied. One release of the
/// plan, `Applying`; one component `hello` of the formation; one workload with the snippet
/// profile and the part's command and environment; one assignment to the plan's revision and
/// environment; one rollout step at position 1 with 1 replica, `Running` with that workload; and
/// the event lines `ReleaseCreated`, `WorkloadDefined`, `WorkloadAssigned`, `ReleaseStepStarted`.
#[test]
fn applying_the_ready_plan_of_the_snippet_golden_rolls_out_one_snippet_job() {
    let mut w = World::new(true);
    let plan_id = w.ready_plan(&golden("snippet"));
    let seq = w.host.last_seq();
    let answer = w.apply(&plan_id);
    assert_eq!(answer.status, 202, "{}", answer.body);
    let lines = w.host.log_after(seq);
    let names = events(&lines);
    assert!(
        in_order(
            &names,
            &[
                "weft.deployments.ReleaseCreated",
                "weft.workloads.WorkloadDefined",
                "weft.architecture.WorkloadAssigned",
                "weft.deployments.ReleaseStepStarted",
            ]
        ),
        "{names:?}"
    );

    let runtime_id = w.runtime_id.clone().expect("a runtime");
    let ports = w.rows();
    let plan = PlanStorage::get(&ports, &PlanId(Uuid(plan_id.clone()))).expect("the plan");

    let releases = ReleaseStorage::list(&ports);
    assert_eq!(releases.len(), 1, "{releases:?}");
    assert_eq!(releases[0].state, ReleaseState::Applying, "{releases:?}");
    assert_eq!(releases[0].data.plan_id.0 .0, plan_id, "{releases:?}");
    assert_eq!(
        answer.body["release_id"],
        json!(releases[0].data.release_id.0 .0)
    );

    let components = ComponentStorage::list(&ports);
    assert_eq!(components.len(), 1, "{components:?}");
    assert_eq!(components[0].data.name.0, "hello", "{components:?}");
    assert_eq!(components[0].data.formation_id.0 .0, w.formation_id);

    let workloads = WorkloadStorage::list(&ports);
    assert_eq!(workloads.len(), 1, "{workloads:?}");
    let workload = &workloads[0].data;
    assert_eq!(workload.scope_id.0 .0, w.scope_id);
    assert_eq!(workload.component.0, "hello");
    assert_eq!(workload.package_kind, PackageKind::Snippet);
    assert_eq!(workload.artifact_digest.0, steps::input::ARTIFACT_DIGEST);
    assert_eq!(workload.runtime_id.0 .0, runtime_id);
    assert_eq!(workload.isolation, IsolationLevel::Container);
    assert_eq!(workload.shape, ShapeKind::Job);
    let ShapeSettings::Job(settings) = &workload.settings else {
        panic!("not Job settings: {workload:?}");
    };
    assert_eq!(
        (settings.parallelism, settings.gang, settings.max_retries),
        (1, false, 0),
        "{settings:?}"
    );
    assert_eq!(workload.restart_policy.mode, RestartMode::Never);
    assert_eq!(workload.priority.0, "standard");
    assert_eq!(workload.claims.len(), 1, "{:?}", workload.claims);
    assert_eq!(
        (
            workload.claims[0].class.0.as_str(),
            workload.claims[0].amount
        ),
        ("cpu", 1)
    );
    assert_eq!(workload.processes.len(), 1, "{:?}", workload.processes);
    let process = &workload.processes[0];
    assert_eq!(process.role, ProcessRole::Main);
    assert_eq!(process.command, vec!["sh", "-c", "echo hello"]);
    assert_eq!(
        process.environment,
        BTreeMap::from([("GREETING".to_owned(), "hello".to_owned())])
    );

    let assignments = ComponentWorkloadStorage::list(&ports);
    assert_eq!(assignments.len(), 1, "{assignments:?}");
    let assignment = &assignments[0].data;
    assert_eq!(assignment.workload_id, workload.workload_id);
    assert_eq!(assignment.component_id, components[0].data.component_id);
    assert_eq!(
        assignment.formation_revision_id,
        plan.data.formation_revision_id
    );
    assert_eq!(assignment.environment_id.0 .0, w.environment_id);

    let steps = ReleaseStepStorage::list(&ports);
    assert_eq!(steps.len(), 1, "{steps:?}");
    let step = &steps[0];
    assert_eq!(step.state, ReleaseStepState::Running, "{step:?}");
    assert_eq!(
        (
            step.data.position,
            step.data.kind,
            step.data.replicas,
            step.data.component.as_ref().map(|name| name.0.as_str()),
        ),
        (1, StepKind::Rollout, Some(1), Some("hello")),
        "{step:?}"
    );
    assert_eq!(step.data.workload_id.as_ref(), Some(&workload.workload_id));
    assert_eq!(step.data.release_id, releases[0].data.release_id);
}

/// Acceptance 2: a plan that is not `Ready` answers `ApplyPlan`'s `plan-not-ready`; with no
/// runtime named `snippet`, `DefineWorkload`'s `no-runtime`. Each leaves no release, component,
/// workload, assignment or step, and logs no event.
#[test]
fn a_refusal_inside_the_unit_leaves_no_row() {
    let mut w = World::new(true);
    let simulating = w.plan(&w.submit(&golden("snippet")));
    let seq = w.host.last_seq();
    let answer = w.apply(&simulating);
    assert_eq!(
        (
            answer.status,
            answer.body["outcome"].clone(),
            answer.body["error"].clone()
        ),
        (
            409,
            json!("plan-not-ready"),
            json!("weft.deployments.PlanNotReady")
        ),
        "{}",
        answer.body
    );
    assert_eq!(events(&w.host.log_after(seq)), Vec::<String>::new());
    nothing_written(&w.rows(), "a plan not Ready");

    let mut w = World::new(false);
    let plan_id = w.ready_plan(&golden("snippet"));
    let seq = w.host.last_seq();
    let answer = w.apply(&plan_id);
    assert_eq!(
        (
            answer.status,
            answer.body["outcome"].clone(),
            answer.body["error"].clone()
        ),
        (
            409,
            json!("no-runtime"),
            json!("weft.workloads.RuntimeNotFound")
        ),
        "{}",
        answer.body
    );
    let lines = w.host.log_after(seq);
    assert_eq!(events(&lines), Vec::<String>::new(), "{lines:?}");
    assert!(
        lines.iter().any(|line| line["kind"] == "request"
            && line["operation"] == "weft.workloads.DefineWorkload"),
        "{lines:?}"
    );
    nothing_written(&w.rows(), "no runtime snippet");
}

/// Acceptance 3: a `Ready` plan that names no source answers `422`
/// `{"refusal": "PlanNotFromCloudspec"}`, runs no command and writes nothing.
#[test]
fn a_plan_with_no_source_is_refused_before_any_command() {
    let w = World::new(true);
    let plan_id = mint();
    let row = Row::Plan(PlanSnapshot {
        state: PlanState::Ready,
        data: PlanData {
            plan_id: PlanId(Uuid(plan_id.clone())),
            scope_id: ScopeId(Uuid(w.scope_id.clone())),
            formation_id: FormationId(Uuid(w.formation_id.clone())),
            environment_id: EnvironmentId(Uuid(w.environment_id.clone())),
            formation_revision_id: FormationRevisionId(Uuid(mint())),
            translation_id: None,
            source_id: None,
            live_release_id: None,
            diff: None,
            simulation: None,
            created_by: PrincipalId(Uuid(mint())),
            created_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
        },
    });
    let mut w = w.reseeded(vec![row]);
    let seq = w.host.last_seq();
    let answer = w.apply(&plan_id);
    assert_eq!(
        (answer.status, answer.body.clone()),
        (422, json!({"refusal": "PlanNotFromCloudspec"}))
    );
    let lines = w.host.log_after(seq);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(
        (
            lines[0]["kind"].clone(),
            lines[0]["status"].clone(),
            lines[0].get("operation").cloned().unwrap_or(Value::Null)
        ),
        (json!("request"), json!(422), Value::Null),
        "{lines:?}"
    );
    nothing_written(&w.rows(), "a plan with no source");
}

/// Acceptance 4: a copy of the golden with a second snippet part `bye` that its coverage marks
/// `refused` defines, assigns and steps only `hello`.
#[test]
fn a_part_the_coverage_refuses_is_not_rolled_out() {
    let mut w = World::new(true);
    let plan_id = w.ready_plan(&with_refused_bye());
    let answer = w.apply(&plan_id);
    assert_eq!(answer.status, 202, "{}", answer.body);
    let ports = w.rows();
    let names: Vec<String> = ComponentStorage::list(&ports)
        .into_iter()
        .map(|row| row.data.name.0)
        .collect();
    assert_eq!(names, vec!["hello"]);
    let workloads = WorkloadStorage::list(&ports);
    assert_eq!(workloads.len(), 1, "{workloads:?}");
    assert_eq!(workloads[0].data.component.0, "hello");
    assert_eq!(ComponentWorkloadStorage::list(&ports).len(), 1);
    let steps = ReleaseStepStorage::list(&ports);
    assert_eq!(steps.len(), 1, "{steps:?}");
    assert_eq!(
        steps[0].data.workload_id,
        Some(workloads[0].data.workload_id.clone())
    );
}

/// The canonical form agrees with the golden's declared architecture digest, so the copy of
/// [`with_refused_bye`] is read as a submission.
#[test]
fn the_canonical_digest_matches_the_golden() {
    let doc: Value = serde_json::from_slice(&golden("snippet")).expect("JSON");
    assert_eq!(
        json!(sha256(&canonical(&doc["architecture"]))),
        doc["digests"]["architecture"]
    );
}
