---
format: aep.planning-md/3
id: story:ports-aws-adapters
kind: story
status: draft
title: Adapters of the `aws` placement through connectors
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-placement
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/aws
revision: 2
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other adapters (hosts, aws, kubernetes) are designed,
not built". Design: `docs/platform-ports.md` § 8, column `aws`, and § 11. Depends on
`story:ports-spec-placement`. Outside the first build: it waits for the connectors gaps of
design § 11 (no AWS provider with writes; standing grants per operation and instance).

## Context

- C1 `machines`: an instance from a launch template per pool, through connectors; weft holds no
  vendor credential, only the bearer of a served connectors host (design § 2).
- Node layer: substrate host driver on each instance.
- Cloud layer: C2 a load balancer, C3 the vendor's DNS, C4 the vendor's certificate service, C5
  the vendor's object store, C6 `ClusterStore` or `ExternalVault` = the vendor's secret service,
  C7 block volumes, C8 weft on its own nodes or the vendor's managed database, C9 the vendor's
  private network and its security groups; all through connectors.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references; the field
  `node_pool_id` is absent for a node not made by a pool — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation `pool`.
- `weft.infrastructure.Node` -> `weft.infrastructure.Cloud`, many-to-one, references — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation
  `cluster`.
- The machine is no `ess/1` entity; the port keys it by `node_id` (design § 6 C1).

## Scope

- `product/cloud/crates/weft-platform/src/aws/`

## Acceptance

A pool's node is made as an instance through connectors and joins, its node moving
`Provisioning` -> `Ready`.

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
