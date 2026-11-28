---
format: aep.planning-md/3
id: story:ports-hosts-adapters
kind: story
status: draft
title: Adapters of the `hosts` placement
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-placement
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/hosts
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other adapters (hosts, aws, kubernetes) are designed,
not built". Design: `cloudspec@2ed72c2:docs/platform-ports.md` § 8, column `hosts`. Depends on
`story:ports-spec-placement` (a control host serves one placement).

## Context

- `hosts`: hosts the operator brought, each with `substrate-daemon`.
- C1 `machines` is not served: the operator brings the host and records it with `ProvisionNode`
  (`ess/1-infrastructure/infrastructure.yaml:930-932`); the node joins with its token.
- Node layer: as `local` (N1 and N2 on the substrate host driver of each host).
- Cloud layer: C2 weft-edge on a public interface of a host; C3 the DNS of the operator through
  connectors, or not served; C4 ACME; C5 a directory on the control host, nodes fetch over the node
  channel; C6 `ClusterStore`; C7 and C9 not served.
- The node channel across the network needs the mutual TLS of the node session, a later story
  (design § 7).
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

- `product/cloud/crates/weft-platform/src/hosts/`

## Acceptance

A host the operator recorded with `ProvisionNode` joins over the network with its token, its node
moving `Provisioning` -> `Ready`, and runs an instance.

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
