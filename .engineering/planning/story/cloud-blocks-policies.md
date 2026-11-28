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
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: policies, owners and grants after the move; the spec validates.

## Outcome

1. `components.yaml`: the 5 bindings that name moved commands follow them; owners of the domains
   `architecture`, `scope`, `topology`, `limits` and `services`; the edge accepts.
2. `6-delivery/deployments.yaml` keeps `rollback_of`.
3. `OpenScope` and `CloseScope` are granted to weft's actors.

## Scope

`ess/components.yaml` (all but `cluster-create-node-pool`), `ess/6-delivery/deployments.yaml`,
`ess/9-actors/actors.yaml` (grants of OpenScope and CloseScope only).

## Acceptance

1. `ess specify validate --path ess --strict-requires` exits 0.
2. `tools/check-stamped-time.sh` exits 0.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
