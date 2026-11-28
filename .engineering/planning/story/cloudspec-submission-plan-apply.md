---
format: aep.planning-md/3
id: story:cloudspec-submission-plan-apply
kind: story
status: active
title: RecordPlanResult moves a simulated plan to Ready or Blocked, and ApplyPlan makes a Ready plan a Planned release, each refusal tested
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
scope:
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/apply_plan.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/record_plan_result.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/plan_apply.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T13:56:42Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T13:56:42Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3, second half (the apply path), first step: a plan
gets its result and becomes a release. Takes part of outcome 3 of the archived
`story:cloudspec-submission-plan`. No dependency: both commands and the `Plan` and `Release`
entities are in `ess/` at integration/2026-11-28-6 (db91440); the test seeds its plans.

## Context

Read on integration/2026-11-28-6 (db91440):

- `weft.deployments.RecordPlanResult` (`ess/cloud/11-translation/deployments.yaml:1512`) and
  `weft.deployments.ApplyPlan` (`ess/6-delivery/deployments.yaml:487`) answer 501: no file of
  `product/cloud/crates/weft-impl/src/behaviours/` names them, so `weft-impl/build.rs` writes the
  `UnmetObligation` stub. A behaviour file registers itself by its name; no other file changes.
- `RecordPlanResult` is granted to `weft.actors.ReleaseController` (`ess/9-actors/actors.yaml:969`),
  `ApplyPlan` to the Deployer. Plan lifecycle `Simulating -> Ready | Blocked`, `Discarded`.
- Who computes the diff and the simulation of a plan is not this story: the caller sends them
  (a test sends a passing simulation with no violations as the ReleaseController).

## Outcome

1. `product/cloud/crates/weft-impl/src/behaviours/record_plan_result.rs`: the outcomes in the order
   of the contract: `no-such-plan`, `blocked` (violations > 0: `Blocked`, `PlanBlocked` with the
   violations), `passed` (`Ready`, `PlanSimulated`), `wrong-state`; both moves set
   `live_release_id`, `diff` and `simulation`.
2. `product/cloud/crates/weft-impl/src/behaviours/apply_plan.rs`: `not-callers-account`,
   `no-such-plan`, `plan-of-other-account`, `simulation-violations`, `plan-not-ready`, `created`:
   a `Release` (state `Planned`) with the plan scope, formation, environment and revision, the
   input strategy, `created_by` the caller, and `ReleaseCreated`. The plan stays `Ready`.

## Domain relations

- Release -> Plan, references one: `ess/6-delivery/deployments.yaml`, entity
  `weft.deployments.Release`, relation `plan`.
- Release -> Environment, FormationRevision, Formation, Scope, references one each, copied from the
  plan: same entity, relations `environment`, `formation_revision`, `formation`, `account`.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/record_plan_result.rs` (new),
`product/cloud/crates/weft-impl/src/behaviours/apply_plan.rs` (new),
`product/cloud/crates/weft-testkit/tests/plan_apply.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line).

Shared: `weft-testkit/tests/main.rs`, one `mod` line each, with `story:cloudspec-submission-submit`,
`story:cloudspec-submission-plan-from-source` and every story of this set. No other file.

## Acceptance

On a seeded `Simulating` plan, `RecordPlanResult` with no violations moves it to `Ready` and
`ApplyPlan` then creates one `Planned` release of that plan, and every refusal of both commands is
answered with its error and writes no row.

Checked by:

1. passed: plan `Ready` with the diff and simulation sent; a `PlanSimulated` event line.
2. blocked: one violation -> plan `Blocked`, a `PlanBlocked` line naming it; `ApplyPlan` on it ->
   `SimulationViolations`, no release.
3. created: the release row has the plan scope, formation, environment and revision, the strategy
   sent, `created_by` the caller, state `Planned`; a `ReleaseCreated` line.
4. Refusals, each with no new row: unknown plan -> `NotFound` (both commands); a `Discarded` plan
   -> `StateConflict` (`RecordPlanResult`); `ApplyPlan` with another `scope_id` -> `NotFound`; a plan
   of another scope -> `NotFound`; a `Simulating` plan -> `PlanNotReady`.
5. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
