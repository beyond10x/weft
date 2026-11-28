//! A plan gets its result and becomes a release (`story:cloudspec-submission-plan-apply`):
//! `weft.deployments.RecordPlanResult` moves a seeded `Simulating` plan to `Ready` or `Blocked`,
//! and `weft.deployments.ApplyPlan` makes a `Ready` plan a `Planned` release. Every refusal of
//! both commands answers its error and writes no row. The plans are seeded through the storage
//! port; who computes a plan's diff and simulation is not this story, so the test sends them.

use serde_json::{json, Value};
use weft_impl::context::mint;
use weft_impl::{Ports, Row};
use weft_testkit::steps::DEPLOYER;
use weft_testkit::InProcess;
use weft_types::behaviour::{PlanStorage, ReleaseStorage};
use weft_types::deployments::{
    PlanData, PlanDiff, PlanSnapshot, PlanState, ReleaseState, RolloutStrategy, Simulation,
    SimulationCheck, Violation, ViolationName,
};
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared::{
    ComponentName, EnvironmentId, FormationId, FormationRevisionId, PlanId, PrincipalId, ReleaseId,
    ScopeId,
};

const RELEASE_CONTROLLER: &str = "weft.actors.ReleaseController";
const RECORD_PLAN_RESULT: &str = "weft.deployments.RecordPlanResult";
const APPLY_PLAN: &str = "weft.deployments.ApplyPlan";

/// The identities of one seeded plan.
#[derive(Clone)]
struct Seeded {
    plan_id: String,
    scope_id: String,
    formation_id: String,
    environment_id: String,
    revision_id: String,
}

/// A plan of a new scope, formation, environment and revision in state `state`: its identities
/// and its row.
fn plan(state: PlanState) -> (Seeded, Row) {
    let seeded = Seeded {
        plan_id: mint(),
        scope_id: mint(),
        formation_id: mint(),
        environment_id: mint(),
        revision_id: mint(),
    };
    let row = Row::Plan(PlanSnapshot {
        state,
        data: PlanData {
            plan_id: PlanId(Uuid(seeded.plan_id.clone())),
            scope_id: ScopeId(Uuid(seeded.scope_id.clone())),
            formation_id: FormationId(Uuid(seeded.formation_id.clone())),
            environment_id: EnvironmentId(Uuid(seeded.environment_id.clone())),
            formation_revision_id: FormationRevisionId(Uuid(seeded.revision_id.clone())),
            translation_id: None,
            source_id: None,
            live_release_id: None,
            diff: None,
            simulation: None,
            created_by: PrincipalId(Uuid(mint())),
            created_at: Timestamp("2026-11-28T12:00:00Z".to_owned()),
        },
    });
    (seeded, row)
}

/// A caller acting in `scope_id` as principal `principal_id`.
fn caller(scope_id: &str, principal_id: &str) -> Value {
    json!({"tenant_id": mint(), "principal_id": principal_id, "scope_id": scope_id})
}

/// The diff every request of this test sends.
fn diff() -> Value {
    json!({"added": ["site"], "changed": [], "removed": []})
}

/// The one violation of a blocked simulation.
fn violation() -> Value {
    json!({
        "check": "Placement",
        "name": "ExceedsCapacity",
        "component": "site",
        "subject": "zone z1",
    })
}

/// `RecordPlanResult` of plan `plan_id` with the violations `violations`.
fn record(plan_id: &str, violations: &[Value]) -> Value {
    json!({
        "plan_id": plan_id,
        "live_release_id": null,
        "diff": diff(),
        "simulation": {"reachability": [], "violations": violations},
    })
}

/// `ApplyPlan` of plan `plan_id` in `scope_id`, rolling.
fn apply(plan_id: &str, scope_id: &str) -> Value {
    json!({"plan_id": plan_id, "scope_id": scope_id, "strategy": "Rolling"})
}

/// Every plan and release row, as text: a refusal leaves it as it was.
fn rows(ports: &Ports) -> String {
    format!(
        "{:?} {:?}",
        PlanStorage::list(ports),
        ReleaseStorage::list(ports)
    )
}

fn plan_row(ports: &Ports, plan_id: &str) -> PlanSnapshot {
    PlanStorage::get(ports, &PlanId(Uuid(plan_id.to_owned()))).expect("the plan row")
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

/// `command` is refused with `outcome` and the error `error`, and no plan or release row changes.
fn refuses(
    host: &InProcess,
    actor: &str,
    caller: &Value,
    command: &str,
    input: &Value,
    outcome: &str,
    error: &str,
) {
    let before = rows(&host.ports());
    let body = answers(host, actor, caller, command, input, outcome);
    assert!(
        body.to_string()
            .contains(&format!("\"weft.deployments.{error}\"")),
        "{command} {input}: no error {error} in {body}"
    );
    assert_eq!(rows(&host.ports()), before, "{command} {input} wrote a row");
}

/// The names of the event lines of the log after `seq`.
fn events_after(host: &InProcess, seq: u64) -> Vec<String> {
    host.log_after(seq)
        .into_iter()
        .filter(|line| line["kind"] == "event")
        .filter_map(|line| line["name"].as_str().map(str::to_owned))
        .collect()
}

/// Statements 1 and 3: a passing simulation moves the plan to `Ready` with the diff and the
/// simulation sent, and `ApplyPlan` then creates one `Planned` release of the plan with its
/// scope, formation, environment and revision, the strategy sent and the caller as creator. The
/// plan stays `Ready`.
#[test]
fn a_passing_plan_becomes_ready_and_apply_plan_creates_one_planned_release_of_it() {
    let (seeded, row) = plan(PlanState::Simulating);
    let host = InProcess::start_seeded(vec![row]);
    let controller = caller(&seeded.scope_id, &mint());

    // The release live in the environment before this plan, which the plan row records.
    let live = mint();
    let mut passed = record(&seeded.plan_id, &[]);
    passed["live_release_id"] = json!(live);
    let seq = host.last_seq();
    let body = answers(
        &host,
        RELEASE_CONTROLLER,
        &controller,
        RECORD_PLAN_RESULT,
        &passed,
        "passed",
    );
    assert_eq!(
        body["published"][0]["payload"]["plan_id"],
        json!(seeded.plan_id),
        "{body}"
    );
    assert_eq!(
        events_after(&host, seq),
        vec!["weft.deployments.PlanSimulated"]
    );
    let ready = plan_row(&host.ports(), &seeded.plan_id);
    assert_eq!(ready.state, PlanState::Ready, "{ready:?}");
    assert_eq!(
        ready.data.diff,
        Some(PlanDiff {
            added: vec![ComponentName("site".to_owned())],
            changed: Vec::new(),
            removed: Vec::new(),
        }),
        "{ready:?}"
    );
    assert_eq!(
        ready.data.simulation,
        Some(Simulation {
            reachability: Vec::new(),
            violations: Vec::new(),
        }),
        "{ready:?}"
    );
    assert_eq!(
        ready.data.live_release_id,
        Some(ReleaseId(Uuid(live))),
        "{ready:?}"
    );

    let principal = mint();
    let deployer = caller(&seeded.scope_id, &principal);
    let seq = host.last_seq();
    let body = answers(
        &host,
        DEPLOYER,
        &deployer,
        APPLY_PLAN,
        &apply(&seeded.plan_id, &seeded.scope_id),
        "created",
    );
    assert_eq!(
        events_after(&host, seq),
        vec!["weft.deployments.ReleaseCreated"]
    );
    let payload = &body["published"][0]["payload"];
    assert_eq!(
        (
            payload["plan_id"].clone(),
            payload["environment_id"].clone(),
            payload["strategy"].clone()
        ),
        (
            json!(seeded.plan_id),
            json!(seeded.environment_id),
            json!("Rolling")
        ),
        "{body}"
    );
    let release_id = payload["release_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no release_id in {body}"))
        .to_owned();
    let ports = host.ports();
    let releases = ReleaseStorage::list(&ports);
    assert_eq!(releases.len(), 1, "{releases:?}");
    let release = &releases[0];
    assert_eq!(release.state, ReleaseState::Planned, "{release:?}");
    let data = &release.data;
    assert_eq!(data.release_id, ReleaseId(Uuid(release_id)), "{release:?}");
    assert_eq!(
        (
            &data.plan_id.0 .0,
            &data.scope_id.0 .0,
            &data.formation_id.0 .0,
            &data.environment_id.0 .0,
            &data.formation_revision_id.0 .0,
        ),
        (
            &seeded.plan_id,
            &seeded.scope_id,
            &seeded.formation_id,
            &seeded.environment_id,
            &seeded.revision_id,
        ),
        "{release:?}"
    );
    assert_eq!(data.strategy, RolloutStrategy::Rolling, "{release:?}");
    assert_eq!(data.created_by, PrincipalId(Uuid(principal)), "{release:?}");
    assert_eq!(
        (&data.rollback_of, &data.live_at, &data.failure),
        (&None, &None, &None),
        "{release:?}"
    );
    let plan = plan_row(&ports, &seeded.plan_id);
    assert_eq!(plan.state, PlanState::Ready, "{plan:?}");
}

/// Statement 2: one violation blocks the plan and names it in `PlanBlocked`; `ApplyPlan` on
/// the blocked plan answers `SimulationViolations` and creates no release.
#[test]
fn a_violation_blocks_the_plan_and_apply_plan_refuses_it_with_simulation_violations() {
    let (seeded, row) = plan(PlanState::Simulating);
    let host = InProcess::start_seeded(vec![row]);
    let controller = caller(&seeded.scope_id, &mint());

    let seq = host.last_seq();
    let body = answers(
        &host,
        RELEASE_CONTROLLER,
        &controller,
        RECORD_PLAN_RESULT,
        &record(&seeded.plan_id, &[violation()]),
        "blocked",
    );
    assert_eq!(
        body["published"][0]["payload"]["violations"],
        json!([violation()]),
        "{body}"
    );
    assert_eq!(
        events_after(&host, seq),
        vec!["weft.deployments.PlanBlocked"]
    );
    let blocked = plan_row(&host.ports(), &seeded.plan_id);
    assert_eq!(blocked.state, PlanState::Blocked, "{blocked:?}");
    assert_eq!(blocked.data.live_release_id, None, "{blocked:?}");
    assert_eq!(
        blocked
            .data
            .simulation
            .map(|simulation| simulation.violations),
        Some(vec![Violation {
            check: SimulationCheck::Placement,
            name: ViolationName::ExceedsCapacity,
            component: Some(ComponentName("site".to_owned())),
            subject: "zone z1".to_owned(),
        }])
    );
    assert_eq!(
        blocked.data.diff.map(|diff| diff.added),
        Some(vec![ComponentName("site".to_owned())])
    );

    refuses(
        &host,
        DEPLOYER,
        &caller(&seeded.scope_id, &mint()),
        APPLY_PLAN,
        &apply(&seeded.plan_id, &seeded.scope_id),
        "simulation-violations",
        "SimulationViolations",
    );
    assert!(ReleaseStorage::list(&host.ports()).is_empty());
}

/// Statement 4: every refusal of `RecordPlanResult` answers its error and writes no row: an
/// unknown plan, a plan of another scope than the caller's, and a `Discarded` plan.
#[test]
fn record_plan_result_refuses_an_unknown_a_foreign_and_a_discarded_plan_without_a_row() {
    let (simulating, simulating_row) = plan(PlanState::Simulating);
    let (discarded, discarded_row) = plan(PlanState::Discarded);
    let host = InProcess::start_seeded(vec![simulating_row, discarded_row]);
    let controller = caller(&simulating.scope_id, &mint());

    refuses(
        &host,
        RELEASE_CONTROLLER,
        &controller,
        RECORD_PLAN_RESULT,
        &record(&mint(), &[]),
        "no-such-plan",
        "NotFound",
    );
    refuses(
        &host,
        RELEASE_CONTROLLER,
        &caller(&mint(), &mint()),
        RECORD_PLAN_RESULT,
        &record(&simulating.plan_id, &[]),
        "not-callers-scope",
        "NotFound",
    );
    for violations in [Vec::new(), vec![violation()]] {
        refuses(
            &host,
            RELEASE_CONTROLLER,
            &caller(&discarded.scope_id, &mint()),
            RECORD_PLAN_RESULT,
            &record(&discarded.plan_id, &violations),
            "wrong-state",
            "StateConflict",
        );
    }
}

/// Statement 4: every refusal of `ApplyPlan` answers its error and writes no row: a scope that is
/// not the caller's, an unknown plan, a plan of another scope, and a `Simulating` plan.
#[test]
fn apply_plan_refuses_a_foreign_scope_an_unknown_plan_a_foreign_plan_and_an_unready_plan() {
    let (own, own_row) = plan(PlanState::Simulating);
    let (foreign, foreign_row) = plan(PlanState::Ready);
    let host = InProcess::start_seeded(vec![own_row, foreign_row]);
    let deployer = caller(&own.scope_id, &mint());

    refuses(
        &host,
        DEPLOYER,
        &deployer,
        APPLY_PLAN,
        &apply(&own.plan_id, &mint()),
        "not-callers-account",
        "NotFound",
    );
    refuses(
        &host,
        DEPLOYER,
        &deployer,
        APPLY_PLAN,
        &apply(&mint(), &own.scope_id),
        "no-such-plan",
        "NotFound",
    );
    refuses(
        &host,
        DEPLOYER,
        &deployer,
        APPLY_PLAN,
        &apply(&foreign.plan_id, &own.scope_id),
        "plan-of-other-account",
        "NotFound",
    );
    refuses(
        &host,
        DEPLOYER,
        &deployer,
        APPLY_PLAN,
        &apply(&own.plan_id, &own.scope_id),
        "plan-not-ready",
        "PlanNotReady",
    );
    assert!(ReleaseStorage::list(&host.ports()).is_empty());
}
