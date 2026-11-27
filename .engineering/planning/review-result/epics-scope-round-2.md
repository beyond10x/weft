---
format: aep.planning-md/3
id: review-result:epics-scope-round-2
kind: review-result
status: active
title: Scope critic, KPI/animation/docs epics, round 2
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

Findings (2):

- story:docs-components-services — The epic promises managed-services pages covering "offering kinds, plans, service instances, bindings, products". Acceptance 2 lists kinds, binding modes, `ServiceInstance` states and one offering-versus-product difference. It names no plans. `OfferingPlan` is in the Context only. Products get that one difference and no coverage of `Product`, `ProductVersion` or `Installation`. This is outcome 6 silently narrowed. Add plans, and a product listing or an explicit statement that products are covered only as that difference. — `.engineering/planning/epic/docs-fidelity.md:36-37`; `.engineering/planning/story/docs-components-services.md:61-66` (Context names `OfferingPlan` at about line 46).

- story:anim-docs-published-run — The story edits `quality/ratchet.yaml` and `ratchet/references/` (an assessment, requirements and reference records for `node-loss-climb`, plus the spread requirement). That is acceptance 8 and scope lines 76-78 and 116. The epic excludes it: "New KPIs or ratchet changes (`epic:kpi-ratchet`)". The kpi-ratchet side does not claim this work either: `story:kpi-platform-gates` acceptance 2 says "each of the six experiments". It also says the `anim-` story adds the seventh, so the ratchet work has no owner in the epic that may do it. Move the ratchet half to a kpi-ratchet story, or put it in `epic:search-animation`'s outcome and drop the exclusion. — `.engineering/planning/epic/search-animation.md:39`; `.engineering/planning/story/anim-docs-published-run.md:76-78`; `.engineering/planning/story/kpi-platform-gates.md:109`.

What I read: 3 epics, `design:quality-kpis` and all 20 stories in full, plus `review-result:epics-scope-round-1`. Commands: `aep plan artifact show` for each, `aep plan artifact graph`, and `grep -n` on the store files.

Promise counts:
- epic:kpi-ratchet: 4 outcomes (1 to 4), all 4 traced. The catalogue's 65 `yes` or `probe` KPIs each have exactly one claiming story:
  - spec 21: `kpi-specification`.
  - platform: all 21, `kpi-platform-gates`.
  - sim 6 plus the probe `ess-revisions`: `kpi-simulator`.
  - `sim.search-fitness-spread`: `kpi-flat-searches`.
  - repo 9: `kpi-repository`.
  - `repo.agents-md-dead-paths`: `kpi-docs-site`.
  - ci 6 and docs 2 (`docs.build-failures`, `docs.build-seconds`): `kpi-ci-runs`.
  - docs 3 on the tree (`docs.status-age-days`, `docs.dead-repo-links`, `docs.planned-without-banner`): `kpi-docs-site`.
- epic:search-animation: 4 outcomes, 4 traced. `anim-search-record` takes 1, `anim-search-player` 2, `anim-point-replay` 3 and `anim-docs-published-run` 4.
- epic:docs-fidelity: 6 outcomes, with 5 KPIs inside outcome 1, so 10 promises. 9 are fully traced and 1 is narrowed (outcome 6, plans).
  - `docs-claims-check` takes 3 KPIs and outcome 3.
  - `docs-status-proofs` takes outcome 2 and `docs.status-claims-false`.
  - `docs-commands-check` takes `docs.commands-refused`.
  - `docs-ux-harness` and `docs-ux-rounds` take outcome 4.
  - `docs-figures` and `docs-figure-search-animation` take outcome 5.
  - `docs-components-services` takes outcome 6.
- Round-1 finding: the `docs-figures` hook `docs.figure-names-unknown` is gone. Lines 52-53 now say no check is added.

What I could not establish (none of this set the verdict):
- Out of lane (design and parallel-safety): `story:anim-docs-published-run` lists `depends_on story:docs-components-services` (graph and relations line 15), but its Order text says that story "edits it after this story and depends on it". `story:docs-components-services` says in its Order (line 92) that it depends on `anim-docs-published-run`, yet its relations list only `docs-claims-check` and `docs-ux-harness`. The direction is contradictory and may be a cycle.
- I judged `sim.search-fitness-spread` as a merge requirement in `kpi-flat-searches` and `chain-missing` in `kpi-sim-fields` to be traceable (the drafter names the first in Ambiguities). I did not count them as reach.

```findings
- file: .engineering/planning/epic/docs-fidelity.md
  line: 36
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "outcome 6 promises managed-services pages covering 'offering kinds, plans, service instances, bindings, products'; story:docs-components-services acceptance 2 (lines 61-66) names kinds, binding modes, ServiceInstance states and an offering-versus-product difference but no plans (OfferingPlan appears only in Context) and no product listing, so the promise is narrowed with nothing recording the decision"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 76
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the story adds an assessment, merge requirements and reference records in quality/ratchet.yaml and ratchet/references for node-loss-climb (acceptance 8), which epic:search-animation excludes ('New KPIs or ratchet changes (epic:kpi-ratchet)', search-animation.md:39), and story:kpi-platform-gates acceptance 2 covers only 'the six experiments', so the work has no owner in the epic that may do it"
```
