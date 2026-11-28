---
format: aep.planning-md/3
id: story:ports-spec-machine-failed
kind: story
status: draft
title: A machine the place refused is recorded and the node goes (S3)
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-machines-remove
scope:
- confidence: cited
  path: ess/1-infrastructure/infrastructure.yaml
- confidence: cited
  path: ess/9-actors/actors.yaml
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/machines.rs
- confidence: inferred
  path: product/cloud/crates/weft-controllers/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours
- confidence: inferred
  path: product/cloud/crates/weft-platform/src/local/machines.rs
- confidence: inferred
  path: quality/spec-diff-acknowledgements.json
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, design rule 3.2 (the outcome weft records is what the place
answered). Design: `docs/platform-ports.md` § 9, spec change S3, and § 6 C1 (refusal
`CapacityExhausted`). Depends on `story:ports-machines-remove`.

## Context

- `ess/1-infrastructure/infrastructure.yaml`: command `weft.infrastructure.RecordNodeMachineFailed
  {node_id, reason: String}`, actor ControlPlane, from `Provisioning` to `Gone` (transition
  `remove`, `:268-270`), emitting `NodeMachineFailed {node_id, reason}`.
- `ess/9-actors/actors.yaml`: grant the command to ControlPlane.
- `ess/components.yaml`: a binding from `NodeMachineFailed` to `scheduling.RecordScaleRefused`
  where a scale request of the node is open.
- Without it a pool whose `make` is refused holds a `Provisioning` node until its token expires.
- The behaviour of the new command goes in its own file under
  `product/cloud/crates/weft-impl/src/behaviours/`.
- The step of `weft-controllers/src/machines.rs` sends the command when `make` answers
  `CapacityExhausted`; the local adapter answers it from a bound on machines per host (design § 6
  C1).
- The change passes `ess specify validate --path ess --strict-requires` and is acknowledged with
  `tools/check-spec-diff.sh --acknowledge`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation `pool`.
- `weft.scheduling.ScaleRequest` -> `weft.infrastructure.Node`, many-to-one, references — inferable:
  `ess/5-runtime/scheduling.yaml`, entity `weft.scheduling.ScaleRequest`, relation `node`.
- `weft.scheduling.ScaleRequest` -> `weft.infrastructure.NodePool`, many-to-one, references —
  inferable: `ess/5-runtime/scheduling.yaml`, entity `weft.scheduling.ScaleRequest`, relation
  `node_pool`.

## Scope

- `ess/1-infrastructure/infrastructure.yaml`, `ess/9-actors/actors.yaml`, `ess/components.yaml`
- `product/cloud/crates/weft-impl/src/behaviours/`
- `product/cloud/crates/weft-controllers/src/machines.rs`

## Acceptance

When the local adapter answers `CapacityExhausted`, the step sends `RecordNodeMachineFailed`, the
node moves `Provisioning` -> `Gone`, and the pool step provisions no node past `max_size`.

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
