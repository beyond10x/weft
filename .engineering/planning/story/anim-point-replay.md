---
format: aep.planning-md/3
id: story:anim-point-replay
kind: story
status: draft
title: 'Replay a search point by its walk number: weft-sim reruns that point and its page shows the timeline'
relations:
- decomposes: epic:search-animation
- serves: vision:weft
- depends_on: story:anim-search-record
- depends_on: story:anim-search-player
scope:
- confidence: cited
  path: tools/sim/src/main.rs
- confidence: cited
  path: tools/sim/src/page.html
- confidence: cited
  path: tools/sim/src/simulate.rs
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: website/docs/simulator/run.md
- confidence: inferred
  path: website/ux/fixtures/sim-player.html
- confidence: inferred
  path: website/ux/sim-player.spec.ts
revision: 6
---
## Epic

Decomposes `epic:search-animation`, outcome 3. Depends on `story:anim-search-record` (the `walk`
of each row) and `story:anim-search-player` (the page that names the replay of the selected
point).

Order: fourth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
this story -> `story:kpi-sim-fields` -> `story:kpi-specification` -> `story:kpi-platform-gates` ->
`story:kpi-flat-searches` -> `story:kpi-simulator` -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). `tools/sim/src/simulate.rs` and `tools/sim/tests/main.rs` are edited after
it by `story:kpi-sim-fields` and `story:kpi-flat-searches`, and `tools/sim/src/page.html` by
`story:kpi-flat-searches`. `website/docs/simulator/run.md` is edited after it by
`story:docs-figure-search-animation`, which depends on this story. `story:docs-commands-check`
depends on this story too: its committed `weft-sim --help` snapshot mirrors the `USAGE` text of
`tools/sim/src/main.rs` this story edits, and it checks the `$ ` lines of `run.md`.

## Context

`weft-sim replay --spec DIR --trace DIR/result.json --walk K` today replays an `explore` walk: it
looks up walk K in `traces` (`tools/sim/src/main.rs:394`) and sends its recorded steps again. A
search point of `simulate` has no trace: its walk ran with `record` false and only its KPIs were
kept. Its run is reproducible instead: the experiment path is in `run.experiment`
(`tools/sim/src/main.rs:358`), the row holds its `point` and `walk`, and `simulate_one` takes the
walk number.

After this story, `replay` on a `simulate` result with a search row of walk K:

- loads the experiment of `run.experiment`, applies the row `point`, and runs it once at walk K
  with its timeline recorded;
- prints each KPI of the rerun beside the recorded one, and exits 0 when all are equal and 2 when
  any differs (as a replay whose answers differ does today);
- writes `result.json` and `index.html` to `--out` (default `sim/out/replay-<K>`), whose
  Experiment timeline section shows that point run;
- refuses a walk number no row has, naming the walk and the trace file, with exit status 2.

The `explore` path of `replay` is unchanged. On the player, the selected row (a click on a cell
or a path mark, or the row of the current frame) shows its walk number in an element
`data-selected-walk` and the replay command line in an element `data-replay-command`, with the
relative paths of the run. The page change changes the page the Rust test of
`story:anim-search-player` renders, so this story writes `website/ux/fixtures/sim-player.html`
again with the command that test names.

Tests:

- Rust: one test on the fixture search of `story:anim-search-record` replays a `rejected` row and
  a `restart` row by their walks and compares the KPIs; each ends in a few seconds.
- Playwright: a test "selected point shows its replay" in `website/ux/sim-player.spec.ts` (the
  spec of `story:anim-search-player`, run by `slot npm --prefix website run ux -- --workers=2`).
- `website/docs/simulator/run.md` gets a section `### Replay a search point` with a `$ ` line of
  `weft-sim replay` that passes `--walk`.

## Domain relations

None of `ess/`.

- Search row to walk, one-to-one within a run. Inferable (inferred from
  `tools/sim/src/simulate.rs:1535`, a point is simulated with `base_walk + i`; recorded as `walk`
  by `story:anim-search-record`).
- Simulate result to experiment file, many-to-one, the result names the file it ran. Inferable
  (inferred from `tools/sim/src/main.rs:358`, `run.experiment`).

## Scope

- `tools/sim/src/main.rs` (`replay_cmd`: the `simulate` search path and `--out`; usage text)
- `tools/sim/src/simulate.rs` (a rerun of one recorded point at its walk with the timeline kept)
- `tools/sim/src/page.html` (walk number and replay command of the selected row)
- `tools/sim/tests/main.rs` (replay tests)
- `website/ux/sim-player.spec.ts` (the selection test), `website/ux/fixtures/sim-player.html`
  (written again)
- `website/docs/simulator/run.md` (section Replay: replaying a search point)

## Acceptance

1. `weft-sim replay --trace <result.json of a fit run> --walk K` for the walk of a recorded search
   row prints each KPI of the rerun beside the recorded value and exits 0 when they are all equal.
2. The same command writes `index.html` to its `--out` directory whose Experiment timeline section
   shows the setup, traffic, faults and controller ticks of that point.
3. A walk number that no search row and no trace of the result holds is refused with a message
   naming the walk and the file, and exit status 2.
4. Selecting a point on the player shows its walk number and the command that reruns it: on the
   fixture page, clicking the `[data-path-step]` of a `rejected` row sets the text of
   `[data-selected-walk]` to that row's `walk` in the inlined snapshot, and the text of
   `[data-replay-command]` starts with `weft-sim replay`, holds `--walk <that walk>` and no
   absolute path (`website/ux/sim-player.spec.ts`, test "selected point shows its replay").
5. `website/docs/simulator/run.md` shows how to replay a search point by its walk number:
   `grep -n '^### Replay a search point' website/docs/simulator/run.md` prints the heading, and
   `grep -n '^\$ weft-sim replay .*--walk' website/docs/simulator/run.md` prints a line below it.

## Rules for this work

- The animation reads recorded output only: the page shows the command, it runs nothing.
- `cargo` is shimmed through `slot`; `slot task check` and `task sim-test` keep their prefix. A
  script that only measures or waits runs without it. A pool job is killed after 10 minutes unless
  it names its time (`slot --timeout`, at most 30m).
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- No Python in anything committed: Rust or POSIX sh.
- The repository and its site are public: no person, host, session or private repository name in
  a file, a test or a committed output.
