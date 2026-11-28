---
format: aep.planning-md/3
id: story:ports-node-workloads-local
kind: story
status: draft
title: The `workloads` port on substrate's workload family
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-exec-facts
- depends_on: story:skeleton-node-run-assignment
scope:
- confidence: cited
  path: product/cloud/crates/weft-agent
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local/substrate.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/node.rs
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`cloudspec@2ed72c2:docs/platform-ports.md` § 5 N3 (`workloads`), § 8 (column `local`), § 10 gap 1. Depends on
`story:ports-node-exec-facts`. Outside the first build (design § 12): it starts when substrate
serves gap 1 (a long-lived, supervised process from a root filesystem on the host driver).

## Context

- Operations: `start(key, spec)`, `stop(key, grace)`, `restart(key)`, `pause(key)`,
  `resume(key)`, `checkpoint(key) -> snapshot ref`, `observe(key)`, `list()`. Key:
  `(assignment_id, process)`. Spec: root filesystem (N6), `argv`, `env`, mounts (N5), endpoints
  (N4), limits, isolation level, restart policy `never`: weft decides restarts itself
  (`node.MarkRestartDue`, `RestartProcess`, `ess/5-runtime/node.yaml:370-415`).
- `observe` answers `{state: starting | running | paused | exited | stopped | unknown, last_exit,
  observed_at}`. Refusals: `IsolationNotServed { level }`, `ImageNotReady`, `NotServed` (for
  example `checkpoint`, until substrate gap 8), `Invalid`, `Unavailable`. A second `start` for a
  running key answers its observation.
- Serves: `node.StartProcess`, `RestartProcess`, `StopProcess`, `StopAssignmentProcesses`;
  `nodes.Stop`, `Pause`, `Resume`, `Snapshot`; `observability.OpenLogStream`.
- Until then, the `local` adapter does not serve it; a process that ends within exec bounds runs
  through N1 (design § 8).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.nodes.Assignment` -> `weft.workloads.Instance`, many-to-one, references — inferable:
  `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Assignment`, relation `instance`.
- `weft.node.LocalProcess` -> `weft.nodes.Assignment` and -> `weft.workloads.Instance`, many-to-one,
  references — inferable: `ess/5-runtime/node.yaml`, entity `weft.node.LocalProcess`, relations
  `assignment` and `instance`.

## Scope

- `product/cloud/crates/weft-platform/src/node.rs`, `src/local/substrate.rs`
- `product/cloud/crates/weft-agent/`

## Acceptance

A `Service` instance whose process runs until stopped moves `Starting` -> `Running` in
`weft.workloads.Instances` through the `workloads` port, and after `nodes.Stop` its process has
ended and the port's `observe` answers `stopped`.

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
