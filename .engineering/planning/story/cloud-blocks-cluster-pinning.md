---
format: aep.planning-md/3
id: story:cloud-blocks-cluster-pinning
kind: story
status: proposed
title: PlaceInstance still refuses a node outside the formation's tenant cluster on the vendored spec
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-account-scope
scope:
- confidence: cited
  path: ess/3-resources/cluster.yaml
- confidence: cited
  path: ess/5-runtime/scheduling.yaml
- confidence: cited
  path: ess/components.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:22Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-e, a formation stays in its tenant cluster.

## Outcome

1. `weft.cluster` records which cluster a formation runs in (the blocks dropped the field).
2. `PlaceInstance` reads it and keeps its three isolation refusals; isolation defaults to deny.
3. `NodePool.requested_for` is read again by RecordClusterNodePoolReady,
   RecordRemovedClusterNodePool and the view TenantNodePools.
4. The binding `cluster-create-node-pool` and its `ClusterNodePoolId` conversion follow the
   vendored command names.

## Scope

`ess/3-resources/cluster.yaml`, `ess/5-runtime/scheduling.yaml`, `ess/components.yaml` (binding
`cluster-create-node-pool` and its `conversions:` entry only; `story:cloud-blocks-policies` edits
the rest after this story).

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports nothing in `cluster.yaml`,
   `scheduling.yaml` or the binding `cluster-create-node-pool`.
2. The scenario check of the isolation refusal runs after the spec compiles, in
   `story:cloud-blocks-policies` acceptance 4.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
