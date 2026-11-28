//! A planned release gets its rollout steps and starts them
//! (`story:cloudspec-submission-release-steps`): on a release made by
//! `weft.deployments.ApplyPlan`, `AddReleaseStep` adds a rollout step, `StartRelease` starts the
//! release and `StartReleaseStep` starts the step naming its workload. The hand-run binding
//! `r1-set-workload-replicas` then sets a Service workload's desired replica count, and drops
//! `not-a-service` for a workload of another shape. Every refusal of the four commands writes no
//! row and moves nothing.
//!
//! The workloads are the skeleton definitions ([`steps::run_definitions`]); the plan the release
//! comes from is seeded `Ready` through the storage port over those definitions, as
//! `plan_apply.rs` seeds it (who computes a plan is not this story).

use serde_json::{json, Value};
use weft_impl::context::mint;
use weft_impl::{Ports, Row};
use weft_testkit::steps::{self, Definitions, Path, DEPLOYER};
use weft_testkit::InProcess;
use weft_types::architecture::{
    ComponentWorkloadData, ComponentWorkloadSnapshot, ComponentWorkloadState,
};
use weft_types::behaviour::{ReleaseStepStorage, ReleaseStorage, ServiceStorage, WorkloadStorage};
use weft_types::deployments::{
    PlanData, PlanSnapshot, PlanState, ReleaseState, ReleaseStepState, StepKind,
};
use weft_types::pipelines::{DeployGateData, DeployGateSnapshot, DeployGateState, GateKind};
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared::{
    ComponentId, ComponentWorkloadId, DeployGateId, EnvironmentId, FormationId,
    FormationRevisionId, PlanId, PrincipalId, ReleaseId, ReleaseStepId, ScopeId, ServiceId,
    WorkloadId,
};
use weft_types::workloads::{ServiceState, ShapeKind, WorkloadState};

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";
const APPLY_PLAN: &str = "weft.deployments.ApplyPlan";
const ADD_RELEASE_STEP: &str = "weft.deployments.AddReleaseStep";
const START_RELEASE: &str = "weft.deployments.StartRelease";
const START_RELEASE_STEP: &str = "weft.deployments.StartReleaseStep";
const SET_WORKLOAD_REPLICAS: &str = "weft.workloads.SetWorkloadReplicas";
const BINDING: &str = "r1-set-workload-replicas";

/// A control host with the operator and tenant path and the skeleton definitions.
struct World {
    host: InProcess,
    path: Path,
    defs: Definitions,
}

impl World {
    fn new() -> Self {
        let host = InProcess::start();
        let path = steps::run_path(host.client());
        let defs = steps::run_definitions(host.client(), &path);
        Self { host, path, defs }
    }

    /// The caller of a request in the path's scope, with a principal of its own.
    fn caller(&self) -> Value {
        caller(&self.path.scope_id)
    }

    fn ports(&self) -> Ports {
        self.host.ports()
    }

    /// Seeds a `Ready` plan of the definitions' formation and revision in `environment_id`, and
    /// applies it as Deployer: the release's identity.
    fn release_in(&self, environment_id: &str) -> (String, String) {
        let plan_id = mint();
        self.ports().put_row(Row::Plan(PlanSnapshot {
            state: PlanState::Ready,
            data: PlanData {
                plan_id: PlanId(Uuid(plan_id.clone())),
                scope_id: ScopeId(Uuid(self.path.scope_id.clone())),
                formation_id: FormationId(Uuid(self.defs.formation_id.clone())),
                environment_id: EnvironmentId(Uuid(environment_id.to_owned())),
                formation_revision_id: FormationRevisionId(Uuid(self.defs.revision_id.clone())),
                translation_id: None,
                source_id: None,
                live_release_id: None,
                diff: None,
                simulation: None,
                created_by: PrincipalId(Uuid(mint())),
                created_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
            },
        }));
        let release_id = self.apply(&plan_id);
        (plan_id, release_id)
    }

    /// `ApplyPlan` of `plan_id` as Deployer: the release's identity.
    fn apply(&self, plan_id: &str) -> String {
        let body = self.answers(
            DEPLOYER,
            APPLY_PLAN,
            &json!({"plan_id": plan_id, "scope_id": self.path.scope_id, "strategy": "Rolling"}),
            "created",
        );
        payload_str(&body, "release_id")
    }

    /// A release of the definitions' environment.
    fn release(&self) -> String {
        self.release_in(&self.defs.environment_id).1
    }

    /// `command` as `actor` in the path's scope: the body, which answers `outcome`.
    fn answers(&self, actor: &str, command: &str, input: &Value, outcome: &str) -> Value {
        answers(&self.host, actor, &self.caller(), command, input, outcome)
    }

    /// `command` as ReleaseController in the path's scope: the body, which answers `outcome`.
    fn controller(&self, command: &str, input: &Value, outcome: &str) -> Value {
        self.answers(RELEASE_CONTROLLER, command, input, outcome)
    }

    /// `command` as ReleaseController with `caller` is refused with `outcome` and `error`, and
    /// no release, step, service or workload row changes, and no event is logged.
    fn refuses_as(&self, caller: &Value, command: &str, input: &Value, outcome: &str, error: &str) {
        let before = rows(&self.ports());
        let seq = self.host.last_seq();
        let body = answers(
            &self.host,
            RELEASE_CONTROLLER,
            caller,
            command,
            input,
            outcome,
        );
        assert!(
            body.to_string().contains(&format!(".{error}\"")),
            "{command} {input}: no error {error} in {body}"
        );
        assert_eq!(rows(&self.ports()), before, "{command} {input} wrote a row");
        assert_eq!(
            events_after(&self.host, seq),
            Vec::<String>::new(),
            "{command} {input} published"
        );
    }

    fn refuses(&self, command: &str, input: &Value, outcome: &str, error: &str) {
        self.refuses_as(&self.caller(), command, input, outcome, error);
    }

    /// `AddReleaseStep`: a rollout step of `release_id` at `position` with `replicas`.
    fn add_rollout(&self, release_id: &str, position: i64, replicas: i64) -> String {
        let body = self.controller(
            ADD_RELEASE_STEP,
            &add_step(release_id, position, "Rollout", json!(replicas)),
            "added",
        );
        payload_str(&body, "step_id")
    }

    /// The skeleton's workload row, copied under a new identity in the path's scope with `shape`
    /// and `state` and no assignment: the copy's identity.
    fn workload_copy(&self, shape: ShapeKind, state: WorkloadState) -> String {
        let ports = self.ports();
        let mut row = WorkloadStorage::get(&ports, &workload(&self.defs.workload_id))
            .expect("the skeleton workload");
        let id = mint();
        row.data.workload_id = workload(&id);
        row.data.shape = shape;
        row.state = state;
        self.ports().put_row(Row::Workload(row));
        id
    }

    /// A service of `workload_id` in state `state`, copied from the skeleton's service.
    fn service_copy(&self, workload_id: &str, state: ServiceState) {
        let ports = self.ports();
        let mut row = ServiceStorage::get(&ports, &ServiceId(Uuid(self.defs.service_id.clone())))
            .expect("the skeleton service");
        row.data.service_id = ServiceId(Uuid(mint()));
        row.data.workload_id = workload(workload_id);
        row.state = state;
        self.ports().put_row(Row::Service(row));
    }

    /// An assignment of `workload_id` to the skeleton component in `environment_id`.
    fn assign(&self, workload_id: &str, environment_id: &str) {
        self.ports()
            .put_row(Row::ComponentWorkload(ComponentWorkloadSnapshot {
                state: ComponentWorkloadState::Assigned,
                data: ComponentWorkloadData {
                    component_workload_id: ComponentWorkloadId(Uuid(mint())),
                    formation_id: FormationId(Uuid(self.defs.formation_id.clone())),
                    formation_revision_id: FormationRevisionId(Uuid(self.defs.revision_id.clone())),
                    environment_id: EnvironmentId(Uuid(environment_id.to_owned())),
                    component_id: ComponentId(Uuid(self.defs.component_id.clone())),
                    workload_id: workload(workload_id),
                },
            }));
    }

    fn service_replicas(&self) -> i64 {
        ServiceStorage::get(
            &self.ports(),
            &ServiceId(Uuid(self.defs.service_id.clone())),
        )
        .expect("the skeleton service")
        .data
        .replicas
    }

    /// The binding lines of `r1-set-workload-replicas` after `seq`.
    fn binding_lines(&self, seq: u64) -> Vec<Value> {
        self.host
            .log_after(seq)
            .into_iter()
            .filter(|line| line["kind"] == "binding" && line["binding"] == BINDING)
            .collect()
    }
}

fn workload(id: &str) -> WorkloadId {
    WorkloadId(Uuid(id.to_owned()))
}

/// A caller acting in `scope_id` with a principal of its own.
fn caller(scope_id: &str) -> Value {
    json!({"tenant_id": mint(), "principal_id": mint(), "scope_id": scope_id})
}

/// `AddReleaseStep` of `release_id` at `position`, of `kind`, component `site`, with `replicas`.
fn add_step(release_id: &str, position: i64, kind: &str, replicas: Value) -> Value {
    json!({
        "release_id": release_id,
        "position": position,
        "kind": kind,
        "component": "site",
        "replicas": replicas,
    })
}

fn start_step(step_id: &str, workload_id: Option<&str>) -> Value {
    json!({"step_id": step_id, "job_id": null, "workload_id": workload_id})
}

fn set_replicas(workload_id: &str, scope_id: &str, replicas: i64) -> Value {
    json!({"workload_id": workload_id, "scope_id": scope_id, "replicas": replicas})
}

/// `command` sent as `actor` with `caller`: the body, which answers `outcome`.
fn answers(
    host: &InProcess,
    actor: &str,
    caller: &Value,
    command: &str,
    input: &Value,
    outcome: &str,
) -> Value {
    let answer = host
        .client()
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

/// Every release, step, service and workload row, as text: a refusal leaves it as it was.
fn rows(ports: &Ports) -> String {
    format!(
        "{:?} {:?} {:?} {:?}",
        ReleaseStorage::list(ports),
        ReleaseStepStorage::list(ports),
        ServiceStorage::list(ports),
        WorkloadStorage::list(ports)
    )
}

/// The names of the event lines of the log after `seq`.
fn events_after(host: &InProcess, seq: u64) -> Vec<String> {
    host.log_after(seq)
        .into_iter()
        .filter(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str().map(str::to_owned))
        .collect()
}

fn release_state(ports: &Ports, release_id: &str) -> ReleaseState {
    ReleaseStorage::get(ports, &ReleaseId(Uuid(release_id.to_owned())))
        .expect("the release row")
        .state
}

/// Statements 1 and 2 (Service): a rollout step at position 1 with 2 replicas is added
/// `Pending`, the release starts `Applying`, the step starts `Running` naming the skeleton's
/// Service workload, `ReleaseStepStarted` carries the release's scope, the workload and the
/// replicas, and the applied `r1-set-workload-replicas` sets the service's desired count to 2.
#[test]
fn a_rollout_step_of_an_applied_plan_starts_and_sets_the_service_replicas() {
    let w = World::new();
    let release_id = w.release();
    assert_eq!(
        release_state(&w.ports(), &release_id),
        ReleaseState::Planned
    );

    let seq = w.host.last_seq();
    let body = w.controller(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 1, "Rollout", json!(2)),
        "added",
    );
    assert_eq!(
        events_after(&w.host, seq),
        vec!["weft.deployments.ReleaseStepAdded"]
    );
    let payload = &body["published"][0]["payload"];
    assert_eq!(
        (
            payload["release_id"].clone(),
            payload["position"].clone(),
            payload["kind"].clone()
        ),
        (json!(release_id), json!(1), json!("Rollout")),
        "{body}"
    );
    let step_id = payload_str(&body, "step_id");
    let step = ReleaseStepStorage::get(&w.ports(), &ReleaseStepId(Uuid(step_id.clone())))
        .expect("the step row");
    assert_eq!(step.state, ReleaseStepState::Pending, "{step:?}");
    assert_eq!(
        (
            step.data.position,
            step.data.kind,
            step.data.replicas,
            step.data.workload_id.clone()
        ),
        (1, StepKind::Rollout, Some(2), None),
        "{step:?}"
    );
    assert_eq!(step.data.release_id, ReleaseId(Uuid(release_id.clone())));

    let seq = w.host.last_seq();
    let body = w.controller(START_RELEASE, &json!({"release_id": release_id}), "started");
    assert_eq!(
        events_after(&w.host, seq),
        vec!["weft.deployments.ReleaseStarted"]
    );
    assert_eq!(
        body["published"][0]["payload"]["release_id"],
        json!(release_id)
    );
    assert_eq!(
        release_state(&w.ports(), &release_id),
        ReleaseState::Applying
    );

    assert_eq!(w.service_replicas(), 1);
    let seq = w.host.last_seq();
    let body = w.controller(
        START_RELEASE_STEP,
        &start_step(&step_id, Some(&w.defs.workload_id)),
        "started",
    );
    let payload = &body["published"][0]["payload"];
    assert_eq!(
        (
            payload["step_id"].clone(),
            payload["release_id"].clone(),
            payload["scope_id"].clone(),
            payload["position"].clone(),
            payload["kind"].clone(),
            payload["component"].clone(),
            payload["workload_id"].clone(),
            payload["replicas"].clone(),
        ),
        (
            json!(step_id),
            json!(release_id),
            json!(w.path.scope_id),
            json!(1),
            json!("Rollout"),
            json!("site"),
            json!(w.defs.workload_id),
            json!(2),
        ),
        "{body}"
    );
    let step = ReleaseStepStorage::get(&w.ports(), &ReleaseStepId(Uuid(step_id.clone())))
        .expect("the step row");
    assert_eq!(step.state, ReleaseStepState::Running, "{step:?}");
    assert_eq!(step.data.workload_id, Some(workload(&w.defs.workload_id)));
    assert!(step.data.started_at.is_some(), "{step:?}");

    let lines = w.binding_lines(seq);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["result"], json!("applied"), "{lines:?}");
    assert!(lines[0].get("refusal").is_none(), "{lines:?}");
    assert_eq!(
        events_after(&w.host, seq),
        vec![
            "weft.deployments.ReleaseStepStarted",
            "weft.workloads.ServiceScaled"
        ]
    );
    assert_eq!(w.service_replicas(), 2);
    assert_eq!(w.host.host().held(), 0);
}

/// Statement 2 (Job): the rollout step of a Job workload starts, and the binding line of
/// `r1-set-workload-replicas` reads refusal `not-a-service`, `dropped`; nothing is held and no
/// service changes.
#[test]
fn a_rollout_step_of_a_job_workload_drops_not_a_service() {
    let w = World::new();
    let job = w.workload_copy(ShapeKind::Job, WorkloadState::Defined);
    w.assign(&job, &w.defs.environment_id);
    let release_id = w.release();
    let step_id = w.add_rollout(&release_id, 1, 2);
    w.controller(START_RELEASE, &json!({"release_id": release_id}), "started");

    let services = format!("{:?}", ServiceStorage::list(&w.ports()));
    let seq = w.host.last_seq();
    w.controller(
        START_RELEASE_STEP,
        &start_step(&step_id, Some(&job)),
        "started",
    );
    let lines = w.binding_lines(seq);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(
        (lines[0]["refusal"].clone(), lines[0]["result"].clone()),
        (json!("not-a-service"), json!("dropped")),
        "{lines:?}"
    );
    assert_eq!(
        events_after(&w.host, seq),
        vec!["weft.deployments.ReleaseStepStarted"]
    );
    assert_eq!(w.host.host().held(), 0);
    assert_eq!(format!("{:?}", ServiceStorage::list(&w.ports())), services);
}

/// Statement 3, `AddReleaseStep` and `StartReleaseStep`: each refusal answers its error, writes
/// no row and moves nothing.
#[test]
fn add_and_start_release_step_refuse_without_a_row() {
    let w = World::new();
    let release_id = w.release();
    let first = w.add_rollout(&release_id, 1, 2);
    let second = w.add_rollout(&release_id, 2, 2);

    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 0, "Rollout", json!(2)),
        "invalid-position",
        "InvalidStepPosition",
    );
    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 3, "Rollout", json!(null)),
        "rollout-needs-replicas",
        "RolloutNeedsReplicas",
    );
    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 3, "Rollout", json!(-1)),
        "invalid-replicas",
        "InvalidReplicas",
    );
    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&mint(), 3, "Rollout", json!(2)),
        "no-such-release",
        "NotFound",
    );
    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 1, "PreInstall", json!(null)),
        "position-taken",
        "StepPositionTaken",
    );

    // The release is still Planned: its steps do not run.
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&first, Some(&w.defs.workload_id)),
        "release-not-applying",
        "ReleaseNotApplying",
    );
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&mint(), Some(&w.defs.workload_id)),
        "no-such-step",
        "NotFound",
    );

    w.controller(START_RELEASE, &json!({"release_id": release_id}), "started");
    w.refuses(
        ADD_RELEASE_STEP,
        &add_step(&release_id, 3, "Rollout", json!(2)),
        "release-not-planned",
        "ReleaseNotPlanned",
    );
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&first, None),
        "rollout-needs-workload",
        "RolloutNeedsWorkload",
    );
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&second, Some(&w.defs.workload_id)),
        "previous-step-incomplete",
        "PreviousStepIncomplete",
    );
    // A workload assigned only in another environment.
    let elsewhere = w.workload_copy(ShapeKind::Service, WorkloadState::Defined);
    w.assign(&elsewhere, &mint());
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&first, Some(&elsewhere)),
        "workload-outside-release",
        "OutsideRelease",
    );

    // The first step still starts.
    w.controller(
        START_RELEASE_STEP,
        &start_step(&first, Some(&w.defs.workload_id)),
        "started",
    );
    w.refuses(
        START_RELEASE_STEP,
        &start_step(&first, Some(&w.defs.workload_id)),
        "wrong-state",
        "StateConflict",
    );
}

/// Statement 3, `StartRelease`: an unknown release, a second release of a plan whose release
/// started, a release of an environment with an `Applying` release, and a release of an
/// environment with an active gate and no passed check are refused without a row or a move.
#[test]
fn start_release_refuses_without_a_row() {
    let w = World::new();
    let (plan_id, first) = w.release_in(&w.defs.environment_id);
    w.refuses(
        START_RELEASE,
        &json!({"release_id": mint()}),
        "no-such-release",
        "NotFound",
    );

    let again = w.apply(&plan_id);
    w.controller(START_RELEASE, &json!({"release_id": first}), "started");
    w.refuses(
        START_RELEASE,
        &json!({"release_id": again}),
        "already-applied",
        "PlanAlreadyApplied",
    );
    assert_eq!(release_state(&w.ports(), &again), ReleaseState::Planned);

    let (_, other) = w.release_in(&w.defs.environment_id);
    w.refuses(
        START_RELEASE,
        &json!({"release_id": other}),
        "release-in-progress",
        "ReleaseInProgress",
    );

    let gated = mint();
    w.ports().put_row(Row::DeployGate(DeployGateSnapshot {
        state: DeployGateState::Active,
        data: DeployGateData {
            deploy_gate_id: DeployGateId(Uuid(mint())),
            environment_id: EnvironmentId(Uuid(gated.clone())),
            scope_id: ScopeId(Uuid(w.path.scope_id.clone())),
            kind: GateKind::Approval,
            approver_role: None,
            signer_key_id: None,
            builder: None,
            source_repository: None,
            window_opens_at: None,
            window_closes_at: None,
            created_by: PrincipalId(Uuid(mint())),
        },
    }));
    let (_, behind_gate) = w.release_in(&gated);
    w.refuses(
        START_RELEASE,
        &json!({"release_id": behind_gate}),
        "gate-not-passed",
        "GateNotPassed",
    );
    assert_eq!(
        release_state(&w.ports(), &behind_gate),
        ReleaseState::Planned
    );
}

/// Statement 3, `SetWorkloadReplicas` sent directly: every refusal in the contract's order writes
/// no row; a count inside the bounds of an awake service is set.
#[test]
fn set_workload_replicas_refuses_each_outcome_without_a_row() {
    let w = World::new();
    let scope = w.path.scope_id.clone();
    let service = w.defs.workload_id.clone();

    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &mint(), 2),
        "not-callers-scope",
        "WorkloadNotFound",
    );
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &scope, -1),
        "negative-replicas",
        "InvalidAmount",
    );
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&mint(), &scope, 2),
        "no-workload",
        "WorkloadNotFound",
    );
    let other = mint();
    w.refuses_as(
        &caller(&other),
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &other, 2),
        "workload-of-other-scope",
        "WorkloadNotFound",
    );
    let retired = w.workload_copy(ShapeKind::Service, WorkloadState::Retired);
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&retired, &scope, 2),
        "workload-retired",
        "WorkloadIsRetired",
    );
    let job = w.workload_copy(ShapeKind::Job, WorkloadState::Defined);
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&job, &scope, 2),
        "not-a-service",
        "ShapeMismatch",
    );
    let bare = w.workload_copy(ShapeKind::Service, WorkloadState::Defined);
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&bare, &scope, 2),
        "no-service",
        "ServiceNotFound",
    );
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &scope, 0),
        "below-bounds",
        "ReplicasOutOfBounds",
    );
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &scope, 7),
        "above-bounds",
        "ReplicasOutOfBounds",
    );
    let asleep = w.workload_copy(ShapeKind::Service, WorkloadState::Defined);
    w.service_copy(&asleep, ServiceState::Asleep);
    w.refuses(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&asleep, &scope, 2),
        "not-awake",
        "StateConflict",
    );

    let seq = w.host.last_seq();
    let body = w.controller(
        SET_WORKLOAD_REPLICAS,
        &set_replicas(&service, &scope, 3),
        "set",
    );
    assert_eq!(
        events_after(&w.host, seq),
        vec!["weft.workloads.ServiceScaled"]
    );
    assert_eq!(
        body["published"][0]["payload"],
        json!({"service_id": w.defs.service_id, "replicas": 3}),
        "{body}"
    );
    assert_eq!(w.service_replicas(), 3);
}
