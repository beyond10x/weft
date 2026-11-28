---
format: aep.planning-md/3
id: story:ports-node-volumes-local
kind: story
status: draft
title: 'The `volumes` port: node-local volumes'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-workloads-local
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
`cloudspec@2ed72c2:docs/platform-ports.md` § 5 N5 (`volumes`), § 8, § 10 gap 6. Depends on
`story:ports-node-workloads-local`. Outside the first build: it starts when substrate mounts a
read-write volume into an exec or workload (gap 6).

## Context

- Operations: `create(volume_id, size)`, `attach(volume_id, key, path, read_only)`,
  `detach(volume_id, key)`, `snapshot(volume_id) -> snapshot ref`, `delete(volume_id)`,
  `observe(volume_id)`.
- Refusals: `InUse` (delete while attached), `NoSpace`, `NotServed`, `Unavailable`. Keyed by
  `volume_id`.
- Serves: `resources.CreateVolume` (node-local), `SnapshotVolume`, `nodes.Attach`, `Detach`,
  `pipelines.CreateBuildCache`; recorded by `ReportVolumeProvisioned` (NodeAgent).
- On `local`, C7 `zone-volumes` is this port (one host is one zone, design § 8).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.resources.Volume` -> `weft.infrastructure.Node`, many-to-one, references (node-local when
  `node_id` is set) — inferable: `ess/3-resources/resources.yaml`, entity `weft.resources.Volume`,
  relation `node`.
- `weft.nodes.Attachment` -> `weft.nodes.Assignment`, many-to-one, references — inferable:
  `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Attachment`, relation `assignment`.

## Scope

- `product/cloud/crates/weft-platform/src/node.rs`, `src/local/substrate.rs`
- `product/cloud/crates/weft-agent/`

## Acceptance

A node-local volume that moves `Provisioning` -> `Available` through the `volumes` port is written
by one instance and read by the next instance it is attached to.

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
