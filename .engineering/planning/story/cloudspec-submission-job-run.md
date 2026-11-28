---
format: aep.planning-md/3
id: story:cloudspec-submission-job-run
kind: story
status: implemented
title: A rollout step of a Job workload submits and starts its Job, and each task becomes an instance placed on a node (D-C2)
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-release-steps
scope:
- confidence: cited
  path: ess/components.yaml
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/start_job.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/submit_job.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/job_instance_scheduling_request.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/job_start_on_submit.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/job_task_instances.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings/mod.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/r1_rollout_submit_job.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/job_run.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
- confidence: inferred
  path: quality/spec-diff-acknowledgements.json
- confidence: cited
  path: tools/synth-input.sh
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T14:23:34Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T14:23:34Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T15:24:40Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3, second half, fourth step: a rollout step of a
job-shaped workload reaches instances on a node. Lands the first half of owner decision D-C2 = A
(`decision-blocker:cloudspec-snippet-job-instances`, cleared): new bindings in weft
`ess/components.yaml`, Rollout -> `SubmitJob`, `JobStarted` -> one instance per task. Depends on
`story:cloudspec-submission-release-steps` (`ReleaseStepStarted`, and `r1-set-workload-replicas`
dropping `not-a-service` for the same event).

## Context

Read on integration/2026-11-28-6 (db91440):

- `ess/components.yaml:1238-1240` marks it OPEN: "a Rollout step of another shape ... reaches its
  instances by no loop yet".
- `SubmitJob` (`ess/cloud/5-workloads/workloads.yaml:3202`) creates a `Pending` Job and emits
  `JobSubmitted {job_id, workload_id, parallelism, gang}`; only `StartJob` (line 3273,
  `bound_tasks`) moves it to `Running` and emits `JobStarted`. Both answer 501 (no behaviour file).
- `r1-submit-scheduling-request` skips an instance with a `job_id` (`ess/components.yaml:1286-1290`);
  lines 1280-1285: a gang member is submitted by the control plane, and a Job instance outside a
  gang is OPEN, "settled by ... a loop of its own for Job tasks". D-C2 = A takes that loop.
- Bindings that drop a refusal are delivered by hand (`weft-impl/src/bindings/mod.rs:1-15`,
  `tools/synth-input.sh` E2).

## Outcome

1. Behaviours `submit_job.rs` and `start_job.rs` under `product/cloud/crates/weft-impl/src/behaviours/`,
   every outcome of each contract in its order.
2. Four bindings, declared in `ess/components.yaml` in one block after `r1-set-workload-replicas`
   (its OPEN and the OPEN of `r1-submit-scheduling-request` replaced by a pointer to the block),
   delivered by hand (a file each under `weft-impl/src/bindings/`, entries in `bindings/mod.rs`, ids
   in `removed_bindings` of `tools/synth-input.sh`):
   - `r1-rollout-submit-job`: `ReleaseStepStarted` [Rollout, workload defined] -> `SubmitJob`
     {workload_id, scope_id of the event; parallelism, gang, max_retries of the workload
     `JobSettings`}; drop `not-a-job`, `workload-retired`;
   - `job-start-on-submit`: `JobSubmitted` [gang false] -> `StartJob` {job_id, bound_tasks: the
     event parallelism}; drop `wrong-state`;
   - `job-task-instances`: `JobStarted` -> `CreateInstance` {workload of the Job, job_id,
     task_index} for each task index below `bound_tasks`;
   - `job-instance-scheduling-request`: `InstanceCreated` [job_id defined] ->
     `SubmitSchedulingRequest`, the mapping of `r1-submit-scheduling-request`; an instance of a gang
     Job is left to the control plane; drop `already-requested`.
   Each comment names that a repeated delivery of `ReleaseStepStarted` submits a second Job (no
   refusal of `SubmitJob` reads the step).
3. Gang jobs, retries and a failed task are not this story.

## Domain relations

- Job -> Workload, references one: `ess/cloud/5-workloads/workloads.yaml`, entity
  `weft.workloads.Job`, relation `workload`.
- Instance -> Job, references one, one instance per task index: `ess/4-applications/workloads.yaml`,
  entity `weft.workloads.Instance`, relation `job`; one per task is D-C2 = A.
- ReleaseStep -> Job: a rollout step names no Job (`StartReleaseStep` sets `job_id` from its input
  only; `ess/6-delivery/deployments.yaml`, entity `weft.deployments.ReleaseStep`, relation `job`).

## Scope

`product/cloud/crates/weft-impl/src/behaviours/{submit_job,start_job}.rs` (new),
`product/cloud/crates/weft-impl/src/bindings/{r1_rollout_submit_job,job_start_on_submit,job_task_instances,job_instance_scheduling_request}.rs`
(new), `product/cloud/crates/weft-impl/src/bindings/mod.rs`, `ess/components.yaml`,
`tools/synth-input.sh`, `product/cloud/crates/weft-testkit/tests/job_run.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line).

Shared: `bindings/mod.rs`, `tools/synth-input.sh` with `story:cloudspec-submission-release-steps`
and `story:cloudspec-submission-job-exit`; `ess/components.yaml` with `-job-exit`;
`weft-testkit/tests/main.rs`, one `mod` line, with every story of this set. If
`tools/check-spec-diff.sh` asks for an acknowledgement of the new bindings, the story writes
`quality/spec-diff-acknowledgements.json`. Overlap: `story:agent-run-local-job` (draft,
`epic:agent-workloads`) names the Job start behaviours too; whichever lands second reuses them.

## Acceptance

With the skeleton node joined, starting the rollout step of a release whose workload is a Snippet
Job of parallelism 1 leaves one `Running` Job and its one instance (task 0) reading `Running` on
that node, with each of the four bindings `applied`.

Checked by:

1. The `weft.workloads.Jobs` row: parallelism 1, gang false, state `Running`; event lines
   `JobSubmitted`, `JobStarted`; the instance has the `job_id` and task index 0, reads `Running`
   within 30 s, and the agent report holds its exit line (exit 0).
2. Binding lines `applied` for the four ids; `r1-set-workload-replicas` `dropped` `not-a-service`;
   `r1-submit-scheduling-request` has no line for the job instance.
3. `SubmitJob` sent directly for a Service workload -> `ShapeMismatch`; parallelism 0 ->
   `InvalidAmount`; `StartJob` with `bound_tasks` 0 -> `InvalidAmount`; neither writes a row.
4. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
