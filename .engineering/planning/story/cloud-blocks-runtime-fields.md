---
format: aep.planning-md/3
id: story:cloud-blocks-runtime-fields
kind: story
status: draft
title: Access mode and control-plane size live on weft rows the vendored commands read and set
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-account-scope
scope:
- confidence: cited
  path: ess/1-infrastructure/infrastructure.yaml
- confidence: cited
  path: ess/2-tenancy/access.yaml
revision: 3
---
## Epic

Decomposes `epic:cloud-blocks`: the run-time fields the blocks dropped get a weft row.

## Outcome

1. `ConfigureAccess` and the view `CloudAccessOf` read `access_mode` from a weft row.
2. `ScaleControlPlane` sets `control_plane` on a weft row.

## Scope

`ess/1-infrastructure/infrastructure.yaml`, `ess/2-tenancy/access.yaml`.

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports nothing in either file.
2. `slot --timeout 30m tools/synth-domain.sh access` reports 0 failed scenarios of
   `weft.access`, and its suite holds a passing scenario of `ConfigureAccess` whose outcome sets
   `access_mode`.
3. `slot --timeout 30m tools/synth-domain.sh infrastructure` reports 0 failed scenarios, and its
   suite holds a passing scenario of `ScaleControlPlane` whose outcome sets `control_plane`.
   (Both runs take about 80 s each and run outside the test step, one at a time.)

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
