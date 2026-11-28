---
format: aep.planning-md/3
id: story:slot-node-pool-placement
kind: story
status: draft
title: A scope placed on a named host or pool runs its workloads there, one instance per node, with per-scope instances and the instance word
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:slot-node-process
scope:
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: cited
  path: product/cloud/crates/weft-controllers/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
- confidence: inferred
  path: product/cloud/crates/weft-testkit
revision: 4
---
## Epic

Decomposes `epic:slot-node-daemon`, outcome 2 and the first step of its Order (placement). cloudspec
rules this lets cloudspec map: `weft_placement_unsupported` (2 refusals of slot: scope `workstation`
on host `workstation`, scope `build-node` on pool `build-nodes`), `weft_per_instance_unsupported`
(1: `toolchain-refresh` per `build-node`), `weft_instance_unsupported` (1: its `${instance}` word):
4 of 44.

## Dependencies

- `story:slot-node-process`: the process workloads to place. PlanFromSource is needed first, through
  that story (`story:cloudspec-submission-plan`).
- Not stopped by, and not covering: what happens to an instance when its node leaves the pool
  (drain, lease end, gone) is `decision-blocker:slot-node-per-node-instance-lifecycle`. Making the
  machines of a pool is `story:ports-local-pool-node`; tests here provision pool nodes with
  `weft.infrastructure.ProvisionPoolNode` as the skeleton does.

## Outcome

1. `PlanFromSource` reads `placement` of a scope part (`weft.scope.Placement`,
   `ess/cloud/3-scope/scope.yaml:21-32`): with `pool: <name>` each workload of the scope is placed
   only on `Active` nodes of the node pool of that name (the `node_pool_id` input of
   `PlaceInstance` and its guard `node-pool-differs`,
   `product/cloud/crates/weft-impl/src/behaviours/place_instance.rs:294-300`); with `host: <name>`
   only on the node of that name. A name that matches no pool or node refuses the plan by a named
   error, nothing planned. `lease.max_hours` is recorded with the scope, not enforced here.
2. One instance per node: a level step of `weft-controllers` gives each `Active` node of the pool
   one instance of each workload of a pool scope, and never a second.
3. `per: <scope>`: one instance per instance of that scope (an instance of a pool scope is a node of
   the pool), placed in the workload's own scope; each `${instance}` word of its command,
   environment and paths is the name of that scope instance, written at instance creation.

## Domain relations

- `weft.scope.Placement.pool` → `weft.infrastructure.NodePool` by name and `.host` → a node by
  name, many-to-one: inferable (inferred from `ess/cloud/3-scope/scope.yaml:21-24`, the type comment
  "a node pool of layer 2 by name", "a named host, a node of layer 2"; no relations: entry).
- `weft.workloads.Instance` → `weft.infrastructure.Node`, one per (workload, node) for a pool scope
  and for `per`: inferable (inferred from cloudspec `59129eb:crates/cloudspec/src/generate/systemd.rs:25-26`
  and `59129eb:blocks/5-workloads/workloads.yaml:4513-4514`; no relations: entry). Its lifecycle
  when the node leaves is the open blocker above.

## Scope

`product/cloud/crates/weft-controllers/src/` (a new step beside `placement.rs`); worked out:
`product/cloud/crates/weft-cloudspec/src/`, `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-testkit/`.

## Acceptance

With pool `build-nodes` of two `Active` nodes and a node `workstation`, the planned architecture
(scope `build-node` on the pool with one process, scope `workstation` on the host with one process
and one process `per: build-node` naming `${instance}`) runs one instance of the pool process on
each pool node and none elsewhere, the host process on `workstation` only, and two `per` instances
on `workstation` whose commands carry the two node names; a third pool node gets its own two.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- No network request in the test step; no machine is made. No Python in anything committed.
