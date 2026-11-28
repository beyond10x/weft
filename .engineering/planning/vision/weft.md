---
format: aep.planning-md/3
id: vision:weft
kind: vision
status: approved
title: 'weft: a compute cloud a team deploys itself, run from its ESS specification'
revision: 4
transitions:
- {from: "draft", to: "in_review", at: "2026-11-27T09:54:36Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "in_review", to: "approved", at: "2026-11-27T09:54:36Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
## Vision

weft is the runtime of cloudspec specifications (`beyond10x/cloudspec`): it realizes a cloud
specified in cloudspec on a placement (local, hosts, aws, kubernetes) through ports and adapters
(`epic:platform-ports`), with a control plane and worker nodes that run confined workloads, from a
code snippet to an agent, an agent architecture or an ESS system. cloudspec is the intermediate
representation: every source (a Helm chart, a Compose file, an agenticspec deployment) translates
into cloudspec, and weft runs only cloudspec specifications.

Its first example, and the use case this repository builds first, is the enterprise multi-tenant
cloud a team deploys itself: tenants and their nested accounts, access, catalog, metering and
consoles (`ess/system.yaml` summary, `docs/domain-sketch.md`). Those product layers are the
example's, not the runtime's core.

The owner, 2026-11-28 about 02:08 CET, typed in a session: "weft is the runtime for a cloudspec. In
weft there is one use case of that, one example: the enterprise multi-tenant cloud." About 02:14
CET (decision D5): cloudspec is the lowest common point; every source translates into it, every
target is generated out of it, and weft runs only cloudspec specifications.

## Objectives

1. The specification under `ess/` is valid and sound, with hardening passes recorded
   (`docs/goal.md`, the owner, 2026-11-26).
2. weft runs from that specification: Spindle runs its domains, weft's code runs what Spindle does
   not (controllers, node agent, deciders), and the conformance suite synthesized from it runs
   against the running system with its coverage counted (the owner, 2026-11-27 about 08:35 CET,
   "start building weft"; `docs/implementation.md`).
