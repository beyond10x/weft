---
format: aep.planning-md/3
id: story:anim-search-record
kind: story
status: active
title: 'weft-sim search output records the climb: round, axis indices, parent, decision, best so far and walk per point; restart points evaluated'
relations:
- decomposes: epic:search-animation
- serves: vision:weft
- depends_on: story:kpi-repository
scope:
- confidence: cited
  path: tools/sim/src/experiment.rs
- confidence: cited
  path: tools/sim/src/simulate.rs
- confidence: inferred
  path: tools/sim/tests/fixture-search
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: website/docs/simulator/results.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T15:01:08Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1,"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-27T15:01:08Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1,"review_outcome":2}}}
---
## Epic

Decomposes `epic:search-animation`, outcome 1. First of the epic: the player, the replay and the
published run all read the columns this story writes.

Order: second link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> this story -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> `story:kpi-flat-searches` -> `story:kpi-simulator` ->
`story:kpi-ci-runs` -> `story:kpi-docs-site`). It depends on `story:kpi-repository`, which adds
its module line to `tools/sim/tests/main.rs` first; `story:kpi-sim-fields` and
`story:kpi-flat-searches` edit `tools/sim/src/simulate.rs` and `tools/sim/tests/main.rs` after it,
and `story:kpi-flat-searches` takes its fitness spread on the restart semantics this story sets.

## Context

`weft-sim simulate` with a `search` block evaluates points of the mutation space (`fit`: a hill
climb over neighbours with random restart; `exhaustive`: every combination up to `limit`).
`record_point` (`tools/sim/src/simulate.rs:1543`) writes per point only `point`, `kpis`,
`fitness`, `feasible` and `error`, in evaluation order. `design:quality-kpis` § Search over KPIs
lists what an animated view needs and is missing; this story writes all of it, and nothing the
page draws.

Per search row (each element of `search` in `result.json` and in the live snapshot):

- `seq`: its position, from 1.
- `round`: the evaluation batch it was part of (one `evaluate` call). `fit`: the start is round 1,
  each round of neighbours and each restart a round of its own. `exhaustive`: every point is
  round 1.
- `index`: per mutation axis, the index of the value into that axis `values` list (values may be
  maps such as `topology`, so the page plots indices, not values).
- `parent`: the `seq` of the point it was a neighbour of; `null` for the start, a restart and
  every `exhaustive` point.
- `decision`: `fit`: `start` for the first point; `accepted` for the neighbour of a round that
  became the current point; `rejected` for every other neighbour; `restart` for a random restart.
  `exhaustive`: `start` for the first point, then `accepted` when the point raised the best so
  far and `rejected` otherwise.
- `best`: the best feasible fitness after this point, `null` while no point was feasible.
- `walk`: the walk number the point was simulated with (`base_walk + i` of `evaluate`). Walk
  numbers are unique within a run; walk 0 stays the rerun of the best point with its timeline.

Restart points: today the restart after a round without improvement sets `current` without
evaluating it (`tools/sim/src/simulate.rs:1660`, `current_score = f64::MIN`), so that point is
never recorded. After this story the restart is drawn among points not visited, evaluated in a
round of its own, recorded with decision `restart`, counts against `budget`, and becomes the
current point with its own score (its fitness when feasible). The restart taken when a point has
no unvisited neighbour (line 1627) is recorded the same way. When every point is visited the
search ends, as it does today.

Run header: the search writes `info.search` into the snapshot before its first point is
evaluated, so the live page has it from the start: `mode`, `seed`, `budget`, `limit`, `weights`,
`gates`, and `axes` (each mutation `name`, `kind`, `point` and its `values` list in order).

Tests: one fit search and one exhaustive search on a test fixture of their own
(`tools/sim/tests/fixture-search/`: a small spec and an experiment with two mutation axes whose
fitness differs between points). Time each test before adding it; each ends in a few seconds and
none runs a search near the 60 s bound.

## Domain relations

None of `ess/`: the relations below are of the search output of weft-sim, which no `ess/1`
document declares.

- Search run to search row, one-to-many (zero when the run fails before its first point), the run
  owns its rows: they exist only in its `result.json` and live snapshot. Inferable (inferred from
  `tools/sim/src/coverage.rs:93`, `search: Vec<Value>` of the run coverage, written by
  `tools/sim/src/simulate.rs:1548`).
- Search row to parent row, many-to-one, zero for start, restart and exhaustive rows; the parent
  is always an earlier row of the same run. Inferable (inferred from
  `tools/sim/src/simulate.rs:1615`, neighbours are built from `current` one axis step away).
- Search row to walk, one-to-one within a run. Inferable (inferred from
  `tools/sim/src/simulate.rs:1535`, each point is simulated with `base_walk + i`, and line 1670,
  the best point rerun with walk 0).
- Search row to axis value, one index per mutation axis of the experiment; the experiment owns
  its axes and their `values`. Inferable (inferred from `tools/sim/src/simulate.rs:1601`,
  `to_point` maps indices to values, and `tools/sim/src/experiment.rs:219`, `Mutation.values`).
- Round to search row, one-to-many. Inferable (inferred from `tools/sim/src/simulate.rs:1639`,
  one `evaluate` call per round of neighbours).

## Scope

- `tools/sim/src/simulate.rs` (`run`, `record_point`, the restart paths)
- `tools/sim/tests/main.rs` (search tests), `tools/sim/tests/fixture-search/` (new fixture)
- `website/docs/simulator/results.md` (the `search` rows and `info.search` in the result.json
  table)

## Acceptance

1. Every row of `search` in `result.json` of a `fit` or `exhaustive` run carries `seq`, `round`,
   `index` (one value index per mutation axis), `parent`, `decision`, `best` and `walk`, beside
   `point`, `kpis`, `fitness`, `feasible` and `error` as before.
2. In a `fit` run on the test fixture, the first row has decision `start` and no parent, each
   round holds at most one `accepted` row, every `accepted` and `rejected` row names as parent
   the current point of its round, and that parent differs from it in exactly one axis index by
   one.
3. In a `fit` run on the test fixture where a round finds no improvement, the next row has
   decision `restart`, no parent, its own KPIs and fitness, and an index tuple no earlier row has
   (a test in `tools/sim/tests/main.rs`, run by `task sim-test`).
4. In a `fit` run on the test fixture with a `budget` smaller than the number of points and at
   least one restart, the number of rows of `search` in `result.json`, restart rows counted, is at
   most `budget` (a test, run by `task sim-test`).
5. In every row of a `fit` and an `exhaustive` run on the test fixture, `best` equals the largest
   fitness of the feasible rows up to and including it, and is `null` while none is feasible (a
   test, run by `task sim-test`).
6. In a `fit` run on the test fixture, the `walk` numbers of the rows are pairwise distinct and
   none is 0 (a test, run by `task sim-test`).
7. With `weft-sim simulate --serve` on the test fixture, `/snapshot` read before the first row
   exists holds `info.search` with the mode, seed, budget, limit, weights, gates and every axis
   with its values in order (a test, run by `task sim-test`).
8. `info.search` of the written `result.json` of the same run equals the object of item 7 (a
   test, run by `task sim-test`).
9. `website/docs/simulator/results.md` names each new row field and `info.search` with its
   meaning.

## Rules for this work

- `cargo` is shimmed through `slot`; `slot task check` and `task sim-test` keep their prefix. A
  script that only measures or waits runs without it. A pool job is killed after 10 minutes unless
  it names its time (`slot --timeout`, at most 30m).
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- No Python in anything committed: Rust or POSIX sh.
- The repository and its site are public: no person, host, session or private repository name in
  a file, a test or a committed output.
