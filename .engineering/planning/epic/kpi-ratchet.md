---
format: aep.planning-md/3
id: epic:kpi-ratchet
kind: epic
status: active
title: KPI catalogue held by quality/ratchet.yaml
relations:
- serves: vision:weft
- derived_from: design:quality-kpis
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T10:52:40Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T10:52:40Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
## Goal

The owner, 2026-11-27: "define kpis for when something is good (repo, ci, the cloud platform, the
individual components, etc) - and then run/exhaust the state space to find better scoring ones.
maybe we should start with kpis". Then, on the catalogue (`design:quality-kpis`): "do it, keep
going".

This epic turns the catalogue into checked numbers: every KPI that can be measured today is held by
`quality/ratchet.yaml`, so a search and a gate read the same values.

## Outcome

1. Every KPI of `design:quality-kpis` marked measurable (`yes`) or `probe` has an assessment in
   `quality/ratchet.yaml` with its subject, assessor and measurement, and its measured baseline as
   the accepted reference; each gate KPI has a requirement in the `merge` gate.
2. weft-sim writes the KPI fields the catalogue names as missing (`crossings-per-message`,
   `instances-lost`, `unschedulable-requests`, `placement-imbalance`, `node-hours-per-workload`)
   and `weft-sim-report@1` reads every field; every experiment gates on every `platform.*` gate
   KPI (the node-loss pattern match `healthy-node-lost` is held, not only `violations`).
3. The two flat searches (duplicate-delivery, cluster-pool-lifecycle) get mutation points whose
   values change at least one objective KPI, or are marked exhaustive-only with the reason.
4. `task kpis` prints every value with its command, outside the test step (it measures; it is not
   run through slot), until `ratchet` checks the file in the gate.

## Not in this epic

- KPIs of components that are not built yet (control host, node agent, controllers, generated
  crates, conformance target): they join as `epic:walking-skeleton` delivers them.
- The search animation (`epic:search-animation`) and the docs checkers (`epic:docs-fidelity`).

## Rules

`~/beyond10x/AGENTS.md` § Tests stay fast: no KPI measurement runs inside the test step; an
expensive measurement is made once and read by the gate.
