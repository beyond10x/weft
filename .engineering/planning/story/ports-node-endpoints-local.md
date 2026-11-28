---
format: aep.planning-md/3
id: story:ports-node-endpoints-local
kind: story
status: draft
title: 'The `endpoints` port: backends, checks and the local endpoint'
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
`cloudspec@2ed72c2:docs/platform-ports.md` § 5 N4 (`endpoints`), § 8, § 10 gaps 2 and 3 (and 7 for `allow_egress`).
Depends on `story:ports-node-workloads-local`. Outside the first build: it starts when substrate
serves an inbound endpoint into a confined process (gap 2) and a per-instance local endpoint
(gap 3).

## Context

- Operations: `expose(key, port, exposure: loopback | node) -> observed address`,
  `withdraw(key, port)`, `tunnel(key, port)` (`protocols/exec-channel.yaml:12-13`),
  `local_endpoint(key)` with the namespace identity of the caller
  (`ess/5-runtime/node.yaml:568-571`), `allow_egress(key, destination)`
  (`ess/5-runtime/claims.yaml:389-391`).
- Refusals: `ExposureRefused`, `PortInUse`, `UnknownInstance`, `EgressNotAllowed`, `NotServed`,
  `Unavailable`. Keyed by `(key, port)` and `(key, destination)`. The answer carries the observed
  address, which may differ from the requested one.
- Serves: the backends of the edge (`edge.RecordBackendReady`), `formations.AddLink`, HTTP and TCP
  checks of `workloads.RecordCheckResult`, `node.RequestIdentityDocument`, `RequestSecret`, port
  forward.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.edge.RouteBackend` -> `weft.workloads.Instance`, many-to-one, references — inferable:
  `ess/5-runtime/edge.yaml`, entity `weft.edge.RouteBackend`, relation `instance`.
- `weft.node.LocalEndpointSession` -> `weft.workloads.Instance`, many-to-one, references —
  inferable: `ess/5-runtime/node.yaml`, entity `weft.node.LocalEndpointSession`, relation
  `instance`.

## Scope

- `product/cloud/crates/weft-platform/src/node.rs`, `src/local/substrate.rs`
- `product/cloud/crates/weft-agent/`

## Acceptance

An HTTP check of an instance's port passes through the `endpoints` port, and once the instance's
backend reads `Ready` in `weft.edge.RouteBackend` the edge forwards a request to it.

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
