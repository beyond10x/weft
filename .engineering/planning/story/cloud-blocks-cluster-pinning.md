---
format: aep.planning-md/3
id: story:cloud-blocks-cluster-pinning
kind: story
status: draft
title: PlaceInstance still refuses a node outside the formation's tenant cluster on the vendored spec
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-account-scope
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-e, a formation stays in its tenant cluster.

## Outcome

1. `weft.cluster` records which cluster a formation runs in (the blocks dropped the field).
2. `PlaceInstance` reads it and keeps its three isolation refusals; isolation defaults to deny.
3. The binding `cluster-create-node-pool` follows the vendored command names.

## Scope

`ess/3-resources/cluster.yaml`, `ess/5-runtime/scheduling.yaml`, `ess/components.yaml` (binding
`cluster-create-node-pool` only).

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports nothing in `cluster.yaml` or
   `scheduling.yaml`.
2. The conformance scenario of `PlaceInstance` with a node outside the formation's cluster ends in
   its refusal and records no `InstancePlaced`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
