---
format: aep.planning-md/3
id: story:ports-local-pool-node
kind: story
status: draft
title: With placement local, a pool's node is made by the `machines` port and joins on its own
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-local-machines
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: ess/9-actors/actors.yaml
- confidence: cited
  path: ess/components.yaml
- confidence: inferred
  path: product/cloud/crates/weft-bin/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-bin/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-bin/src/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-controllers/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/machines.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, acceptance 3: with placement `local`, a node pool's node is made
by the machines port and joins on its own. Design: `docs/platform-ports.md` § 6 C1 (steps
`pool-scale`, `pool-machines`), § 9 S2 and § 12.4. Depends on `story:ports-local-machines`.

## Context

- Spec change S2 (design § 9):
  - `ess/9-actors/actors.yaml`, ControlPlane `may:` (`:966-1079`): add
    `weft.infrastructure.ProvisionPoolNode`, `weft.infrastructure.IssueJoinToken` and the views
    `weft.infrastructure.Clouds`, `NodePools`, `Nodes`. Today only the ClusterOperator may send the
    two commands (`ess/9-actors/actors.yaml:59-60`).
  - `ess/components.yaml`, the level-step block (`:1104-1147`): the three `(machines)` lines of
    S2.
  - The change passes `ess specify validate --path ess --strict-requires` and is acknowledged with
    `tools/check-spec-diff.sh --acknowledge`.
- `weft-controllers` adds `src/machines.rs` with the steps, sending through `send`
  (`product/cloud/crates/weft-controllers/src/lib.rs:47-64`) as ControlPlane:
  - `pool-scale`: an `Active` pool whose `desired_size` exceeds its nodes that are not `Gone` gets
    `ProvisionPoolNode`. The count is made in code from the views `NodePools` and `Nodes`
    (`ess/1-infrastructure/infrastructure.yaml:1345`, `:1398`): a count against a field of another
    row is not expressible in ESS (`docs/ess-gaps.md:18`).
  - `pool-machines`: a pool node in `Provisioning` whose machine `observe` answers `Gone` gets
    `IssueJoinToken` (a fresh token; the old one is void, `:1039-1040`), then `make`. One path for a
    node the step provisioned, a node the operator provisioned, and a control host that restarted
    between `ProvisionPoolNode` and `make`.
  - `gone-machines` is `story:ports-machines-remove`.
- `weft-bin` takes `--machines local --node-agent <path> --substrate <socket>`; without
  `--machines` it makes no machine. The steps act only on clouds whose placement is `Local`, read
  from the view `weft.infrastructure.Clouds` (`story:ports-spec-placement` later replaces that
  constant with the placement the control host serves). `Host::tick`
  (`product/cloud/crates/weft-bin/src/lib.rs:234`) runs the steps after the existing steps, under
  the same lock.
- The walking skeleton keeps its path: its tests run the control host without `--machines`;
  `story:skeleton-end-to-end` sends `ProvisionPoolNode` and starts its agent itself (through the
  local adapter) and creates its cloud with placement `Hosts`
  (`product/cloud/crates/weft-testkit/src/steps.rs:28-36`), which the local steps do not act on.
- The new test (`weft-testkit`, `tests/main.rs` module `local_pool`): a control host with
  `--machines local`, then `CreateCloud {placement: Local, access_mode: SingleUser, control_plane:
  {replicas: 1}}` (the only combination `CreateCloud` accepts for `Local`,
  `ess/1-infrastructure/infrastructure.yaml:519-539`), `DeclareZone`, `CreateNodePool {min_size:
  1}`. Bound 60 s; it reads `Ready` within 10 s. It starts one control host and one agent beside
  the setup hook's daemon (design § 12.6).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references; the field
  `node_pool_id` is absent for a node not made by a pool — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation `pool`.
- `weft.infrastructure.NodePool` -> `weft.infrastructure.Cloud`, many-to-one, references —
  inferable: `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.NodePool`,
  relation `cluster`.
- `weft.infrastructure.NodePool` -> `weft.infrastructure.Zone`, many-to-one, references —
  inferable: `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.NodePool`,
  relation `in_zone`.
- `weft.infrastructure.Node` -> `weft.infrastructure.Cloud`, many-to-one, references — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation
  `cluster`.
- The machine is no `ess/1` entity; the port keys it by `node_id` (design § 6 C1).

## Scope

- `ess/9-actors/actors.yaml`, `ess/components.yaml` (S2), `quality/spec-diff-acknowledgements.json`
- `product/cloud/crates/weft-controllers/`: `src/machines.rs`, `src/lib.rs`, `tests/main.rs`
- `product/cloud/crates/weft-bin/`: `src/main.rs`, `src/lib.rs`
- `product/cloud/crates/weft-testkit/`: `src/lib.rs`, `tests/main.rs` module `local_pool`

## Acceptance

A control host with `--machines local` reads the node of a pool created with `min_size: 1` move
`Provisioning` -> `Ready` in `weft.infrastructure.Nodes` within 10 s, the test having sent no
`ProvisionPoolNode` and started no agent, while a control host without `--machines` makes no node
for the same commands.

## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh.
- The test step makes no request that leaves the host; no test starts a `substrate-daemon` of its
  own except where this story says so (design § 12.6).
