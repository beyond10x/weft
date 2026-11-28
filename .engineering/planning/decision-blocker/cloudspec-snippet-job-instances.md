---
format: aep.planning-md/3
id: decision-blocker:cloudspec-snippet-job-instances
kind: decision-blocker
status: cleared
title: Nobody has decided how a job-shaped snippet of a release reaches an instance on a node, and what finished means
relations:
- blocks: epic:cloudspec-submission
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T13:46:33Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided how a job-shaped snippet of a release reaches an instance on a node, and what finished means

## The relation

`weft.deployments.Release` (and its `Rollout` step) → `weft.workloads.Instance` of a workload of shape
`Job`. Today the only path from a release to instances is R1: `StartReleaseStep` emits
`ReleaseStepStarted`, binding `r1-set-workload-replicas` invokes `SetWorkloadReplicas`, which only a
`Service` takes; for another shape the binding drops `not-a-service`, and `ess/components.yaml:1232-1234`
says: "OPEN: a Rollout step of another shape ... reaches its instances by no loop yet". The job
commands (`SubmitJob`, `StartJob`, `RecordTaskSucceeded`, `ess/cloud/5-workloads/workloads.yaml:3132-3262`)
have no binding from `JobSubmitted` or `JobStarted` to an instance, and none from an exited instance to
a task. "A finished job" may be `Job` state `Succeeded` or an instance that exited 0
(`RecordInstanceExited`).

The apply path also answers 501 today for every step (no behaviour file on integration/2026-11-28-6):
`RecordPlanResult`, `ApplyPlan`, `AddReleaseStep`, `StartRelease`, `StartReleaseStep`,
`SetWorkloadReplicas`, `SubmitJob`, `RecordInstanceExited`; and `StartRelease` has no synthesized
scenario (`ess/6-delivery/deployments.yaml:540-549`).

## Options

- **A (default)**: weft adds the loop in its own components: a `Rollout` step of a job-shaped
  workload invokes `SubmitJob`; `JobStarted` creates one instance per task; an instance that exited 0
  records `RecordTaskSucceeded`. Finished = `Job` `Succeeded`.
- **B**: the skeleton skips releases for now: after the workloads exist it sends `SubmitJob`
  directly; the job-to-instance and exit-to-task bindings of A are still added. Finished = `Job`
  `Succeeded`.
- **C**: finished = the instance exited 0 and is recorded `Exited`, reached through a one-replica
  `Service` with restart `Never` (against cloudspec shape `job`).

A change of architecture: it waits for an answer and takes no default after 15 minutes.

## What it stops

The run of the `snippet` golden to a finished job on the local node (`epic:cloudspec-submission`
outcome 3, second half; outcome 3 of `story:cloudspec-submission-plan`).
