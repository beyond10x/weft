---
format: aep.planning-md/3
id: story:cloud-blocks-sim-pin
kind: story
status: draft
title: One pin serves cloudspec-sim and the blocks, and the simulator inputs name the renamed commands
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-f, one pin for cloudspec-sim and the blocks.

## Outcome

1. `CLOUDSPEC_REV` in `Taskfile.yml` is the blocks' pin and the simulator's.
2. The 5 experiments and `sim/relations/globex-defines-in-shop.yaml` name the renamed commands.

## Scope

`sim/experiments/`, `sim/relations/globex-defines-in-shop.yaml`, `Taskfile.yml`
(`CLOUDSPEC_REV` only).

## Acceptance

1. `task sim-relations` (slow lane; it fetches) runs its full setup to the last step at the pin.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
