---
format: aep.planning-md/3
id: decision-blocker:cloudspec-snippet-workload-definition
kind: decision-blocker
status: cleared
title: Nobody has decided which command turns a snippet part of a planned cloudspec revision into a workload
relations:
- blocks: epic:cloudspec-submission
- blocks: story:slot-node-process
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T13:46:33Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided which command turns a snippet part of a planned cloudspec revision into a workload

## The relation

snippet part of a cloudspec architecture → `weft.workloads.Workload` (package kind `Snippet`, shape
`Job`). Cardinality one-to-one (inferred from cloudspec `fdcf7e8:crates/cloudspec/src/generate/weft/runs.rs`,
row `SNIPPET`; no ess/1 document declares this relation). Ownership and lifecycle are undecided:

- `PlanFromSource` creates one record, the `Plan`, and mints the revision
  (`ess/cloud/11-translation/deployments.yaml:1406-1480`); no command creates a workload from a
  revision. `DefineWorkload` (`ess/cloud/5-workloads/workloads.yaml:1855`) takes a revision and an
  environment, and also `runtime_id`, `artifact_digest`, `isolation`, `claims`, `restart_policy` and
  `priority`, which a snippet part (`command`, `environment`, shape job) does not carry.
- A release names workloads that exist already: `AddReleaseStep` takes a `workload_id`
  (`ess/6-delivery/deployments.yaml:613`).

## Options

- **A (default)**: the control host defines them after `PlanFromSource`: one `DefineWorkload` per
  snippet part naming the plan revision and environment, with the runtime, artifact, isolation and
  claims of a fixed snippet profile kept by weft; refused parts of the coverage are not defined.
- **B**: the implementation of `PlanFromSource` writes the workload rows itself, beside the revision
  (rows its contract does not show).
- **C**: cloudspec adds a command that defines the workloads of a revision; a later pin move.

A change of architecture: it waits for an answer and takes no default after 15 minutes.

## What it stops

The run of the `snippet` golden to a finished job (`epic:cloudspec-submission` outcome 3, second
half; outcomes 3 and 4 of `story:cloudspec-submission-plan`). Submit and `PlanFromSource` are not
behind it (`story:cloudspec-submission-submit`, `story:cloudspec-submission-plan-from-source`).
