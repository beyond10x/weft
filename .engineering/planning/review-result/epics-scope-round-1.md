---
format: aep.planning-md/3
id: review-result:epics-scope-round-1
kind: review-result
status: active
title: Scope critic, KPI/animation/docs epics, round 1
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

- story:docs-figures — the hook `60-figure-names.sh` and its new KPI `docs.figure-names-unknown` (AC4) are not in the epic's five-KPI list and not in the catalogue, so drop the hook and its fixture, or add the KPI to the epic and the design. — `.engineering/planning/story/docs-figures.md:65-68` (also :92-94); epic list at `.engineering/planning/epic/docs-fidelity.md:23-25`; `grep figure-names` finds the string only in this story.

What I read: 8 artifacts, 20 in all. I read design:quality-kpis and the three epics before any story, then all 4 anim, 7 docs and 8 kpi stories in full, and `aep plan artifact graph`, `kinds` and `relations`.

Promise counts:
- epic:kpi-ratchet — 4 outcomes, 4 traced.
- epic:search-animation — 4 outcomes, 4 traced.
- epic:docs-fidelity — 5 outcomes with 5 KPIs inside outcome 1, 9 promises, 9 traced.
- Outcomes are claimed by:
  - kpi-ratchet:
    - Outcome 1 is claimed by kpi-repository, kpi-specification, kpi-simulator, kpi-ci-runs, kpi-docs-site and kpi-platform-gates.
    - Outcome 2 is split across kpi-sim-fields and kpi-platform-gates.
    - Outcome 3 is kpi-flat-searches.
    - Outcome 4 is the hooks of the same stories.
  - search-animation:
    - Outcome 1 is anim-search-record, outcome 2 is anim-search-player, outcome 3 is anim-point-replay, outcome 4 is anim-docs-published-run.
  - docs-fidelity:
    - Outcome 1 is claimed by docs-claims-check, docs-status-proofs and docs-commands-check.
    - Outcome 2 is docs-status-proofs, outcome 3 is docs-claims-check, outcome 4 is docs-ux-harness and docs-ux-rounds.
    - Outcome 5 is docs-figures and docs-figure-search-animation.
- Every `yes` or `probe` KPI of the catalogue (spec 21, platform 16, sim 7, docs 5, repo 10, ci 6) has exactly one claiming story.

What I could not establish (nothing here set the verdict):
- Several stories edit `tools/sim/src/page.html`, `simulate.rs` and `sim/experiments` (anim-search-player, anim-point-replay, anim-search-record, kpi-flat-searches). That is for plan-critic-parallel-safety.
- anim-docs-published-run adds a seventh experiment, `node-loss-climb.yaml`, while kpi-platform-gates AC2 says "each of the six experiments". That is for plan-critic-design.
- kpi-flat-searches (lines 51-57) changes `page.html` to show the exhaustive-only mark and turns the catalogue's report KPI `sim.search-fitness-spread` into a merge requirement. The drafter names this under Ambiguities and it traces to outcome 3, so I did not count it as reach.
- docs-commands-check adds an `ess --version` equality check to the gate. It looks tied to the help snapshots, so I judged it traceable.

```findings
- file: .engineering/planning/story/docs-figures.md
  line: 65
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the hook 60-figure-names.sh and its new KPI docs.figure-names-unknown (acceptance 4) are not among the five docs KPIs of epic:docs-fidelity outcome 1 and not in design:quality-kpis, so nothing the epic asked for covers them; drop the hook and its fixture, or add the KPI to the epic and the design"
```
