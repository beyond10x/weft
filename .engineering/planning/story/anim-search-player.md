---
format: aep.planning-md/3
id: story:anim-search-player
kind: story
status: active
title: weft-sim page plays a search run over two chosen axes, live with --serve and from a finished result.json
relations:
- decomposes: epic:search-animation
- serves: vision:weft
- depends_on: story:anim-search-record
- depends_on: story:docs-ux-harness
scope:
- confidence: cited
  path: tools/sim/src/page.html
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: website/docs/simulator/results.md
- confidence: inferred
  path: website/ux/fixtures/sim-player.html
- confidence: inferred
  path: website/ux/sim-player.spec.ts
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1,"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"approval":1,"review_outcome":5}}}
---
## Epic

Decomposes `epic:search-animation`, outcome 2. Depends on `story:anim-search-record`: the page
plays the rows and the run header that story writes. Depends on `story:docs-ux-harness`: the
browser checks of the player are a Playwright spec of its harness in `website/ux/`.

Order: third link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> this story ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> `story:kpi-flat-searches` -> `story:kpi-simulator` ->
`story:kpi-ci-runs` -> `story:kpi-docs-site`). `tools/sim/tests/main.rs` is edited before it by
`story:kpi-repository` and `story:anim-search-record`, and after it by `story:anim-point-replay`,
`story:kpi-sim-fields` and `story:kpi-flat-searches`; `tools/sim/src/page.html` after it by
`story:anim-point-replay` and `story:kpi-flat-searches` (the exhaustive-only mark).

## Context

The page (`tools/sim/src/page.html`, section Search, around line 250) draws one table of the
search points and marks the final best. It is one file with its data inlined: `report::page`
(`tools/sim/src/report.rs:267`) puts the snapshot into it for `index.html`, and with `--serve` the
same page reads the snapshot from `/events` (`tools/sim/src/serve.rs`, one full snapshot about
every 600 ms) while the run goes on.

This story adds a player above the table, in `page.html` only, with inline SVG and script (no
library, no request other than `/events` in live mode). It reads recorded rows only: it runs no
simulation, and it computes nothing a row does not hold except positions on the screen.

- Axes: two pickers over `info.search.axes` (default the first two). With one axis the landscape
  is a single row.
- Landscape: one cell per pair of value indices of the two chosen axes, the other axes fixed at
  the index tuple of the best point at the frame shown. A cell is coloured by the fitness of the
  row at that tuple once the frame has reached it; infeasible and failed rows are marked as such;
  cells not yet evaluated stay empty. Value labels on the axes come from the axis `values`.
- Climber path up to the frame: an edge from parent to each `accepted` row, `rejected` neighbours
  as marks of their own, a `restart` as a jump drawn apart from a move; rows off the shown slice
  are drawn projected onto it and faded.
- Beside it the best-so-far curve: `best` over `seq`, with the frame marked.
- Controls: play, pause, step (one row forward and back), speed. The frame is one row (`seq`).
- Live: rows that arrive extend the timeline; when the player is at the last row and playing, it
  follows them.
- A result without the new fields (written before `story:anim-search-record`) shows the table and
  a line saying the run has no recorded climb.

Player state: the player keeps its state in one object and writes it on its root element as
`data-frame` (the `seq` shown), `data-playing` (`true` or `false`) and `data-speed` (rows per
second), and the chosen axes as `data-x-axis` and `data-y-axis`. What it draws carries data
attributes, so a test reads the drawing from the DOM and not from a screenshot:

- each landscape cell: `data-cell="<i>,<j>"` (value indices on the x and y axis), `data-state`
  (`empty`, `feasible`, `infeasible`, `failed`) and, once evaluated, `data-fitness`;
- each axis label: `data-axis-label="x"` or `"y"` with `data-value-index`, its text the value;
- each path mark: `data-path-step="<seq>"` and `data-decision` (`start`, `accepted`, `rejected`,
  `restart`); an accepted edge also `data-parent="<seq of the parent>"`;
- each point of the best-so-far curve: `data-curve-point="<seq>"` and `data-best`;
- the line shown for a run without a recorded climb: `data-no-climb`.

The page exposes `window.weftPlayer.receive(snapshot)`; the live mode calls it for each
snapshot of `/events`, and a test can call it with a snapshot of its own. Play advances on the
page timer, so a fake clock drives it.

Tests:

- Rust, in `tools/sim/tests/main.rs` (run by `task sim-test`): renders `report::page` from the
  snapshot of a fixture search (the fixture of `story:anim-search-record`), finds the player and
  the inlined rows, and compares the rendered page byte for byte with the committed
  `website/ux/fixtures/sim-player.html`; the test names the command that writes it again. That
  committed page is the one input of the browser checks, so they build nothing.
- Playwright, `website/ux/sim-player.spec.ts` in the harness of `story:docs-ux-harness` (run by
  `slot npm --prefix website run ux -- --workers=2` locally and by the `pages.yml` docs job): opens
  the committed fixture page with `page.clock` installed, drives the controls by role and name, and
  reads the `data-*` state of the player root and counts the drawn elements by the attributes
  above after advancing the clock. The expected numbers come from the snapshot inlined in the
  fixture page (its `info.search.axes` and rows), read by the spec, not from the player. It listens
  for `pageerror`, and a script error fails the test. Each test ends in a few
  seconds; none comes near the 60 s bound, and the harness step stays under 120 s.
- Screenshots of a `fit` run at three frames stay in the evidence of the story for the reader; no
  acceptance item rests on them.

## Domain relations

None of `ess/`. The player reads the relations `story:anim-search-record` records (row to parent,
row to axis value index), each cited there.

## Scope

- `tools/sim/src/page.html` (section Search: player, landscape, path, best-so-far curve, controls)
- `tools/sim/tests/main.rs` (page rendering test, byte comparison with the fixture page)
- `website/ux/fixtures/sim-player.html` (new, the rendered fixture page)
- `website/ux/sim-player.spec.ts` (new, the browser checks of the player)
- `website/docs/simulator/results.md` (the Search row of the page table: what the player shows and
  its controls)

## Acceptance

All items with a spec file run as `slot npm --prefix website run ux -- --workers=2`.

1. Landscape: on the fixture page at the last frame, the count of `[data-cell]` elements equals
   the product of the value counts of the two chosen axes in the inlined snapshot's
   `info.search.axes` (`website/ux/sim-player.spec.ts`, test "landscape cells").
2. Axis labels: on the fixture page, the texts of the `[data-axis-label="x"]` elements in
   `data-value-index` order equal the `values` of the x axis in the inlined snapshot, and the same
   for y (`website/ux/sim-player.spec.ts`, test "axis labels").
3. Step: on the fixture page at frame k, pressing Step forward raises the count of
   `[data-path-step]` by 1 (the new one has `data-path-step` k+1) and the count of
   `[data-curve-point]` by 1, and changes `data-state` of at most one `[data-cell]`; pressing Step
   back restores all three counts (`website/ux/sim-player.spec.ts`, test "step adds one row").
4. Path marks: at the last frame of the fixture `fit` run (which holds a restart), the counts of
   `[data-path-step]` with `data-decision` `accepted`, `rejected` and `restart` equal the counts of
   rows with that decision in the inlined snapshot, and each accepted mark's `data-parent` equals
   its row's `parent` (`website/ux/sim-player.spec.ts`, test "decision marks").
5. Curve end: at the last frame, the `data-best` of the last `[data-curve-point]` equals the
   `best` of the last row of the inlined snapshot (`website/ux/sim-player.spec.ts`, test "curve
   ends at the best").
6. Play: on the fixture page at frame 1, pressing Play and advancing the clock until every row is
   due leaves `data-frame` at the last `seq` and `data-playing` at `false`
   (`website/ux/sim-player.spec.ts`).
7. Pause: on the fixture page while playing, pressing Pause and then advancing the clock by 5 s
   leaves `data-frame` where it was when Pause was pressed and `data-playing` at `false`
   (`website/ux/sim-player.spec.ts`).
8. Speed: on the fixture page at frame 1, setting the speed to 2 and then to 4 rows per second and
   advancing the clock by 1 s from the same frame each time moves `data-frame` by 2 and by 4 rows
   (`website/ux/sim-player.spec.ts`).
9. Axis picker: on the fixture page at frame k, choosing the second axis for x and the first for
   y sets `data-x-axis` and `data-y-axis` to those names, leaves `data-frame` at k, and gives the
   cell `[data-cell="j,i"]` the `data-state` that `[data-cell="i,j"]` had before, for every cell
   (`website/ux/sim-player.spec.ts`, test "axis picker redraws"; the fixture search has two axes).
10. Live follow: on the fixture page playing at the last row, calling `window.weftPlayer.receive`
   with a snapshot of three more rows and advancing the clock moves `data-frame` to the new last
   `seq`; at an earlier row the same call leaves `data-frame` on that row
   (`website/ux/sim-player.spec.ts`).
11. Live wiring: with `weft-sim simulate --serve` on the fixture search, the live page passes each
    snapshot of `/events` to `window.weftPlayer.receive` (the inlined script names that call; the
    Rust page test finds it).
12. Old result, table: given through `window.weftPlayer.receive` the inlined snapshot with
    `decision`, `parent`, `best` and `walk` removed from every row, the fixture page shows one
    row of the points table per search row and one `[data-no-climb]` element, and no
    `[data-path-step]` (`website/ux/sim-player.spec.ts`, test "result without a climb").
13. Old result, no error: in the same test, no `pageerror` event fires from page load to the end
    of the test.
14. The committed `website/ux/fixtures/sim-player.html` equals the page rendered from the fixture
    search (the Rust page test, run by `task sim-test`).

## Rules for this work

- The animation reads recorded output only; no simulation runs in the browser.
- `cargo` is shimmed through `slot`; `slot task check` and `task sim-test` keep their prefix. A
  script that only measures or waits runs without it. A pool job is killed after 10 minutes unless
  it names its time (`slot --timeout`, at most 30m). Playwright runs with `--workers=2` through
  `slot` (`slot npm --prefix website run ux -- --workers=2`).
- No test runs over 60 s (nextest and the Playwright config kill it); each test step ends within
  120 s. A test over 5 s is named with its time in the report.
- No Python in anything committed: Rust, POSIX sh, TypeScript in `website/`, or the script inlined
  in `page.html`.
- The repository and its site are public: no person, host, session or private repository name in
  a file, a test, a screenshot or a committed output.
