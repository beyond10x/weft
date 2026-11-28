---
format: aep.planning-md/3
id: story:agent-loom-runtime
kind: story
status: draft
title: weft runs an Agent workload as a Loom agent
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-run-local-job
revision: 1
---
# weft runs an Agent workload as a Loom agent

## Context

Decomposes `epic:agent-workloads`. The owner, 2026-11-28 (relayed by the agentic operator): an
agent stays an agent workload through cloudspec, and "on a runtime level, weft will turn that
agent into a Loom agent". The spec already names the source kind `LoomAgent`
(`ess/6-delivery/deployments.yaml:34`) and the format `loom.agent/1`
(`docs/domain-sketch.md:346`, `:725-726`).

- weft writes a `loom.agent/1` document from the `Agent` workload's settings
  (`story:agent-package-settings`) and starts Loom from the workload's `runtime` artifact.
- Agents that run a coding harness go through Metaharness behind an executor the host supplies
  (Loom's README): the executor runs inside the workload.
- Model access goes through a gateway reached by a named egress destination; the model key is a
  secret delivered at start (substrate gap 4, `docs/platform-ports.md` § 10).
- `story:agent-run-local-job` runs a fixture runtime; this story replaces it with Loom.
- The step agenticspec agent part -> cloudspec agent workload is cloudspec's
  (`translate-in-agenticspec`, `agent-workload-kind` in beyond10x/cloudspec); this story owns
  cloudspec agent workload -> `loom.agent/1` -> a running Loom process.

## Acceptance

An `Agent` workload of shape Job on placement local with a pinned Loom runtime artifact runs
Loom with the `loom.agent/1` document weft wrote, reaches the model only through the named
gateway destination, and its instance moves `Starting` -> `Running` -> `Succeeded`.

## Scope

- product/cloud/crates/weft-agent/ (start of an Agent instance)
- product/cloud/crates/weft-testkit/ (Loom fixture)
