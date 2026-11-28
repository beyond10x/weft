---
format: aep.planning-md/3
id: story:ports-substrate-gaps
kind: story
status: draft
title: The substrate gaps of § 10 handed to the substrate maintainers
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
scope:
- confidence: cited
  path: docs/platform-ports.md
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "the substrate gaps". Design:
`cloudspec@2ed72c2:docs/platform-ports.md` § 10. Depends on none.

## Context

- The 12 gaps of design § 10, ordered by weft's first need, each matched to a track of
  `substrate@0.29.0:ROADMAP.md:24-33` or "not on its roadmap": 1 long-lived supervised process
  (N3), 2 inbound endpoint (N4), 3 per-instance local endpoint (N4), 4 secret value per run (C6
  through N3), 5 images without a Docker daemon (N6), 6 read-write volume (N5), 7 egress per run
  (N4), 8 freeze, thaw, checkpoint (N3), 9 network byte counters (N2), 10 `MicroVm` (N3), 11 exec on
  the Kubernetes namespace driver (N1), 12 accelerators (N7).
- Each gap is handed to the substrate maintainers; § 10 records, per gap, the id of the story in
  substrate's planning store or their recorded answer.
- The stories of the node layer that wait on a gap name its number in their context:
  `story:ports-node-workloads-local` (1), `story:ports-node-endpoints-local` (2, 3),
  `story:ports-node-images-local` (5), `story:ports-node-volumes-local` (6),
  `story:ports-kubernetes-adapters` (11).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None: a document change.

## Scope

- `cloudspec@2ed72c2:docs/platform-ports.md` (§ 10)

## Acceptance

Each of the 12 gaps of design § 10 moves from open to handed over: it names a story id in
substrate's planning store or the recorded answer of the substrate maintainers.

