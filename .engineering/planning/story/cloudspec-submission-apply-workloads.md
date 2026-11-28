---
format: aep.planning-md/3
id: story:cloudspec-submission-apply-workloads
kind: story
status: draft
title: Applying a plan of a cloudspec source defines, assigns and rolls out one Job workload per snippet part with weft's fixed snippet profile (D-C1)
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-release-steps
- depends_on: story:cloudspec-submission-plan-from-source
scope:
- confidence: inferred
  path: product/cloud/crates/weft-bin/src/cloudspec.rs
- confidence: inferred
  path: product/cloud/crates/weft-bin/src/cloudspec_apply.rs
- confidence: cited
  path: product/cloud/crates/weft-bin/src/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/cloudspec_apply.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
revision: 5
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3, second half, third step: the workloads of a
cloudspec plan exist and its release rolls them out. Lands owner decision D-C1 = A
(`decision-blocker:cloudspec-snippet-workload-definition`, cleared): the control host calls
`DefineWorkload` once per part when the plan is applied, with a fixed profile per part kind
(snippet now; process later, `story:slot-node-process`). Depends on
`story:cloudspec-submission-release-steps` (the release commands) and
`story:cloudspec-submission-plan-from-source` (the plan of the source, and through it
`story:cloudspec-submission-submit`: the stored file and `weft-bin/src/cloudspec.rs`).

## Context

Read on integration/2026-11-28-6 (db91440):

- Since `story:cloud-blocks-layer-refactor-pin` a workload reaches its formation through
  `weft.architecture.AssignWorkload` (`ess/cloud/9-composition/architecture.yaml:1681`), which needs
  a `component_id` of the formation; `DefineWorkload` (`ess/cloud/5-workloads/workloads.yaml:1928`)
  no longer checks formation, revision or component (`weft-impl/src/behaviours/define_workload.rs:9-12`).
  `PlanFromSource` mints a revision and adds no component. So one component per part is added at
  apply, beside D-C1 (see Domain relations).
- `JobSettings` {parallelism, gang, max_retries, backoff} (`workloads.yaml:89`); `RestartMode`
  {Never, OnFailure, Always} (line 443).
- The control host runs several commands in one transaction with `Host::unit`
  (`product/cloud/crates/weft-bin/src/lib.rs:165`).

## Outcome

1. Route `POST /cloudspec/plans/<plan_id>/apply`, body `{"strategy": "Replace"}`, the caller as for
   every request; module `product/cloud/crates/weft-bin/src/cloudspec_apply.rs` (new), reached from
   the `/cloudspec/` dispatch of `weft-bin/src/cloudspec.rs`, declared in `main.rs`.
2. The snippet profile, one constant of that module: runtime the active one named `snippet`, artifact
   `sha256:weft-skeleton-snippet` (the skeleton `ARTIFACT_DIGEST`), package kind `Snippet`,
   isolation `Container`, claims cpu 1, shape `Job` with settings parallelism 1, gang false,
   max_retries 0, restart mode `Never`, priority `standard`; one `Main` process with the part
   `command` and `environment`.
3. In one `Host::unit`: the plan must name a source (else 422 `{"refusal": "PlanNotFromCloudspec"}`,
   no command); the stored file (`weft_impl::documents::get` by the source digest) is read with
   `weft_cloudspec::Submission::read`; `ApplyPlan` (caller); then for each part with `block:
   workloads`, `kind: snippet` that the coverage does not mark `refused`, in key order:
   `AddComponent` named after the part key (an existing component of that name is reused),
   `DefineWorkload` with the profile, `AssignWorkload` (plan revision and environment),
   `AddReleaseStep` (Rollout, position n, component the part key, replicas 1); then `StartRelease`
   and `StartReleaseStep` of position 1 with its workload. The release commands run as
   `weft.actors.ReleaseController` of the plan scope. A refusal of any command commits nothing; the
   route answers that refusal.
4. Steps after position 1 stay `Pending`: no loop completes a rollout step of a Job in this epic.
   The release stays `Applying`.

## Domain relations

- snippet part -> `weft.workloads.Workload` (Snippet, shape Job), one-to-one, defined at apply by
  the control host: D-C1 = A on `decision-blocker:cloudspec-snippet-workload-definition`; inferable
  (inferred from cloudspec `fdcf7e8:crates/cloudspec/src/generate/weft/runs.rs`, row `SNIPPET`; no
  ess/1 document declares this relation).
- snippet part -> `weft.architecture.Component` named by the part key, one-to-one: inferable
  (inferred from `ess/cloud/9-composition/architecture.yaml:1681`, `AssignWorkload` requires a
  component, and line 1179, `AddComponent` refuses `name-taken`; no ess/1 document declares a part
  to component relation).
- Formation -> Component, owns many: `ess/cloud/9-composition/architecture.yaml`, entity
  `weft.architecture.Formation`, relation `components`.
- ComponentWorkload -> Component, Workload, FormationRevision, Environment, references one each:
  entity `weft.architecture.ComponentWorkload`, relations `component`, `workload`,
  `formation_revision`, `environment`.
- Release -> ReleaseStep, owns many: `ess/6-delivery/deployments.yaml`, entity
  `weft.deployments.Release`, relation `steps`.

## Scope

`product/cloud/crates/weft-bin/src/cloudspec_apply.rs` (new), `product/cloud/crates/weft-bin/src/cloudspec.rs`
(one dispatch arm), `product/cloud/crates/weft-bin/src/main.rs` (one `mod` line),
`product/cloud/crates/weft-testkit/tests/cloudspec_apply.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line).

Shared: `weft-bin/src/cloudspec.rs` and `weft-bin/src/main.rs` with
`story:cloudspec-submission-submit` (which creates the module and its branch). Reads
`weft-impl/src/documents.rs` of that story. `weft-testkit/tests/main.rs`, one `mod` line, with every
story of this set. `story:slot-node-process` adds the process profile beside this one.

## Acceptance

Applying the `Ready` plan of the submitted snippet golden creates one release with one started
rollout step whose workload is a `Snippet` Job assigned to component `hello` with command
`sh -c "echo hello"` and environment `GREETING=hello`, and a refusal inside the unit leaves no row.

Checked by:

1. Release `Applying` of the plan; one component `hello` of the formation; one workload: package
   kind `Snippet`, shape `Job`, the profile values, the process command and environment of the part;
   one `ComponentWorkload` naming the plan revision and environment; one step, position 1, Rollout,
   replicas 1, `Running`, naming that workload; event lines `ReleaseCreated`, `WorkloadDefined`,
   `WorkloadAssigned`, `ReleaseStepStarted`.
2. A plan not `Ready` -> `PlanNotReady`; with no runtime named `snippet` registered -> the
   `DefineWorkload` refusal; each leaves no release, component, workload or step.
3. A plan with no source -> 422 `PlanNotFromCloudspec`, no row.
4. A copy of the golden with a second snippet part `bye` that its coverage marks `refused` (seeded
   source and stored file) defines and steps only `hello`.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
