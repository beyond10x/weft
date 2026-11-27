---
format: aep.planning-md/3
id: epic:search-animation
kind: epic
status: active
title: 'weft-sim search animation: watch a hill climb move'
relations:
- serves: vision:weft
- informed_by: design:quality-kpis
- informed_by: design:simulation-and-kpis
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T10:52:41Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T10:52:41Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
## Goal

The owner, 2026-11-27: "could we somehow create a cool animation of a weft-sim
simulation/exploration hill climbing?" weft-sim already searches (`search.mode: fit`, a hill climb
with random restarts, and `exhaustive`), but its page shows the points as a table, and the search
output does not record how the climb moved (`design:quality-kpis` § Search over KPIs).

## Outcome

1. weft-sim's search output records per point: the round, the value index on each mutation axis,
   the parent point, the decision (start, accepted, rejected, restart), the best fitness so far and
   the walk number; restart points are evaluated and recorded like any other point.
2. The weft-sim page plays a search run step by step: a fitness landscape over two chosen mutation
   axes (the other axes fixed at the best point), the climber's path with accepted moves, rejected
   neighbours and restarts marked, and the best-so-far curve beside it. It plays live with
   `--serve` and from a finished `result.json`; play, pause, step and speed are controls.
3. A point of the animation replays by its walk number (`weft-sim` replays that walk and the page
   shows its timeline).
4. One recorded run is published on the docs site as a self-contained page (no server), linked from
   the weft-sim section.

## Not in this epic

- New KPIs or ratchet changes (`epic:kpi-ratchet`), with one exception: the experiment this epic
  adds to publish a run (outcome 4) gets its own assessment, its gate requirements and its
  reference records in `quality/ratchet.yaml` and `ratchet/references/`, so the rule every
  experiment follows holds for it too. Any other KPI or ratchet change stays in
  `epic:kpi-ratchet`.
- Searching over specification variants (a later epic of `design:simulation-and-kpis`).

## Rules

The animation reads recorded output only; it runs no simulation in the browser. No new test runs a
search longer than the 60 s per-test bound.
