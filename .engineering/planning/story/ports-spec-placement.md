---
format: aep.planning-md/3
id: story:ports-spec-placement
kind: story
status: draft
title: weft-control declares the placement it serves; `CreateCloud` refuses another (S1)
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-local-pool-node
scope:
- confidence: cited
  path: ess/1-infrastructure/infrastructure.yaml
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: product/cloud/crates/weft-bin/src/main.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/machines.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/context/decider.rs
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, design rule 3.5 (one adapter per port per control host, chosen
from the placement it serves). Design: `cloudspec@2ed72c2:docs/platform-ports.md` § 9, spec change S1. Depends on
`story:ports-local-pool-node`, whose `Local` constant this story replaces.

## Context

- `ess/components.yaml`, component `weft-control`: add `settings: [{name: placement, type:
  weft.infrastructure.Placement, required: true, summary: The placement whose adapters this control
  host runs.}]`.
- `ess/1-infrastructure/infrastructure.yaml`, `CreateCloud` (`:510`): a new outcome before
  `created`: `name: placement-not-served`, `external: "port machines: the control host serves
  another placement than input.placement"`, `error: weft.infrastructure.PlacementNotServed`; and
  the error type `weft.infrastructure.PlacementNotServed` beside `SingleUserOutsideLocal` (`:319`).
- The decider (`product/cloud/crates/weft-impl/src/context/decider.rs`) answers the branch from the
  `--placement` of the control host (pattern B, design § 4).
- `weft-bin` takes `--placement`; the `Local` constant of `weft-controllers/src/machines.rs`
  (design § 12.4) becomes the served placement.
- The change passes `ess specify validate --path ess --strict-requires` and is acknowledged with
  `tools/check-spec-diff.sh --acknowledge`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None is added. `placement` is a field of `weft.infrastructure.Cloud`, fixed at creation
  (`ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Cloud`, field
  `placement`); the new setting is a component setting, not an entity.

## Scope

- `ess/components.yaml`, `ess/1-infrastructure/infrastructure.yaml`,
  `quality/spec-diff-acknowledgements.json`
- `product/cloud/crates/weft-impl/src/context/decider.rs`
- `product/cloud/crates/weft-bin/src/main.rs`
- `product/cloud/crates/weft-controllers/src/machines.rs`

## Acceptance

A control host started with `--placement local` answers `CreateCloud {placement: Hosts}` with
`placement-not-served`, no cloud row appearing in `weft.infrastructure.Clouds`, and `CreateCloud
{placement: Local, access_mode: SingleUser, control_plane: {replicas: 1}}` with `created`, the
cloud reading `Active`.

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
