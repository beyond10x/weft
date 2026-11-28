---
format: aep.planning-md/3
id: story:ports-memory-placement
kind: story
status: draft
title: 'Placement `memory`: every port served in this process, for tests and simulation'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-exec-facts
- depends_on: story:ports-local-machines
revision: 2
---
## Epic

Decomposes `epic:platform-ports`: a placement whose every port is served in this process, for tests
and simulation. A later specification change, after the `local` adapter of the first build
(`cloudspec@2ed72c2:docs/platform-ports.md` § 12).

## Context

- The owner, 2026-11-28 about 02:17 CET, typed in a session: "what must not be missing is an
  in-memory runtime, which is also the thing we typically would use for the simulations."
  cloudspec plans it as its epic `memory-runtime` (any cloudspec architecture in one process,
  deterministic under a seed); this story is weft's placement for it.
- The placement list (`ess/1-infrastructure/infrastructure.yaml:22-24`, enum
  `weft.infrastructure.Placement`, `[Local, Hosts, Aws, Kubernetes]`) gains `Memory`: all ports
  in-process, for tests and simulation. Access mode as for `Local` (single-user).
- Prior art in weft: `weft-sim simulate` runs an experiment's setup, traffic, faults and controllers
  on the interpreted model with the invariants checked after every step
  (`cloudspec@2ed72c2:crates/cloudspec-sim/src/simulate.rs:1-3`); `weft-testkit`'s control host in this process, whose test
  decides when controllers and bindings act (`product/cloud/crates/weft-testkit/src/in_process.rs:1-4`).
- Memory adapters per port (`cloudspec@2ed72c2:docs/platform-ports.md` § 5, § 6): `machines` makes a node as an
  in-process node agent; `exec` and `workloads` run a behavioural model of the process (exit code,
  output, duration from a seeded generator) instead of a process; `endpoints`, `edge`, `dns`,
  `certificates` route in memory; `volumes`, `images`, `objects`, `secret-store` keep bytes in maps.
  No file, socket or process outside the test's own.
- Ownership: the port definitions move to cloudspec; weft keeps the wiring of the adapters
  (`cloudspec@2ed72c2:docs/platform-ports.md`, as for the other placements).

## Scope

- `ess/1-infrastructure/infrastructure.yaml` (the `Memory` variant; guards that name `Local` for
  access mode), `cloudspec@2ed72c2:docs/platform-ports.md` § 8 (a column `memory`)
- `product/cloud/crates/weft-platform/src/memory/` (proposed, beside `src/local/`)

## Acceptance

1. `ess specify validate --path ess --strict-requires` passes with `Memory` in
   `weft.infrastructure.Placement`, and `CreateCloud` with placement `Memory` and single-user access
   is accepted.
2. With placement `Memory` and one seed, a node pool's node is made, joins, and a snippet instance
   reads `Running` then exits with the modelled exit code; two runs with the same seed write the
   same report file byte for byte; no process is started (the test counts child processes).

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed.
