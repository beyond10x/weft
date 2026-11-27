---
format: aep.planning-md/3
id: story:skeleton-workload-definitions
kind: story
status: implemented
title: 'Workload definitions: formation, artifact, workload and service obligations of the skeleton path'
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
- depends_on: story:skeleton-control-host
scope:
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/add_component.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/commit_revision.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_environment.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_formation.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/define_service.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/define_workload.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/register_artifact.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/src
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 24, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 25, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-27T20:35:46Z", actor: "human:operator", revision: 26, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 3 (the obligations of the workload-definition part of
the path). Depends on `story:skeleton-control-host` (the control host, `weft-impl`, `weft-client`,
`weft-testkit`). Blocked by no ESS gap: no E1-E3 transformation touches these commands.

## Context

The tenant part of the path (`sim/experiments/full-setup.yaml:54-61`, deployed into the default
cluster) defines what will run. Obligations implemented here, each in its own file under
`product/cloud/crates/weft-impl/src/behaviours/` against its contract in `generated/PLAN.md` § Obligations:
`weft.formations.CreateFormation`, `weft.formations.AddComponent`, `weft.formations.CommitRevision`,
`weft.formations.CreateEnvironment`, `weft.artifacts.RegisterArtifact`,
`weft.workloads.DefineWorkload`, `weft.workloads.DefineService` (7 of the 24 command behaviours on
the skeleton path). `weft.workloads.RegisterRuntime` is generated. `AddLink` is not on the skeleton
path (one component, no link).

The workload of the skeleton: package kind `Snippet`, one `Main` process
`["sh", "-c", "echo weft"]`, isolation `Container`, shape `Service`, a runtime registered with
`package_kinds: [Snippet]`, an artifact of kind `Snippet`, a service with `replicas: 1`.
`weft-testkit` gets these steps as functions after the operator steps of
`story:skeleton-control-host`.

## Domain relations

- `weft.workloads.Workload` -> `weft.accounts.Account`, `weft.formations.Formation`,
  `weft.formations.FormationRevision`, `weft.formations.Environment` and `weft.workloads.Runtime`,
  each many-to-one, references — inferable: `ess/4-applications/workloads.yaml`, entity
  `weft.workloads.Workload`, relations `account`, `formation`, `formation_revision`, `environment`,
  `runtime`.
- `weft.workloads.Service` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Service`, relation `workload`.
- `weft.accounts.Tenant` -> `weft.accounts.Account`, one-to-many, tenant owns account — inferable:
  `ess/2-tenancy/accounts.yaml`, entity `weft.accounts.Tenant`, relation `accounts`.

## Scope

- `product/cloud/crates/weft-impl/` (`src/behaviours/`: one file per obligation listed above)
- `product/cloud/crates/weft-testkit/` (`src/` path functions; `tests/main.rs` module `definitions`)

## Acceptance

1. After the operator and tenant steps, `CreateFormation`, `AddComponent`, `CommitRevision`,
   `CreateEnvironment` and `RegisterArtifact`, each sent as its own actor, leave rows that read back
   with the identity each answered and the values sent: the formation in
   `weft.formations.Formations` (its `name` and `cluster_id` the default cluster), the component in
   `weft.formations.FormationComponents` (its `name`), the revision in
   `weft.formations.FormationRevisions` (its `formation_id`), the environment in
   `weft.formations.Environments` (its `name`, `protected` false) and the artifact in
   `weft.artifacts.Artifacts` (its `artifact_digest`, `kind` `Snippet`).
2. Then `RegisterRuntime`, `DefineWorkload` and `DefineService` leave the workload in
   `weft.workloads.WorkloadList` with its `Snippet` package kind and its one `Main` process
   `["sh", "-c", "echo weft"]`, and the service in `weft.workloads.Services` with `replicas` 1 and
   that `workload_id`.
3. `DefineWorkload` naming a runtime whose `package_kinds` do not include `Snippet` answers outcome
   `runtime-kind` and adds no row to `weft.workloads.WorkloadList`, while the same request naming the
   `Snippet` runtime is accepted (positive control).
## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh (`yq` v4 may be called from sh).
- No load tests, no VMs, no GPU workloads.
- The test step makes no request that leaves the host: the control host, the node agent and
  substrate-daemon run on loopback and a Unix socket; seeded rows or fixtures stand in for
  anything else.
