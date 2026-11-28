---
format: aep.planning-md/3
id: story:workload-environment-and-args
kind: story
status: draft
title: A Service workload's environment text values and arguments reach its instance at start, or admission refuses it by name
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:ports-node-workloads-port
- depends_on: story:cloudspec-submission-apply-workloads
scope:
- confidence: cited
  path: ess/4-applications/workloads.yaml
- confidence: cited
  path: ess/5-runtime/scheduling.yaml
- confidence: cited
  path: product/cloud/crates/weft-agent/src
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
revision: 7
---
## Epic

Decomposes `epic:slot-node-daemon`. A consumer's requirement (2026-11-28): a Service workload's
document names environment variables with text values and command arguments, and the instance is
started with them; a workload whose environment or arguments cannot be delivered on a placement is
refused at admission with the name of what is missing. cloudspec (main 393e5a7 and later) lowers a
unit's environment text and args for a Kubernetes target; its weft target refuses them today. Secret
values at start are a separate need (substrate gap 4).

## Outcome

1. A Service workload's document carries environment variables with text values and arguments; the
   placed instance is started with exactly those, through the `Workloads` port
   (`story:ports-node-workloads-port`) and through `Exec` for a snippet.
2. Admission refuses a workload whose environment or arguments a placement cannot deliver (the
   node's runtime does not take them), naming what is missing; nothing is placed.
3. The deployment path from a cloudspec submission carries both (the per-part workload of
   `story:cloudspec-submission-apply-workloads`).

## Scope

`ess/4-applications/workloads.yaml`, `ess/5-runtime/scheduling.yaml`,
`product/cloud/crates/weft-impl/src/behaviours/`, `product/cloud/crates/weft-agent/src/`,
`product/cloud/crates/weft-testkit/tests/`, `quality/spec-diff-acknowledgements.json`.

## Acceptance

1. A testkit test defines a Service workload with two environment text values and two arguments,
   places it, and reads them from the stub port's start record (and from the snippet's exec
   request).
2. A test of a placement whose runtime cannot deliver environment variables gets the refusal naming
   `environment`, and no PlacementDecision exists.
3. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task check'` exits 0.
