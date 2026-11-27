---
format: aep.planning-md/3
id: story:docs-ux-rounds
kind: story
status: draft
title: 'Measured fidelity rounds on the docs site, R-01 first: at most 5 changes, each with its numbers before and after'
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-ux-harness
- depends_on: story:anim-docs-published-run
- depends_on: story:docs-figures
- depends_on: story:docs-status-proofs
- depends_on: story:docs-figure-search-animation
- depends_on: story:docs-components-services
- depends_on: story:anim-point-replay
- depends_on: story:docs-commands-check
scope:
- confidence: inferred
  path: website/docs
- confidence: cited
  path: website/package.json
- confidence: inferred
  path: website/src/css/custom.css
- confidence: inferred
  path: website/src/theme
- confidence: inferred
  path: website/ux/baseline.json
- confidence: inferred
  path: website/ux/findings
- confidence: inferred
  path: website/ux/round.ts
- confidence: inferred
  path: website/ux/rounds
revision: 6
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 4, its round loop: "each round names what it changed and
the numbers before and after". Depends on `story:docs-ux-harness` (adapter, gates, baseline).

Order: the scope keeps the whole of `website/docs/`, since which pages a round changes is known
only at its Triage (`triage.md` names them). Instead the story depends on every story that lands a
docs page or changes one: `story:anim-docs-published-run` (`simulator/search.md`),
`story:docs-figures` (`index.md`, `concepts/nodes-and-clusters.md`, `specification/`),
`story:docs-status-proofs` (`status.md`), `story:docs-figure-search-animation` and
`story:anim-point-replay` (`simulator/run.md`), `story:docs-components-services`
(`concepts/components.md`, `concepts/managed-services.md`), and `story:docs-commands-check`,
which may fix a refused `$ ` line in any of the six pages that carry one. `story:anim-search-record` and
`story:anim-search-player` (`simulator/results.md`) come before `story:anim-point-replay` in the
chain of the weft-sim surfaces, so R-01 starts on a site where every planned page is in.

## Context

The `ui-iteration-loop` skill (uxui plugin) § 3: a round is Trial, Measure, Triage, Design
decision, Build, Gate. The skill puts each round as one task under one standing story, so a round
needs no new approval; this is that story. Its acceptance is the first round, R-01; later rounds are
tasks under it with the same acceptance for their own number. At most two rounds a week (skill § 2).

R-01 on this site:

- Trial: the screen-only user plays the two anchor scenarios of `website/ux/adapter.ts` and one
  rotating scenario. No paid level runs (L4, L5); a survey is taken only when the owner runs a
  session, and a bot answers none.
- Measure: a deterministic round command, `npm --prefix website run ux:round -- R-01`, writes
  `website/ux/rounds/R-01/report.md`, `round.json` and `shots/` (the worst moments and a contact
  sheet of every gating viewport in both themes), and a row of `website/ux/rounds/trend.md`. Then
  two fresh review agents write `review-heuristics.md` and `review-report.md` in the round
  directory (skill § 8; the `docs-site-review` skill where installed).
- Triage by the skill formula (priority = severity weight x frequency x confidence / cost) into
  `triage.md`: at most 5 changes, every S1 finding included. Findings are
  `website/ux/findings/UXF-nnn.md` records with the header fields of skill § 5.
- Design decision: at most one layout or interaction change per round goes through the A/B order of
  skill § 3.4 in `decision.md`; its answer is the owner's, put as a question with its default,
  and it blocks no other change of the set.
- Build failing-first: each S1 and S2 finding gets its regression asset in the harness, run red,
  then the change makes it green.
- Gate: the harness of `story:docs-ux-harness` passes; every improved value becomes the new
  baseline in the same merge.

## Domain relations

None. This story touches no entity of `ess/`.

## Scope

- `website/ux/rounds/`, `website/ux/findings/`, `website/ux/baseline.json`
- `website/ux/round.ts` (the round command), `website/package.json` (script `ux:round`)
- `website/src/css/custom.css`, `website/src/theme/`, `website/docs/` (the changes a round makes;
  each round names its files in `triage.md`)

## Acceptance

1. `website/ux/rounds/R-01/report.md` and `round.json` give every metric of
   `website/ux/baseline.json` per scenario and page with its value before the round's changes and
   after them, and `website/ux/rounds/trend.md` has a row for R-01.
2. `website/ux/rounds/R-01/triage.md` names at most 5 changes, each with its finding id, target
   metric, priority by the skill formula and the files it changed.
3. Every S1 and S2 finding of R-01 closed in the round has a regression asset in the harness that
   fails on the tree before its change and passes after it.
4. No value of `website/ux/baseline.json` is worse after R-01 than before it, and every value R-01
   improved is the new baseline in the same merge.

## Rules for this work

- Playwright runs with `--workers=2` through `slot`; no test over 60 s; the harness step under
  120 s. The round command measures and runs without the `slot` prefix; its builds and Playwright
  runs inside it carry the prefix.
- No Python in anything committed; TypeScript in `website/`.
- No claim on the site that the tree does not prove.
- The repository is public: reports, review files, findings and screenshots name no person, host,
  session or private repository.
