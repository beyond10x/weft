---
format: aep.planning-md/3
id: story:cloud-blocks-policies
kind: story
status: active
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:22Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T09:00:36Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
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

1. `ess specify validate --path ess --strict-requires` exits 0, and `tools/check-stamped-time.sh`
   exits 0.
2. The compiled model (`ess specify compile --path ess --format json`) names an owning component
   for each of `weft.architecture`, `weft.scope`, `weft.topology`, `weft.limits` and
   `weft.services`.
3. The compiled model has a binding from `weft.accounts.AccountCreated` to `weft.scope.OpenScope`
   and grants of `OpenScope` and `CloseScope` to weft actors (`story:cloud-blocks-account-scope`).
4. Moved to `story:cloud-blocks-synth-domain-time` (2026-11-28): these runs exceeded the 30-minute
   pool limit on the vendored spec. Former text: run one at a time, outside the test step, `slot --timeout 30m
   tools/synth-domain.sh <name>` reports 0 failed scenarios of the domain for `accounts`,
   `scheduling`, `access` and `infrastructure`; the `scheduling` suite holds a scenario in which
   `PlaceInstance` on a node outside the formation's cluster ends in that refusal
   (`-cluster-pinning`); the `access` suite holds a passing `ConfigureAccess` scenario that sets
   `access_mode` and the `infrastructure` suite a passing `ScaleControlPlane` scenario that sets
   `control_plane` (`-runtime-fields`).

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
