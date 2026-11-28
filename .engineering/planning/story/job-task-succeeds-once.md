---
format: aep.planning-md/3
id: story:job-task-succeeds-once
kind: story
status: draft
title: A Job task succeeds at most once, and a lost answer to RecordInstanceRunning does not lose the exit
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:cloudspec-submission`, found while building `story:cloudspec-submission-job-exit`
(integration/2026-11-28-8, fab73ae): a repeated `InstanceExited` of one task while a Job of
parallelism above 1 is still running counts that task twice, because `RecordTaskSucceeded` keeps no
record of which task index succeeded. With parallelism 1 the repeat is refused `not-running`, so
the snippet path is not affected.

A second defect from the same build: when `RecordInstanceRunning` took effect but its answer was
lost, and the instance has left `Starting`, the node agent stops following it and never sends
`RecordInstanceExited` (`product/cloud/crates/weft-agent/src/follow.rs`).

## Outcome

1. A task index succeeds at most once per Job: a second `RecordTaskSucceeded` for the same index is
   refused by name and `succeeded_tasks` is unchanged.
2. The node agent sends `RecordInstanceExited` for an instance whose `RecordInstanceRunning` answer
   was lost, once the instance's row reads `Running`.

## Scope

`ess/4-applications/workloads.yaml` (a per-task record or guard), the generated or hand-written
behaviour of `RecordTaskSucceeded` under `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-agent/src/follow.rs`, `product/cloud/crates/weft-testkit/tests/job_run.rs`,
`product/cloud/crates/weft-agent/tests/follow_adversary.rs`.

## Acceptance

1. A testkit test with a Job of parallelism 2 delivers `InstanceExited` of task 0 twice and reads
   `succeeded_tasks` 1 and the Job still `Running`.
2. A weft-agent test loses the answer of `RecordInstanceRunning`, lets the instance's row read
   `Running`, ends its process with 0, and reads one `RecordInstanceExited`.
3. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task check'` exits 0.
