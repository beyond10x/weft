---
format: aep.planning-md/3
id: review-result:epics-design-round-1
kind: review-result
status: active
title: Design critic, KPI/animation/docs epics, round 1
relations:
- reviews: story:kpi-repository
- reviews: story:kpi-ci-runs
- reviews: story:kpi-specification
- reviews: story:kpi-sim-fields
- reviews: story:kpi-platform-gates
- reviews: story:kpi-flat-searches
- reviews: story:kpi-simulator
- reviews: story:kpi-docs-site
- reviews: story:anim-search-record
- reviews: story:anim-search-player
- reviews: story:anim-point-replay
- reviews: story:anim-docs-published-run
- reviews: story:docs-claims-check
- reviews: story:docs-status-proofs
- reviews: story:docs-commands-check
- reviews: story:docs-ux-harness
- reviews: story:docs-ux-rounds
- reviews: story:docs-figures
- reviews: story:docs-figure-search-animation
- reviews: story:docs-components-services
revision: 1
---
needs-revision

- story:anim-docs-published-run — it adds `sim/experiments/node-loss-climb.yaml` while `story:kpi-platform-gates` and `story:kpi-flat-searches` quantify over every experiment in that directory ("the six", "every experiment with a search"), and no edge or scope line says who gates, assesses or ratchets the seventh. The body should either keep the experiment outside `sim/experiments/` or state that it carries every gate KPI and its `quality/ratchet.yaml` assessment, with a `depends_on` edge to `story:kpi-platform-gates` — `.engineering/planning/story/anim-docs-published-run.md:45,74`, `.engineering/planning/story/kpi-platform-gates.md:63,91,94`, `.engineering/planning/story/kpi-flat-searches.md:87-88`
- story:kpi-ci-runs — it and `story:kpi-docs-site` each add the `docs-site` subject only "if the other has not added it yet", so the surface has two conditional owners and no edge. Put the subject in `story:kpi-repository`'s frame, or add a `depends_on` edge from `story:kpi-ci-runs` to `story:kpi-docs-site` and make `story:kpi-docs-site` the sole owner — `.engineering/planning/story/kpi-ci-runs.md:59-60`, `.engineering/planning/story/kpi-docs-site.md:62`
- story:kpi-flat-searches — its acceptance reads the fitness spread of the `fit` search over duplicate-delivery's feasible points. `story:anim-search-record` changes that search: the restart is now evaluated, recorded and counted against `budget`. No `depends_on` edge orders the two, so the spread and its reference record can be taken on semantics that change afterwards. Add `depends_on story:anim-search-record`, or state in the body that the spread is independent of it — `.engineering/planning/story/kpi-flat-searches.md:81`, `.engineering/planning/story/anim-search-record.md:53-56`
- story:anim-docs-published-run — it adds the new docs page `website/docs/simulator/search.md`, which enters the sitemap. `story:docs-ux-harness` requires `baseline.json` to hold a value for every page and fails the ratchet on a missing value. Which story writes the baseline entries for the new page is unrecorded, and `website/ux/baseline.json` is in no scope of this story. Add `depends_on story:docs-ux-harness` and put `baseline.json` in scope, or record that the harness lands after this story — `.engineering/planning/story/docs-ux-harness.md:108,114`, `.engineering/planning/story/anim-docs-published-run.md:56,77`

**What you read:** 41 artifacts. I ran `aep plan artifact show` on the 3 epics and the 18 kpi, anim and docs stories, plus `relations`, `graph` and `validate`.
- **Edges walked:** all declared `depends_on` edges in the store, including the cross-epic ones. This covered `story:kpi-docs-site` -> `story:docs-claims-check` and `story:docs-figure-search-animation` -> `story:anim-docs-published-run`. There are no cycles.
- **Acceptance reads traced:** about 20 to a producer in the set, 17 of them recorded by an edge and the 3 unrecorded ones are the findings above.
- **Chains:** the longest chain is `story:docs-figure-search-animation` -> `story:anim-docs-published-run` -> `story:anim-search-player` -> `story:anim-search-record`. It has a reason on every edge, and the set is not a queue.
- **Cleared:**
  - `story:docs-claims-check` creates the hook directory, and `story:docs-commands-check`, `story:docs-status-proofs` and `story:docs-figures` extend it without editing its scripts.
  - `website/sidebars.ts` has one owner, `story:anim-docs-published-run`.
  - `pages.yml` and `Taskfile.yml` are touched by different steps and tasks in each story.
  - The `quality/ratchet.yaml` and `tools/sim/src/simulate.rs` sharing between stories is ordered or is a parallel-safety matter.

**What I could not establish:**
- Whether sibling `kpi-*` stories that edit `quality/ratchet.yaml` and `weft-sim-report@1.assessor.yaml` without an ordering edge can run at the same time. That is out of my lane (parallel-safety).
- `docs.status-claims-false` and `docs.commands-refused` are produced by `story:docs-status-proofs` and `story:docs-commands-check`, but no story holds them in `quality/ratchet.yaml` (`.engineering/planning/story/kpi-docs-site.md:65-67`). That is a scope question, so it is out of my lane.
- `story:docs-figures` and `story:docs-status-proofs` change page content and could also move `baseline.json` values. I raised only the sharpest case, `story:anim-docs-published-run`.
- `.engineering/planning/story/docs-components-services.md` is in the store directory but not in `aep plan artifact list`. I did not read it.

```findings
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 74
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds sim/experiments/node-loss-climb.yaml while story:kpi-platform-gates and story:kpi-flat-searches quantify over every experiment in that directory, and no edge or scope line says who gates, assesses or ratchets the seventh; keep it outside sim/experiments/ or state its gates and assessment, with depends_on story:kpi-platform-gates"
- file: .engineering/planning/story/kpi-ci-runs.md
  line: 59
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it and story:kpi-docs-site each add the docs-site subject only if the other has not, so two items conditionally own one surface with no edge; give the subject to story:kpi-repository's frame, or add depends_on story:kpi-docs-site and make that story the sole owner"
- file: .engineering/planning/story/kpi-flat-searches.md
  line: 81
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reads the spread of the fit search over duplicate-delivery whose restart story:anim-search-record changes (evaluated, counted against budget), and no depends_on edge orders them; add depends_on story:anim-search-record or state the spread is independent of it"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 56
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds the sitemap page website/docs/simulator/search.md, which story:docs-ux-harness requires baseline.json to hold a value for (missing value fails the ratchet), with no edge and no baseline.json in scope; add depends_on story:docs-ux-harness and baseline.json to scope, or record that the harness lands after"
```
