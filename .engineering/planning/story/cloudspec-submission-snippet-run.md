---
format: aep.planning-md/3
id: story:cloudspec-submission-snippet-run
kind: story
status: draft
title: The snippet golden, submitted, planned and applied, runs to a Succeeded Job on the local node in one control-host test
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-apply-workloads
- depends_on: story:cloudspec-submission-job-exit
scope:
- confidence: inferred
  path: product/cloud/crates/weft-testkit/src/steps.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/cloudspec_snippet.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
revision: 4
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3 as a whole: "the walking-skeleton path runs the
`snippet` golden to a finished job on the local node". Finished = the Job is `Succeeded` (D-C2 = A).
Takes outcome 3 and its acceptance from the archived `story:cloudspec-submission-plan`. Depends on
`story:cloudspec-submission-apply-workloads` (the apply route, D-C1) and
`story:cloudspec-submission-job-exit` (the Job finishes, D-C2); through them on every other story
of this set and on `-submit` and `-plan-from-source`.

## Context

- Today the skeleton test
  (`product/cloud/crates/weft-testkit/tests/main.rs`, module `skeleton`,
  `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running`) goes
  DefineWorkload -> DefineService -> RequestReplica -> PlaceInstance -> Running, with no plan or
  release. This story adds the cloudspec path beside it and leaves it unchanged.
- No production code is expected. A defect the test finds in a story of this set is fixed in the
  file of that story, named in the report.

## Outcome

One test, `product/cloud/crates/weft-testkit/tests/cloudspec_snippet.rs` (module of the one test
binary), on the control host with the skeleton node agent: the operator and tenant path of
`steps::run_path`, the node joined, a formation pinned to the cluster with one environment, the
runtime `snippet` and the snippet artifact registered; then
`POST /cloudspec/submissions` with `tests/fixtures/cloudspec/snippet.submission.json`,
`PlanFromSource` (Deployer), `RecordPlanResult` with no violations (ReleaseController),
`POST /cloudspec/plans/<plan_id>/apply`, and a wait for the Job.

## Scope

`product/cloud/crates/weft-testkit/tests/cloudspec_snippet.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line). Helpers it needs from
`weft-testkit/src/steps.rs` are added there only if no earlier story of this set added them.

Shared: `weft-testkit/tests/main.rs`, one `mod` line, with every story of this set and with
`-submit` and `-plan-from-source`; `weft-testkit/src/steps.rs` with the skeleton tests.

## Acceptance

A control-host test submits the committed `snippet` golden, plans it with `PlanFromSource`, applies
the plan, and within 30 s of its first command reads the one Job `Succeeded` with its instance
`Exited` 0 on the local node, with no network, no cloudspec binary and no 501.

Checked by:

1. The Jobs row `Succeeded`, `succeeded_tasks` 1; the instance `Exited`, exit code 0; the agent
   report holds `{"instance_id": <it>, "exit_code": 0, "stdout": "hello\n"}`.
2. Event lines in this order: `SourceSubmitted`, `PlanCreated`, `PlanSimulated`, `ReleaseCreated`,
   `WorkloadDefined`, `WorkloadAssigned`, `ReleaseStepStarted`, `JobSubmitted`, `JobStarted`,
   `InstanceCreated`, `InstanceExited`, `TaskSucceeded`, `JobSucceeded`.
3. A 2xx request line for the submission route, `PlanFromSource`, `RecordPlanResult`, the apply
   route, `RecordArtifactVerified`, `RecordInstanceRunning`, `RecordInstanceExited`; no request
   line with status 501.
4. Binding lines `applied`: `r1-rollout-submit-job`, `job-start-on-submit`, `job-task-instances`,
   `job-instance-scheduling-request`, `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`,
   `r4-record-placement-assigned`, `job-task-succeeded-on-exit`, `job-complete-on-task`;
   `r1-set-workload-replicas` `dropped` with `not-a-service`.
5. The test time is printed and named in the report; the skeleton test still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
