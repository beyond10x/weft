---
format: aep.planning-md/3
id: story:cloud-blocks-sim-pin
kind: story
status: active
title: One pin serves cloudspec-sim and the blocks, and the simulator inputs name the renamed commands
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
scope:
- confidence: cited
  path: sim/experiments
- confidence: cited
  path: sim/relations/globex-defines-in-shop.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:23Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T10:10:15Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-f, the simulator runs on the blocks' pin.

## Outcome

1. The 5 experiments and `sim/relations/globex-defines-in-shop.yaml` name the renamed commands
   and `scope_id`.
2. The simulator is installed from `CLOUDSPEC_REV` (set by `story:cloud-blocks-vendor`); this
   story does not change the pin.

## Scope

`sim/experiments/`, `sim/relations/globex-defines-in-shop.yaml`.

## Acceptance

1. `task sim-experiments` exits 0 with `CLOUDSPEC_REV` at the pin of `story:cloud-blocks-vendor`
   (was 2ed72c2). Slow lane; it fetches.
2. `task sim-relations` exits 0 at the same pin.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
