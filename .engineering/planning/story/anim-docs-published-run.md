---
format: aep.planning-md/3
id: story:anim-docs-published-run
kind: story
status: draft
title: One recorded hill-climb run published on the docs site as a self-contained page, linked from the weft-sim section
relations:
- decomposes: epic:search-animation
- serves: vision:weft
- depends_on: story:anim-search-player
- depends_on: story:kpi-platform-gates
- depends_on: story:docs-claims-check
- depends_on: story:docs-ux-harness
- depends_on: story:kpi-docs-site
- depends_on: story:docs-components-services
scope:
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: sim/experiments/node-loss-climb.yaml
- confidence: inferred
  path: tools/sim-publish-check.sh
- confidence: inferred
  path: website/docs/simulator/search.md
- confidence: cited
  path: website/sidebars.ts
- confidence: inferred
  path: website/static/weft-sim/search/index.html
- confidence: inferred
  path: website/static/weft-sim/search/recording.json
- confidence: inferred
  path: website/ux/baseline.json
- confidence: inferred
  path: website/ux/published-search.spec.ts
revision: 9
---
## Epic

Decomposes `epic:search-animation`, outcome 4. Depends on `story:anim-search-player`: the
published page is the `index.html` weft-sim writes, with the player. `story:anim-point-replay`
lands before it (the chain of the weft-sim surfaces runs through `story:kpi-platform-gates`, on
which this story depends), so the recorded page shows the replay command of a point.

Order:

- `story:kpi-platform-gates`: its rule (every experiment gates on every platform gate KPI and is
  assessed on every `platform.*` KPI) holds for the experiment this story adds.
- `story:kpi-docs-site`, the last story of the chain over `quality/ratchet.yaml`,
  `quality/assessors/` and `ratchet/references/`: this story adds its experiment's assessment to
  those files after every `kpi-` story, `story:kpi-flat-searches` (the fitness spread of every
  experiment with a search) included.
- `story:docs-claims-check`: it edits `Taskfile.yml` and `.github/workflows/pages.yml` before this
  story (and `story:kpi-repository`, before it, `Taskfile.yml`).
- `story:docs-ux-harness`: it edits `pages.yml` before this story and owns
  `website/ux/baseline.json`, which must hold a value for every sitemap page; this story writes the
  values of its new page `website/docs/simulator/search.md`.
- `story:docs-components-services`: it lands first, at the owner's request that the components and
  managed-services pages come early, and this story depends on it. It adds its two pages to
  `website/sidebars.ts` and their values to `website/ux/baseline.json`; this story edits both files
  after it (the weft-sim category of the sidebar, the values of `/docs/simulator/search`).

## Context

The docs site (`website/`, Docusaurus, published at the `/weft/` base URL by
`.github/workflows/pages.yml`) has a weft-sim section (`website/sidebars.ts`: `simulator/install`,
`simulator/run`, `simulator/results`). It shows no search. weft-sim already writes a page that
needs no server: `index.html` of a run, with the snapshot inlined (`tools/sim/src/report.rs:267`).

- The run: a committed experiment with `search.mode: fit` over at least two mutation axes, whose
  recorded run holds at least one `accepted` move, one `rejected` neighbour and one `restart`, and a
  fitness spread above 0 (two of five searches are flat today, `design:quality-kpis` § Search over
  KPIs; `duplicate-delivery`, the only `fit` experiment, is one of them, and `epic:kpi-ratchet`
  changes its mutation points). This story adds its own experiment rather than editing one another
  epic changes; `sim/experiments/node-loss-climb.yaml` (`node-loss` setup and faults, `fit` over
  its axes with more values) is the working name.
- The new experiment lives in `sim/experiments/` like the six others and follows their rules: its
  `search.gates` lists every platform gate KPI of `story:kpi-platform-gates`, and
  `quality/ratchet.yaml` gets an assessment of it under the subject `platform` with a measurement
  for every `platform.*` KPI, each gate KPI a requirement in the `merge` gate, each ratchet KPI a
  requirement with a reference record under `ratchet/references/`, and its search fitness spread
  measured and required above 0 as `story:kpi-flat-searches` does for every experiment with a
  search.
- The ratchet work of this story is its own experiment's only: the assessment, the gate
  requirements and the reference records of `node-loss-climb` in `quality/ratchet.yaml` and
  `ratchet/references/`. `epic:search-animation` allows exactly that ("Not in this epic"); any
  other KPI or ratchet change stays in `epic:kpi-ratchet`. The tests that hold the rule for every
  experiment are those of `story:kpi-platform-gates` and `story:kpi-flat-searches` in
  `tools/sim/tests/ratchet.rs`; they enumerate `sim/experiments/`, so they cover the new file
  without a change.
- `website/ux/baseline.json` gets the values of the new page `/docs/simulator/search` for every
  key the harness of `story:docs-ux-harness` measures, so its ratchet finds no missing value.
- `task sim-publish` (in `Taskfile.yml`): builds weft-sim, runs that experiment with a fixed seed
  and relative paths, copies its `index.html` to `website/static/weft-sim/search/index.html`, and
  fails when `tools/sim-publish-check.sh <file>` (POSIX sh) finds an absolute file path or a URL
  the page would load in that file. It writes
  `website/static/weft-sim/search/recording.json` with `experiment`, `seed` and `commit`, the
  last being `git rev-parse HEAD` at record time: the commit the run was recorded from, before the
  commit that adds the recording (a committed file cannot hold its own hash). It
  measures a run, so it runs without the `slot` prefix (cargo inside it is shimmed). It is not
  part of the test step or `task check`.
- The published file is a recorded output: it is committed, and the docs page shows what recorded
  it and names the command that records it again. The site parses a `.md` page as CommonMark
  (`markdown.format: 'detect'`, `website/docusaurus.config.ts:29`), so the page imports nothing:
  `task sim-publish` writes the three values into the block between `<!-- recording -->` and
  `<!-- /recording -->` of `website/docs/simulator/search.md`, as the lines `Experiment:`,
  `Seed:` and `Commit:`, each value in backticks. It is not compared with the tree in any gate; a
  stale recording is still true about the commit it names.
- Browser checks: `website/ux/published-search.spec.ts` in the harness of `story:docs-ux-harness`
  runs on the built site its `webServer` serves. It opens `/weft/weft-sim/search/index.html`,
  records every request the page makes, drives the player by role and name, and reads the player
  state (`data-frame`, `data-playing`, `data-speed`) and the drawn elements (`data-path-step`
  with `data-decision`, `data-cell` with `data-fitness`) that `story:anim-search-player` defines.
- `website/docs/simulator/search.md`: what the animation shows (landscape, path, decisions,
  best-so-far, controls), how to watch a run live (`--serve`) and from a `result.json`, and the
  link that opens the published page. Listed in the weft-sim category of `website/sidebars.ts`.
- `.github/workflows/pages.yml`: after the build, a step fails when
  `website/build/weft-sim/search/index.html` is missing or holds no search row with a `decision`.
- `epic:docs-fidelity` outcome 5 embeds this animation in other pages once it exists; that
  embedding is its own, not this story.

## Domain relations

None of `ess/`.

- Published page to recorded run, one-to-one: the page holds one run, and the docs page names its
  experiment, seed and commit. Inferable (inferred from `tools/sim/src/report.rs:267`, `page`
  inlines one snapshot into one page).

## Scope

- `sim/experiments/node-loss-climb.yaml` (new; working name)
- `Taskfile.yml` (task `sim-publish`)
- `tools/sim-publish-check.sh` (new, the check of the written page)
- `website/static/weft-sim/search/index.html` (the recorded page)
- `website/static/weft-sim/search/recording.json` (new: experiment, seed, commit of the recording)
- `website/docs/simulator/search.md` (new page)
- `website/sidebars.ts` (weft-sim category)
- `.github/workflows/pages.yml` (check of the built page)
- `quality/ratchet.yaml`, `ratchet/references/` (the assessment of the new experiment only)
- `website/ux/baseline.json` (the values of the new docs page)
- `website/ux/published-search.spec.ts` (new, the browser checks of the published page)

## Acceptance

Local browser checks run as `slot npm --prefix website run build`, then
`slot npm --prefix website run ux -- --workers=2`; in CI the docs job of `pages.yml` runs the same
spec.

1. On the built docs site, the weft-sim section lists a page about the search animation whose
   link opens `weft-sim/search/index.html` under the site base URL (`published-search.spec.ts`,
   test "docs page links the published run": it follows the link from `/weft/docs/simulator/search`
   and lands on a page whose player root carries `data-frame`).
2. On the published page, pressing Play and advancing the clock moves `data-frame` forward;
   Pause sets `data-playing` to `false` and holds `data-frame`; Step forward raises `data-frame`
   by one row; setting the speed to 4 sets `data-speed` to `4` (`published-search.spec.ts`, test
   "controls").
3. Opening the published page and playing it to the last frame makes no request to a URL other
   than the page itself: the requests the spec records hold only the page's own URL, the
   browser's own `favicon.ico` request aside (`published-search.spec.ts`, test "loads nothing
   else").
4. At the last frame of the published page, at least one element
   `[data-path-step][data-decision=accepted]`, one `[data-decision=rejected]` and one
   `[data-decision=restart]` exists (`published-search.spec.ts`, test "decision kinds").
5. At the last frame of the published page, the `[data-cell]` elements carry at least two
   distinct `data-fitness` values (`published-search.spec.ts`, test "fitness differs").
6. The built docs page `/docs/simulator/search` shows the lines `Experiment:`, `Seed:` and
   `Commit:` whose values equal `experiment`, `seed` and `commit` of
   `website/static/weft-sim/search/recording.json` (`published-search.spec.ts`, test "recording
   named"), and names the `task sim-publish` command (`grep -n 'task sim-publish'
   website/docs/simulator/search.md` prints a line).
7. `tools/sim-publish-check.sh`, the check `task sim-publish` runs on the page it writes, exits 1
   on a copy of the recorded page with an absolute local path (a fixture string built in the test from `$HOME`) inserted, exits 1 on a copy with
   `<script src="https://example.org/a.js"></script>` inserted, and exits 0 on the committed
   page; `grep -n 'sim-publish-check.sh' Taskfile.yml` prints the line of the task that calls it.
8. In `.github/workflows/pages.yml` a step after the build fails when
   `website/build/weft-sim/search/index.html` is missing or holds no search row with a
   `decision` (`grep -n 'weft-sim/search/index.html' .github/workflows/pages.yml` prints the
   step).
9. The docs job of `pages.yml` on this story's pull request is green with that step run (the log
    of the step shows the file and its count of rows with a decision).
10. `search.gates` of `sim/experiments/node-loss-climb.yaml` lists every platform gate KPI of
    `story:kpi-platform-gates`: its test `ratchet::every_search_gates_every_platform_gate_kpi`
    passes with the new file in `sim/experiments/`, and `the_committed_experiments_parse` passes
    (`task sim-test`).
11. `quality/ratchet.yaml` has an assessment of `node-loss-climb` under the subject `platform` with
    a measurement for every `platform.*` KPI: `ratchet::every_experiment_measures_every_platform_kpi`
    of `story:kpi-platform-gates` passes with the new file (`task sim-test`).
12. Each gate KPI of `node-loss-climb` is a requirement in the `merge` gate:
    `ratchet::every_experiment_gate_kpi_is_a_merge_requirement` passes (`task sim-test`).
13. The search fitness spread of `node-loss-climb` is measured and required above 0 in the `merge`
    gate: `ratchet::every_search_spread_is_measured` and
    `ratchet::every_unmarked_search_requires_a_spread` of `story:kpi-flat-searches` pass
    (`task sim-test`).
14. Each ratchet-role requirement of `node-loss-climb` has its reference record:
    `ratchet::the_ratchet_file_is_consistent` of `story:kpi-repository` passes (`task sim-test`).
15. No requirement, assessment or reference record of another experiment changes:
    `git diff --stat main -- ratchet/references/` lists only files named after `node-loss-climb`,
    and `git diff main -- quality/ratchet.yaml` touches only the `node-loss-climb` assessment and
    its `merge` entries.
16. `website/ux/baseline.json` holds a value for every key of `/docs/simulator/search`, and
    `slot npm --prefix website run ux -- --workers=2` after a build exits 0.

## Rules for this work

- The published page reads recorded output only; it runs no simulation and needs no server.
- `cargo` is shimmed through `slot`; `slot npm run build` in `website/` and `slot task check` keep
  their prefix; `task sim-publish` measures and runs without it. A pool job is killed after 10
  minutes unless it names its time (`slot --timeout`, at most 30m).
- No test runs over 60 s; the test step ends within 120 s. The publish run is not in the test
  step. Playwright runs with `--workers=2` through `slot`.
- No Python in anything committed: Rust or POSIX sh.
- The site is public: the page and the recording name no person, host, session or private
  repository (`AGENTS.md` § Layout, `website/`).
