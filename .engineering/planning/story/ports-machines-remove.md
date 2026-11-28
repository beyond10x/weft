---
format: aep.planning-md/3
id: story:ports-machines-remove
kind: story
status: draft
title: A node that is gone loses its machine; a machine without a node is removed
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-local-pool-node
scope:
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/machines.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/tests/main.rs
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, acceptance 3 (the machines port owns the life of a pool's
machine, to its end). Design: `cloudspec@2ed72c2:docs/platform-ports.md` § 6 C1, step `gone-machines`. Depends on
`story:ports-local-pool-node` (the steps module and the grants of S2).

## Context

- Step `gone-machines` in `product/cloud/crates/weft-controllers/src/machines.rs`, actor
  ControlPlane:
  - a node `Gone` whose machine is not gone gets `remove`;
  - a machine whose node id the store does not hold (from `list()`) gets `remove`;
  - a pool node `Ready` whose machine is `Gone` is drained and removed (`DrainNode`, `RemoveNode`,
    granted to ControlPlane at `ess/9-actors/actors.yaml:1034-1036`).
- `RemoveNode` records `Gone` first; the step then removes the machine and observes it (design § 4,
  row `RequestNodePoolScaleDown` .. `DeleteNodePool`).
- Only clouds whose placement the control host serves are stepped (`Local` until
  `story:ports-spec-placement`).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation `pool`.
- The machine is no `ess/1` entity; the port keys it by `node_id` and weft holds no row for it
  (design § 6 C1), so a machine without a node is found by `list()`, not by a relation.

## Scope

- `product/cloud/crates/weft-controllers/`: `src/machines.rs`, `tests/main.rs`

## Acceptance

After `RemoveNode` moves a node the local adapter made to `Gone`, its agent process has ended by
the next tick, and an agent the adapter started for a node id the store does not hold ends on the
next tick.

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
