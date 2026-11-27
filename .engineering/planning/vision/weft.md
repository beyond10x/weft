---
format: aep.planning-md/3
id: vision:weft
kind: vision
status: draft
title: 'weft: a compute cloud a team deploys itself, run from its ESS specification'
revision: 1
---
## Vision

weft is a compute cloud a team deploys itself: a control plane and worker nodes that run confined
workloads, from a code snippet to an agent or an ESS system, for tenants and their nested
accounts (`ess/system.yaml` summary, `docs/domain-sketch.md`).

## Objectives

1. The specification under `ess/` is valid and sound, with hardening passes recorded
   (`docs/goal.md`, the owner, 2026-11-26).
2. weft runs from that specification: Spindle runs its domains, weft's code runs what Spindle does
   not (controllers, node agent, deciders), and the conformance suite synthesized from it runs
   against the running system with its coverage counted (the owner, 2026-11-27 about 08:35 CET,
   "start building weft"; `docs/implementation.md`).
