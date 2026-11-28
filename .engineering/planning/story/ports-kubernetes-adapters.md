---
format: aep.planning-md/3
id: story:ports-kubernetes-adapters
kind: story
status: draft
title: Adapters of the `kubernetes` placement
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-placement
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/kubernetes
revision: 2
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other adapters (hosts, aws, kubernetes) are designed,
not built". Design: `docs/platform-ports.md` § 8, column `kubernetes`, § 10 gap 11 and § 11.
Depends on `story:ports-spec-placement`. Outside the first build: it waits for exec on the
Kubernetes namespace driver on an enforcing network plugin (substrate gap 11) and for the
connectors gap of design § 11 (whether the Kubernetes provider writes pods).

## Context

- C1 `machines`: a `weft-agent` pod per node of the pool through the Kubernetes API (connectors
  provider); the autoscaler of the cluster owns its machines.
- Node layer: substrate's Kubernetes namespace driver; exec admitted only where its probe observed
  confinement.
- Cloud layer: C2 a Service of type LoadBalancer or an Ingress; C3 not served; C4 a Certificate
  object of the issuer of the cluster; C5 an object store through connectors; C6 Secret from the
  backend; C7 volume -> PVC; C9 Link -> NetworkPolicy.
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

- `product/cloud/crates/weft-platform/src/kubernetes/`

## Acceptance

A pool's node is made as a `weft-agent` pod through connectors, its node moving `Provisioning` ->
`Ready`, and runs an exec on the namespace driver.

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
