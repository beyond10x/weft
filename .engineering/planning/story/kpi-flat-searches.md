---
format: aep.planning-md/3
id: story:kpi-flat-searches
kind: story
status: draft
title: The duplicate-delivery and cluster-pool-lifecycle searches move an objective KPI or are marked exhaustive-only with the reason; search fitness spread held
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-platform-gates
scope:
- confidence: cited
  path: quality/assessors/weft-sim-report@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: cited
  path: sim/experiments/cluster-pool-lifecycle.yaml
- confidence: cited
  path: sim/experiments/duplicate-delivery.yaml
- confidence: inferred
  path: tools/kpis.d/45-search.sh
- confidence: cited
  path: tools/sim/src/experiment.rs
- confidence: cited
  path: tools/sim/src/page.html
- confidence: cited
  path: tools/sim/src/simulate.rs
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
- confidence: inferred
  path: website/ux/fixtures/sim-player.html
revision: 12
---
## Epic

Decomposes `epic:kpi-ratchet`, outcome 3, and holds `sim.search-fitness-spread` (outcome 1).
Depends on `story:kpi-platform-gates`: the spread is measured over feasible points, and that story
sets the gates that decide feasibility in the same two experiment files.

Order: eighth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> this story -> `story:kpi-simulator` -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). Before it, `story:kpi-specification` and `story:kpi-platform-gates` edit
`quality/ratchet.yaml` and `quality/assessors/weft-sim-report@1.assessor.yaml`, and the
`anim-` stories and `story:kpi-sim-fields` edit `cloudspec@2ed72c2:crates/cloudspec-sim/src/simulate.rs`,
`cloudspec@2ed72c2:crates/cloudspec-sim/src/page.html` and `cloudspec@2ed72c2:crates/cloudspec-sim/tests/main.rs`; after it `story:kpi-simulator` edits
`quality/ratchet.yaml` (the chain gives it a `depends_on` edge to this story).

The spread is taken on the search semantics of `story:anim-search-record`, which comes before this
story in the chain: the restart of a `fit` search is evaluated, recorded and counted against
`budget`. Restart rows are search points like any other: a feasible restart row counts in the
spread, and the baselines of this story and its reference records are measured after that change.

## Outcome

Every search of `sim/experiments/` either separates its points (its fitness differs between
feasible points) or says in its file why it does not, and a search that goes flat again shows as a
failed requirement.

## Context

- `design:quality-kpis` § 2, M7: the duplicate-delivery search (`fit`, budget 12) and the
  cluster-pool-lifecycle search (`exhaustive`, 6 points) are flat: fitness 1.3 to 1.3 and -1 to -1.
  In duplicate-delivery no point produces a duplicate effect and two points are infeasible (draw 11
  refused in setup); in cluster-pool-lifecycle every point converges in 1 tick.
- § Search over KPIs: "a flat search is a design defect of the experiment, not a result".
- `sim.search-fitness-spread` (§ weft-sim itself): max minus min fitness over a search’s feasible
  points, a report today; baselines 0 (duplicate-delivery, cluster-pool-lifecycle), 10 node-loss,
  612 stuck-placement, 420 full-setup.
- The experiment format refuses unknown fields (`cloudspec@2ed72c2:crates/cloudspec-sim/src/experiment.rs`,
  `#[serde(deny_unknown_fields)]`), so a marker in a file needs a field weft-sim parses.

Shape (taken here):

- For each of the two experiments, first try mutation points whose values change at least one
  objective KPI of its `search.weights` (for duplicate-delivery: faults on the steps whose effects
  are not idempotent by construction, a draw near the quota; for cluster-pool-lifecycle: a pool
  removed while instances are placed, a fault during the drain). Keep a point only if the search
  then shows a spread above 0 under the gates of `story:kpi-platform-gates`.
- An experiment that keeps no such point gets a field in its `search:` block, parsed by weft-sim,
  that marks it exhaustive-only with the reason in words (it is run for gap finding and pattern
  matches over the whole space, not to climb); weft-sim prints the mark in its summary and the
  page shows it.
- weft-sim writes the spread of a search into `result.json`; `weft-sim-report@1` declares it;
  `quality/ratchet.yaml` measures it for every experiment with a search, with a requirement in the
  `merge` gate that it is above 0 for every experiment not marked exhaustive-only.
- `tools/kpis.d/45-search.sh` prints `sim.search-fitness-spread` per experiment with its command,
  reading the results `task sim-experiments` wrote.
- Tests in `cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs` (the module and helpers of `story:kpi-repository`,
  file-only, `task sim-test`): `ratchet::every_search_spread_is_measured` (every file of
  `sim/experiments/` with a `search:` block has a measurement of `sim.search-fitness-spread` in
  its assessment), `ratchet::every_unmarked_search_requires_a_spread` (each such experiment
  without the exhaustive-only mark has a requirement above 0 on that measurement listed in
  `merge`), and `ratchet::check_runs_no_search` (the `check` and `test` tasks of `Taskfile.yml`
  with every task they call, and the files of `tools/check.d/`, name neither `sim-experiments`
  nor `tools/kpis.d/45-search.sh`).
- The page change (the exhaustive-only mark) changes the page the Rust test of
  `story:anim-search-player` renders, so this story writes `website/ux/fixtures/sim-player.html`
  again with the command that test names.

## Domain relations

None: no ESS entity relation is read beyond the experiments’ existing setup. The ratchet/1
relations are those of `story:kpi-repository` § Domain relations.

## Acceptance

1. Running the duplicate-delivery search gives a fitness spread above 0 over its feasible points,
   or its file carries the exhaustive-only mark with the reason.
2. Running the cluster-pool-lifecycle search gives a fitness spread above 0 over its feasible
   points, or its file carries the exhaustive-only mark with the reason.
3. weft-sim refuses an exhaustive-only mark without a reason and an experiment in `fit` mode that
   carries the mark (tests, each under 1 s).
4. `quality/ratchet.yaml` measures the search fitness spread of every experiment with a search:
   `ratchet::every_search_spread_is_measured` passes on the tree and fails on a copy of the file
   with one such measurement removed (`task sim-test`).
5. The `merge` gate requires the spread above 0 for every experiment with a search not marked
   exhaustive-only: `ratchet::every_unmarked_search_requires_a_spread` passes on the tree and
   fails on a copy with one such requirement removed (`task sim-test`).
6. `task kpis` prints `sim.search-fitness-spread` for each experiment with a search, with its
   command.
7. `task check` runs no search: `ratchet::check_runs_no_search` passes on the tree and fails on a
   copy of `Taskfile.yml` whose `check` task calls `task: sim-experiments` (`task sim-test`).

## Out of Scope

- The search recording and animation (`epic:search-animation`).
- Specification variants as a mutation kind.
- Evaluating the restart point of `fit` (`design:quality-kpis` § Search over KPIs;
  `epic:search-animation` outcome 1): done by `story:anim-search-record` before this story.

## Rules for the work

- No test runs a search longer than a few seconds; no test over 60 s; `task sim-test` under 120 s;
  no KPI measurement in the test step.
- No Python in anything committed.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- `sim.search-fitness-spread` is a report in the catalogue — taken: a requirement above 0 for an
  unmarked search, because outcome 3 makes a flat search a defect; reversible by dropping the
  requirement.

## Open Questions

None.
