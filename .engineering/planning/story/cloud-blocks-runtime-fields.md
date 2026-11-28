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
revision: 1
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
2. The conformance scenario of `ConfigureAccess` followed by `CloudAccessOf` returns the
   configured `access_mode`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
