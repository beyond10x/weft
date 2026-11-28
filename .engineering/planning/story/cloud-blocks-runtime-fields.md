---
format: aep.planning-md/3
id: story:cloud-blocks-runtime-fields
kind: story
status: active
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:22Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T08:51:30Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the run-time fields the blocks dropped get a weft row.

## Outcome

1. `ConfigureAccess` and the view `CloudAccessOf` read `access_mode` from a weft row.
2. `ScaleControlPlane` sets `control_plane` on a weft row.

## Scope

`ess/1-infrastructure/infrastructure.yaml`, `ess/2-tenancy/access.yaml`.

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports nothing in `infrastructure.yaml`
   or `access.yaml`.
2. The scenario checks of `ConfigureAccess` and `ScaleControlPlane` run after the spec compiles,
   in `story:cloud-blocks-policies` acceptance 4.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
