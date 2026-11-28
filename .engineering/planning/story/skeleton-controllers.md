---
format: aep.planning-md/3
id: story:skeleton-controllers
kind: story
status: implemented
title: 'weft-controllers: placement and assignment repair in the control host, with the PlaceInstance and Assign obligations'
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
- depends_on: story:skeleton-replica-instances
scope:
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/placement.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/repair.rs
- confidence: cited
  path: product/cloud/crates/weft-controllers/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/assign.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/place_instance.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings
- confidence: cited
  path: product/cloud/crates/weft-impl/src/conversions.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/lib.rs
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T22:37:32Z", actor: "human:operator", revision: 24, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T22:37:32Z", actor: "human:operator", revision: 25, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T00:17:29Z", actor: "human:operator", revision: 26, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 4 (placement and assignment repair) and outcome 3 (the
obligations, conversion and restored binding policies of the placement part of the path). Depends
on `story:skeleton-replica-instances` (`product/cloud/crates/weft-controllers`, the controller loop, the
binding runner). Blocked by no ESS gap: it delivers its four bindings by hand, with their E2
policies.

## Context

Steps, in `product/cloud/crates/weft-controllers` (`tick` of `story:skeleton-replica-instances`):

- Scheduler decision: `weft.scheduling.PendingRequests`, candidates from
  `weft.infrastructure.Nodes` -> `weft.scheduling.PlaceInstance` as Scheduler, `decided_at`
  stamped from the controller clock (`weft.shared.StampedAt`; Scheduler is in
  `tools/stamped-time-actors`). `PlaceInstance` refuses a `decided_at` after its `now`
  (`ess/5-runtime/scheduling.yaml` line 894), and the context port reads the same clock. On a
  refusal the next candidate is tried, isolation from the weakest the request admits.
- R4 level (repair): `weft.scheduling.UnassignedPlacements` -> `weft.nodes.Assign` as Scheduler,
  `assigned_at` from the controller clock; an `already-assigned` answer is followed by
  `weft.scheduling.RecordPlacementAssigned` for the row.
- Clock: a parameter; tests read the wall clock before the tick and after its answer and bound each
  stamped time by those two reads.

The edges are hand-run: `tools/synth-input.sh` removes `r4-bind-placement`, `r4-assign`,
`r4-schedule-instance` and `r4-record-placement-assigned` from the synthesis input (the generated
System has no seam for a per-refusal policy), and this story adds their four entries to the binding
runner of `story:skeleton-replica-instances`, each with a comment naming ESS `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`,
mapped and with the policy `ess/components.yaml` declares:
- `r4-bind-placement`: `InstancePlaced` -> `BindPlacement`; drops `gang-member`, `wrong-state`;
- `r4-assign`: `PlacementBound` -> `Assign` (`assigned_at` from the event's `decided_at`);
  drops `already-assigned`, `placement-not-bound`, `no-open-session`;
- `r4-schedule-instance`: `PlacementBound` -> `ScheduleInstance`; drops `wrong-state`;
- `r4-record-placement-assigned`: `InstanceAssigned` -> `RecordPlacementAssigned`; drops
  `not-bound`;
each retries every other refusal.

Obligations implemented here, each in its own file under `product/cloud/crates/weft-impl/src/behaviours/`:
`weft.scheduling.PlaceInstance` and `weft.nodes.Assign` (2 of the 24 command behaviours on the
skeleton path); and the conversion `weft.shared.StampedAt -> Timestamp`
(`product/cloud/crates/weft-impl/src/conversions.rs`), which the path needs where a stamped instant lands in a
row. Not in the skeleton: `DeclareUnschedulable`, the R2, R3, R6-R9 steps, the cluster pool steps
(the skeleton deploys into the default cluster).

Tests run in-process on a state seeded through the storage port (`weft-testkit`), holding just the
rows a step reads and the rows those reference, then one `tick`, then reads of the generated views
and of the control host log after the last `seq` read before the tick.

## Domain relations

- `weft.scheduling.PlacementDecision` -> `weft.scheduling.SchedulingRequest`, many-to-one,
  references — inferable: `ess/5-runtime/scheduling.yaml`, entity
  `weft.scheduling.PlacementDecision`, relation `request`.
- `weft.nodes.Assignment` -> `weft.scheduling.PlacementDecision`, many-to-one, references —
  inferable: `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Assignment`, relation
  `placement_decision`.
- `weft.nodes.Assignment` -> `weft.workloads.Instance` and `weft.infrastructure.Node`, each
  many-to-one, references — inferable: `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Assignment`,
  relations `instance`, `node`.
- `weft.nodes.NodeSession` -> `weft.infrastructure.Node`, many-to-one, references (the seeded open
  session of the candidate node) — inferable: `ess/5-runtime/nodes.yaml`, entity
  `weft.nodes.NodeSession`, relation `node`.
- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references (candidate
  node and its pool in `PlaceInstance`) — inferable: `ess/1-infrastructure/infrastructure.yaml`,
  entity `weft.infrastructure.Node`, relation `pool`.

## Scope

- `product/cloud/crates/weft-controllers/` (`src/placement.rs`, `src/repair.rs`, `tests/main.rs` modules
  `placement` and `repair`)
- `product/cloud/crates/weft-impl/` (`src/behaviours/`: `place_instance.rs`, `assign.rs`;
  `src/conversions.rs`; the four `r4-*` entries of the binding runner in `src/bindings/`)

## Acceptance

1. From a seeded state holding a pending scheduling request and a Ready node with an open session
   that offers `Container`, one `tick` puts a row for that request and node in
   `weft.scheduling.PlacementDecisions` whose `decided_at` lies between the wall-clock reads taken
   before the tick and after its answer.
2. After the same tick, the hand-run `r4-*` bindings leave an `Active` row for that instance and
   node in `weft.nodes.Assignments` whose `assigned_at` (the stamped `decided_at`, through the
   conversion `StampedAt -> Timestamp`) lies between the same two wall-clock reads, and the instance
   in state `Scheduled` with that `node_id` in `weft.workloads.Instances`.
3. From a seeded state whose only Ready node has no open session, one `tick` adds no row to
   `weft.scheduling.PlacementDecisions` and the request stays pending, the negative side of
   statement 1.
4. From a seeded state holding a bound, unassigned placement, one `tick` of the repair step leaves
   an `Active` row for that placement in `weft.nodes.Assignments` whose `assigned_at` lies between
   the wall-clock reads taken before the tick and after its answer.
5. In-process, events replayed or constructed through the binding runner give, in the log after the
   `seq` read before each, for each `r4-*` binding one refusal on its drop list with result
   `dropped` and one off it with result `retried`:
   - `r4-bind-placement`: `wrong-state` (an `InstancePlaced` of a bound decision) `dropped`;
     `no-such-placement` (an event naming no decision) `retried`;
   - `r4-assign`: `already-assigned` (a `PlacementBound` of an assigned placement) `dropped`;
     `node-not-ready` (its node cordoned) `retried`;
   - `r4-schedule-instance`: `wrong-state` (an instance already `Scheduled`) `dropped`;
     `no-such-instance` (an event naming no instance) `retried`;
   - `r4-record-placement-assigned`: `not-bound` (a released placement) `dropped`;
     `no-such-placement` (an event naming no decision) `retried`.
## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh (`yq` v4 may be called from sh).
- No load tests, no VMs, no GPU workloads.
- The test step makes no request that leaves the host: the control host, the node agent and
  substrate-daemon run on loopback and a Unix socket; seeded rows or fixtures stand in for
  anything else.
