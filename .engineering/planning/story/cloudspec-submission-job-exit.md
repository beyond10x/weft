---
format: aep.planning-md/3
id: story:cloudspec-submission-job-exit
kind: story
status: draft
title: A job instance that exits 0 is recorded Exited, its task succeeds, and the Job reads Succeeded (D-C2)
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-job-run
scope:
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: product/cloud/crates/weft-agent/src/follow.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/tests/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/complete_job.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/record_instance_exited.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/record_task_succeeded.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/job_complete_on_task.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/job_task_succeeded_on_exit.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings/mod.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/job_run.rs
- confidence: cited
  path: tools/synth-input.sh
revision: 4
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3, second half, fifth step: a job instance that
ended finishes its Job. Lands the second half of owner decision D-C2 = A
(`decision-blocker:cloudspec-snippet-job-instances`, cleared): exit 0 -> `RecordTaskSucceeded`;
"finished" = the Job is `Succeeded`. Depends on `story:cloudspec-submission-job-run` (a job instance
on a node, and the files it adds to first).

## Context

Read on integration/2026-11-28-6 (db91440):

- `RecordInstanceExited` (`ess/4-applications/workloads.yaml:1354`, granted to the NodeAgent) answers
  501; its event `InstanceExited` carries `instance_id`, `exit_code`, `workload_id`, `service_id`,
  `replica_index`, `restart`, not the job or task: the instance row holds them.
- `RecordTaskSucceeded` (`ess/cloud/5-workloads/workloads.yaml:3310`) only counts
  `succeeded_tasks`; the move to `Succeeded` is `CompleteJob` (line 3380, `outstanding` while
  `succeeded_tasks < parallelism`). Both answer 501. So reaching "finished" needs a second binding,
  `TaskSucceeded` -> `CompleteJob`, beside the one D-C2 names.
- weft-agent writes each exit to its report file and sends no command for it
  (`product/cloud/crates/weft-agent/src/follow.rs:362-380`). The `weft.workloads.Instances` view it
  reads carries `restart_policy`.

## Outcome

1. Behaviours `record_instance_exited.rs`, `record_task_succeeded.rs`, `complete_job.rs` under
   `product/cloud/crates/weft-impl/src/behaviours/`, every outcome of each contract in its order.
2. weft-agent (`follow.rs`): once every process of an instance whose restart mode is `Never` has
   exited 0 and its `RecordInstanceRunning` was answered, it sends `RecordInstanceExited`
   {instance_id, node_id, exit_code 0} as the NodeAgent, again in the next pass until answered. An
   instance of another restart mode (the skeleton Service) is not reported, as today; a non-zero
   exit is not this story.
3. Two bindings, declared in `ess/components.yaml` in the block of `-job-run`, delivered by hand
   (files under `weft-impl/src/bindings/`, entries in `bindings/mod.rs`, ids in `removed_bindings`):
   - `job-task-succeeded-on-exit`: `InstanceExited` [exit_code 0, service_id undefined] ->
     `RecordTaskSucceeded` {job_id, task_index of the instance row}; an instance with no job is
     skipped; drop `not-running`;
   - `job-complete-on-task`: `TaskSucceeded` -> `CompleteJob` {job_id, outputs []}; drop
     `outstanding`, `wrong-state`.

## Domain relations

- Instance -> Job, references one: `ess/4-applications/workloads.yaml`, entity
  `weft.workloads.Instance`, relation `job`; the binding reads `job_id` and `task_index` from that
  row.
- Job -> Artifact (outputs), references many, empty for a snippet: `ess/cloud/5-workloads/workloads.yaml`,
  entity `weft.workloads.Job`, relation `output_artifacts`.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/{record_instance_exited,record_task_succeeded,complete_job}.rs`
(new), `product/cloud/crates/weft-impl/src/bindings/{job_task_succeeded_on_exit,job_complete_on_task}.rs`
(new), `product/cloud/crates/weft-impl/src/bindings/mod.rs`, `ess/components.yaml`,
`tools/synth-input.sh`, `product/cloud/crates/weft-agent/src/follow.rs`,
`product/cloud/crates/weft-agent/tests/main.rs`, `product/cloud/crates/weft-testkit/tests/job_run.rs`
(the exit case beside the run of `-job-run`).

Shared: `bindings/mod.rs`, `tools/synth-input.sh`, `ess/components.yaml` and
`weft-testkit/tests/job_run.rs` with `story:cloudspec-submission-job-run`; `bindings/mod.rs` and
`tools/synth-input.sh` with `-release-steps`. `weft-agent/src/follow.rs` with `story:slot-node-process`
(`epic:slot-node-daemon`, which depends on this set through `-apply-workloads`). Overlap:
`story:agent-run-local-job` names the Job end behaviours and an agent exit report too.

## Acceptance

A Snippet Job of parallelism 1 started through its rollout step reads `Succeeded` with
`succeeded_tasks` 1 after its one instance exits 0 on the local node, and that instance reads
`Exited` with exit code 0.

Checked by:

1. Within 30 s: the Jobs row `Succeeded`, `succeeded_tasks` 1; the instance `Exited`, exit code 0;
   event lines `InstanceExited`, `TaskSucceeded`, `JobSucceeded`; a request line of
   `RecordInstanceExited` with a 2xx status.
2. Binding lines `applied` for both ids.
3. Sent directly: `RecordInstanceExited` from another node -> `WrongNode`; `RecordTaskSucceeded`
   with task index 1 of a parallelism-1 Job -> `InvalidAmount`; `CompleteJob` of a Job with no
   succeeded task -> `TasksOutstanding`; none changes a row.
4. A weft-agent test: an instance of restart mode `Always` that exited gets no `RecordInstanceExited`.
5. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
