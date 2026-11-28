---
format: aep.planning-md/3
id: story:ports-cloud-edge-local
kind: story
status: draft
title: 'The `edge` port: weft-edge on a loopback listener'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-endpoints-local
scope:
- confidence: cited
  path: product/cloud/crates/weft-bin
- confidence: cited
  path: product/cloud/crates/weft-platform/src/cloud.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local
revision: 2
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`docs/platform-ports.md` § 6 C2 (`edge`), § 8 (`local`: weft-edge on a loopback port). Depends on
`story:ports-node-endpoints-local` (backends need an inbound endpoint).

## Context

- Operations: `publish(listener {hosts, port}) -> observed public address`, `withdraw(listener)`,
  `observe(listener)`. Routing to ready backends stays in weft-edge (`weft.edge.RouteBackend`,
  `ess/5-runtime/edge.yaml:24-33`); the port only gives weft-edge an address the public reaches.
- Refusals: `PortInUse`, `AddressExhausted`, `NotServed`, `Unavailable`. Keyed by listener (host
  set and port).
- Serves: `formations.AddEndpoint`, `RemoveEndpoint`, `AddRoute`, `RemoveRoute`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.edge.RouteBackend` -> `weft.workloads.Instance`, many-to-one, references — inferable:
  `ess/5-runtime/edge.yaml`, entity `weft.edge.RouteBackend`, relation `instance`.

## Scope

- `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/`
- `product/cloud/crates/weft-bin/`

## Acceptance

A request to the loopback listener with a route's host reaches a backend once that backend moves
to `Ready` in `weft.edge.RouteBackend`.

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
