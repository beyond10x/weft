---
format: aep.planning-md/3
id: story:docs-figure-search-animation
kind: story
status: draft
title: weft-sim pages show the published search animation as a figure where the search is explained
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-ux-harness
- depends_on: story:anim-docs-published-run
- depends_on: story:anim-point-replay
scope:
- confidence: cited
  path: website/docs/simulator/run.md
- confidence: inferred
  path: website/static/img
- confidence: inferred
  path: website/ux/adapter.ts
revision: 7
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 5 for its third figure: "the search animation of
`epic:search-animation` once it exists". Depends on `story:anim-docs-published-run` (the published
run) and on `story:docs-ux-harness` (the animation page joins its page list). Depends on
`story:anim-point-replay`, which edits `website/docs/simulator/run.md` (section Replay) before it.
`story:docs-commands-check` depends on this story: `run.md` carries `$ ` lines, and that story
checks them after this story has edited the page.

## Context

`story:anim-docs-published-run` (`epic:search-animation`, outcome 4) publishes one recorded `fit`
run as `website/static/weft-sim/search/index.html`, a page that needs no server, with its own docs
page `website/docs/simulator/search.md` in the weft-sim category; that page names the experiment,
the seed and the commit of the recording. That story leaves the embedding in other pages to this
epic. This story makes the run a figure where the docs explain the search, so a reader sees the
climb before reading the table of points:

- `/docs/simulator/run` § Simulate, where `mutations` and `search` are explained
  (`website/docs/simulator/run.md`: the line "exhaustive search over 2 mutation points" and the
  table row "`mutations`, `search`"): a figure that shows a still of the recorded run, with a
  caption that names the experiment, the seed and the commit the recording names, and links to the
  published page and to `/docs/simulator/search`. The still is taken from the published page, not
  drawn.
- The published page is added to the page list of `website/ux/adapter.ts`, so the hard gates of
  `story:docs-ux-harness` (no horizontal scroll, axe 0 serious or critical, both themes, both
  gating viewports) hold on it and on the page with the figure.

## Domain relations

None of `ess/`. The figure shows the one run the published page holds (the one-to-one relation of
`story:anim-docs-published-run`, inferred there from `tools/sim/src/report.rs:267`).

## Scope

- `website/docs/simulator/run.md`
- `website/static/img/` (the still of the recorded run)
- `website/ux/adapter.ts` (the published page in the page list)

## Acceptance

1. The built `/docs/simulator/run` page shows, in its Simulate section, a still of the recorded
   search run whose caption names the experiment, the seed and the commit of the recording, and
   which links to the published animation page.
2. The layout, accessibility and screen-only checks of `story:docs-ux-harness` pass on the published
   animation page and on `/docs/simulator/run` at both gating viewports in both themes.

## Rules for this work

- Playwright runs with `--workers=2` through `slot`; no test over 60 s; the step under 120 s.
- The figure reads recorded output only; nothing runs a simulation in the browser or in the build.
- No Python in anything committed.
- No claim on the site that the tree does not prove; no person, host, session or private
  repository name.
