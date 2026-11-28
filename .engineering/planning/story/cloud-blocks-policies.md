---
format: aep.planning-md/3
id: story:cloud-blocks-policies
kind: story
status: draft
title: Bindings, owners and scope grants follow the vendored domains, and the combined spec validates
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-account-scope
- depends_on: story:cloud-blocks-cluster-pinning
- depends_on: story:cloud-blocks-runtime-fields
scope:
- confidence: cited
  path: ess/6-delivery/deployments.yaml
- confidence: cited
  path: ess/components.yaml
revision: 3
---
## Epic

Decomposes `epic:cloud-blocks`: policies and owners after the move; the combined spec validates.

## Outcome

1. `components.yaml`: the 5 binding policies the blocks left empty (`already-requested`,
   `already-counted` cut) get a policy again; owners of the domains `architecture`, `scope`,
   `topology`, `limits` and `services`; the edge accepts.
2. `6-delivery/deployments.yaml`: `RollbackPlanned` stops setting `rollback_of`, which D7 dropped
   from `Plan`.

## Scope

`ess/components.yaml` (all but the binding `cluster-create-node-pool`, its conversion and the
`AccountCreated -> OpenScope` binding), `ess/6-delivery/deployments.yaml`.

## Acceptance

1. `ess specify validate --path ess --strict-requires` exits 0.
2. `tools/check-stamped-time.sh` exits 0.
3. The compiled model (`ess specify compile --path ess --format json`) names an owning component
   for each of `weft.architecture`, `weft.scope`, `weft.topology`, `weft.limits` and
   `weft.services`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
