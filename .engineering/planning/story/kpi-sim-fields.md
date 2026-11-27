---
format: aep.planning-md/3
id: story:kpi-sim-fields
kind: story
status: draft
title: weft-sim writes crossings-per-message, instances-lost, unschedulable-requests, placement-imbalance and node-hours-per-workload, gates on the whole chain, and weft-sim-report@1 declares every Kpis field
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:anim-point-replay
scope:
- confidence: cited
  path: quality/assessors/weft-sim-report@1.assessor.yaml
- confidence: cited
  path: tools/sim/src/simulate.rs
- confidence: inferred
  path: tools/sim/tests/fixtures
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
- confidence: inferred
  path: website/ux/fixtures/sim-player.html
revision: 11
---
## Epic

Decomposes `epic:kpi-ratchet`, outcome 2, first half: weft-sim writes the KPI fields the catalogue
names as missing, and its assessor declaration reads every field.
`story:kpi-platform-gates` puts the fields into the experiments and `quality/ratchet.yaml`.

Order: fifth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> this story -> `story:kpi-specification` -> `story:kpi-platform-gates`
-> `story:kpi-flat-searches` -> `story:kpi-simulator` -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). It depends on `story:anim-point-replay`: `tools/sim/src/simulate.rs` and
`tools/sim/tests/main.rs` are edited before it by `story:kpi-repository` and the three `anim-`
stories. `quality/assessors/weft-sim-report@1.assessor.yaml` is edited after it by
`story:kpi-specification`, `story:kpi-platform-gates` and `story:kpi-flat-searches`.

## Outcome

A developer reads in every `result.json` of `weft-sim simulate` five figures that were missing
(`crossings-per-message`, `instances-lost`, `unschedulable-requests`, `placement-imbalance`,
`node-hours-per-workload`), can make a search reject a point whose chain stopped early, and finds
an output for every `Kpis` field in `weft-sim-report@1`.

## Context

- `tools/sim/src/simulate.rs:231` `struct Kpis` holds the per-run counts; `Kpis::get`
  (`simulate.rs:262`) reads a field by its KPI name with `-` for `_`, and `fitness`
  (`simulate.rs:1511`) makes a point infeasible when a `search.gates` KPI is not 0.
- `design:quality-kpis` § 2 names the missing fields; `design:simulation-and-kpis` § KPIs names
  `instances-lost`, `unschedulable-requests` and `node-hours` per workload.
- `quality/assessors/weft-sim-report@1.assessor.yaml` declares six outputs; none for a `Kpis` field
  but `violations` and `abstract_states`.
- `platform.chain-reached` is a gate "at the chain length" in the catalogue, but a search gate holds
  only "is 0", so no experiment can gate on it today.

Definitions (taken here; each is a deterministic count per seed):

- `crossings_per_message`: `link_crossings / messages`, 0 when `messages` is 0. A ratio, written as
  a number with fractions.
- `instances_lost`: instances (`weft.workloads.Instance`) in state `Running` just before the first
  `node_loss` fault that are not `Running` at the end of the run, after the settle; 0 in a run with
  no `node_loss` fault.
- `unschedulable_requests`: scheduling requests (`weft.scheduling.SchedulingRequest`) in state
  `Unschedulable` at the end of the run.
- `placement_imbalance`: the largest number of `Active` assignments (`weft.nodes.Assignment`) on
  one node divided by the mean over the nodes the setup joined and the run did not lose; 0 when no
  assignment is active.
- `node_hours_per_workload`: for each controller tick, each `Active` assignment adds one node-tick
  to the workload of its instance; the field is the mean over workloads that had an instance. Unit
  node-ticks.
- `chain_missing`: chain length minus `chain_reached`, so `gates: [chain-missing]` makes a point
  infeasible when the chain stopped early, under the existing "gate is 0" rule.

Test of the declaration: `ratchet::every_kpis_field_is_a_declared_output` in
`tools/sim/tests/ratchet/mod.rs` (the module of `story:kpi-repository`) enumerates the fields of
`Kpis` from its JSON form and holds each one to an output of `weft-sim-report@1`, the direction
`ratchet::the_ratchet_file_is_consistent` (measurement to declared output) does not cover. The
new fields change the search rows of the fixture page that the Rust test of
`story:anim-search-player` renders, so this story writes `website/ux/fixtures/sim-player.html`
again with the command that test names.

## Domain relations

- `weft.nodes.Assignment` -> `weft.infrastructure.Node`, many-to-one, references (the node
  outlives its assignments) — inferable from `ess/5-runtime/nodes.yaml`, entity
  `weft.nodes.Assignment`, relation `node` (`via: node_id`). Used by `placement_imbalance`.
- `weft.nodes.Assignment` -> `weft.workloads.Instance`, many-to-one, references (the instance
  outlives each assignment) — inferable from `ess/5-runtime/nodes.yaml`, entity
  `weft.nodes.Assignment`, relation `instance` (`via: instance_id`). Used by
  `node_hours_per_workload`.
- `weft.workloads.Instance` -> `weft.workloads.Workload`, many-to-one, references — inferable from
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Instance`, relation `workload`
  (`via: workload_id`). Used by `node_hours_per_workload`.
- The states read (`Assignment.Active`, `Instance.Running`, `SchedulingRequest.Unschedulable`) are
  lifecycle states of those entities in the same documents, not relations.

## Acceptance

1. `run.result.kpis` of `result.json` holds `crossings_per_message`, `instances_lost`,
   `unschedulable_requests`, `placement_imbalance`, `node_hours_per_workload` and `chain_missing`
   for every committed experiment, and each search point records them too.
2. `crossings_per_message`: a weft-sim test on a small fixture whose run sends messages over
   links gives `link_crossings / messages` (for example 6 crossings over 4 messages gives 1.5),
   and a fixture run that sends no message gives 0 (`task sim-test`).
3. `instances_lost`: a weft-sim test on a small fixture whose `node_loss` fault leaves a
   `Running` instance not running at the end gives a value above 0, and the same fixture without
   the fault gives 0 (`task sim-test`).
4. `unschedulable_requests`: a weft-sim test on a small fixture with one scheduling request no node
   can take gives 1, and the same fixture with room on a node gives 0 (`task sim-test`).
5. `placement_imbalance`: a weft-sim test on a small fixture gives a larger value for a run whose
   active assignments sit on one node than for a run that spreads them evenly, and 0 for a run with
   no active assignment (`task sim-test`).
6. `node_hours_per_workload`: a weft-sim test on a small fixture with one workload whose one
   instance holds one `Active` assignment for N controller ticks gives N, and a fixture of two such
   workloads with N and M ticks gives (N + M) / 2 (`task sim-test`).
7. A search with `gates: [chain-missing]` marks a point whose chain stopped early as infeasible
   (a weft-sim test, `task sim-test`).
8. `weft-sim-report@1.assessor.yaml` declares one output for every field of `Kpis`, the new ones
   included, with its value type, kind and unit: `ratchet::every_kpis_field_is_a_declared_output`
   (new, in `tools/sim/tests/ratchet/mod.rs`) enumerates the keys of
   `serde_json::to_value(weft_sim::simulate::Kpis::default())` and fails naming each key the
   declaration has no output for; it passes on the tree and fails on a copy of the declaration
   with the `instances_lost` output removed (`task sim-test`).
9. The committed experiments give the same values for every old field as before the change (same
   seed; `task sim-experiments` before and after, compared field by field).
10. `task sim-test` stays under 120 s with no test over 60 s; every new test is timed in the
    report.

## Out of Scope

- `branch-points-without-guard` (catalogue `spec.*`, not in this epic’s outcome 2).
- Putting the fields into experiment weights, gates or `quality/ratchet.yaml`
  (`story:kpi-platform-gates`).
- Usage records in the simulation (metering rows).
- The search recording of `epic:search-animation`.

## Rules for the work

- Tests run on small fixtures, no test over 60 s, `task sim-test` under 120 s; no KPI measurement in
  the test step.
- A build or test job in the pool ends within 10 minutes.
- No Python in anything committed.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- `node-hours-per-workload` "over the metering rows" (`design:quality-kpis` § 2) — taken: counted
  from `Active` assignments, because weft-sim writes no usage record (`weft.metering`) and a
  metering-based field would read 0 in every run. Reversible: the field switches to metering rows
  when the traffic writes usage.
- The chain gate — taken: a derived field `chain_missing`, which keeps the one gate rule of
  `fitness`, rather than a gate with a bound.

## Open Questions

None.
