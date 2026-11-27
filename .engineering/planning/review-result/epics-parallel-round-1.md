---
format: aep.planning-md/3
id: review-result:epics-parallel-round-1
kind: review-result
status: active
title: Parallel-safety critic, KPI/animation/docs epics, round 1
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
revision: 2
---
needs-revision

Findings, one per line (artifact — reason — citation):

story:anim-search-record — shares `tools/sim/src/simulate.rs` (cited, both bodies) with story:kpi-sim-fields and story:kpi-flat-searches, and `tools/sim/tests/main.rs` (cited) with those two and story:kpi-repository; no depends_on edge or body sentence orders the anim-* chain against the kpi-* ones — .engineering/planning/story/anim-search-record.md:12,16 ; kpi-sim-fields.md:14,18 ; kpi-flat-searches.md:27,29 ; kpi-repository.md:28
story:anim-search-player — shares `tools/sim/src/page.html` (cited) with story:kpi-flat-searches and `tools/sim/tests/main.rs` (cited) with story:kpi-flat-searches, story:kpi-sim-fields and story:kpi-repository, and neither says so — anim-search-player.md:13,15 ; kpi-flat-searches.md:25,29
story:anim-point-replay — shares `simulate.rs`, `page.html` and `tests/main.rs` (cited) with story:kpi-flat-searches, `simulate.rs` and `tests/main.rs` with story:kpi-sim-fields, and `website/docs/simulator/run.md` (cited) with story:docs-figure-search-animation, with no edge to any of them — anim-point-replay.md:16,18,20,22 ; docs-figure-search-animation.md:14
story:kpi-repository — shares `tools/sim/tests/main.rs` (cited) with story:kpi-sim-fields, both inside the kpi epic, and no edge orders them — kpi-repository.md:28 ; kpi-sim-fields.md:18
story:kpi-ci-runs — all of story:kpi-docs-site, story:kpi-flat-searches, story:kpi-platform-gates, story:kpi-specification and story:kpi-simulator also edit `quality/ratchet.yaml` (cited) and no edge orders them against kpi-ci-runs; the only edges are a star out of story:kpi-repository, so the siblings are scheduled at once — kpi-ci-runs.md:17 ; kpi-docs-site.md:18 ; kpi-flat-searches.md:15 ; kpi-platform-gates.md:16 ; kpi-specification.md:19 ; kpi-simulator.md:16
story:kpi-docs-site — shares `quality/ratchet.yaml` (cited) with story:kpi-flat-searches, story:kpi-platform-gates, story:kpi-specification and story:kpi-simulator, and `quality/assessors/repo-facts@1.assessor.yaml` (inferred) with story:kpi-ci-runs, kpi-specification and kpi-simulator, with no edge among them — kpi-docs-site.md:16,18
story:kpi-flat-searches — shares `quality/ratchet.yaml` and `quality/assessors/weft-sim-report@1.assessor.yaml` (cited, both) with story:kpi-specification, and `ratchet.yaml` with story:kpi-simulator, with no edge (story:kpi-simulator.md:50 names kpi-flat-searches in prose only) — kpi-flat-searches.md:13,15 ; kpi-specification.md:17,19 ; kpi-simulator.md:50
story:kpi-platform-gates — shares `quality/ratchet.yaml` and `weft-sim-report@1.assessor.yaml` (cited) with story:kpi-specification, and `ratchet.yaml` with story:kpi-simulator, with no edge — kpi-platform-gates.md:14,16 ; kpi-specification.md:17,19
story:kpi-sim-fields — shares `quality/assessors/weft-sim-report@1.assessor.yaml` (cited) with story:kpi-specification (cited) and story:kpi-simulator (inferred, via the `quality/assessors` directory), with no edge — kpi-sim-fields.md:12 ; kpi-specification.md:17
story:kpi-simulator — scope names the whole directory `quality/assessors` (inferred), which collides with every assessor-editing kpi story; narrow it to the assessor files it writes — kpi-simulator.md:14
story:docs-ux-rounds — scope names the whole directory `website/docs` (inferred), which collides with every docs page that story:anim-search-record, anim-search-player, anim-point-replay, anim-docs-published-run, docs-figures, docs-status-proofs, docs-figure-search-animation and docs-components-services land on, while its only edge is to story:docs-ux-harness; narrow it or order it after those — docs-ux-rounds.md:13,10
story:anim-docs-published-run — shares `Taskfile.yml` (cited) with story:docs-claims-check and story:kpi-repository, `.github/workflows/pages.yml` (cited) with story:docs-claims-check and story:docs-ux-harness, and the `sim/experiments` directory (cited) with story:kpi-platform-gates, which gates every experiment, so a new `node-loss-climb.yaml` lands inside its surface (the file is not there yet); no edge to any of them — anim-docs-published-run.md:13,15,17 ; docs-claims-check.md:12,16 ; docs-ux-harness.md:12 ; kpi-repository.md:14 ; kpi-platform-gates.md:20
story:docs-claims-check — shares `Taskfile.yml` and `AGENTS.md` (cited) with story:kpi-repository, and `.github/workflows/pages.yml` (cited) with story:docs-ux-harness, with no edge (story:kpi-docs-site depends on both, but that orders neither of them) — docs-claims-check.md:12,14,16 ; kpi-repository.md:12,14 ; docs-ux-harness.md:12
story:docs-components-services — shares `website/sidebars.ts` (cited) with story:anim-docs-published-run, and its acceptance reads the UX checks of story:docs-ux-harness ("once that story is in") with no depends_on edge to it, so a reader sits beside its producer; add the edge or merge — docs-components-services.md:23,67 ; anim-docs-published-run.md:21

What I read: 20 stories (every kpi-*, anim-*, docs-* story, including story:docs-components-services, which appeared in the store mid-pass), plus the skeleton scopes. I ran `aep plan artifact waves` (text and json), `aep plan artifact graph` and `aep plan artifact show` equivalents by reading the story bodies, and I compared paths and the transitive depends_on closure with a scratch script.
- **Surfaces:** 3 items are fully cited, 16 mix cited and inferred, 1 is all inferred (story:docs-commands-check), and 0 are unplaced.
- **Acceptance reads:** I traced 14 to a producer in the set; 13 are recorded by a depends_on edge and 1 is not (the docs-components-services finding above).
- **Waves:** the 12 waves keep colliding pairs apart, but they do so by the tool's own placement. The finding lines above are pairs in different waves with no recorded edge, so their order is not a fact of the plan.
- **Skeleton:** against the moved skeleton scope (`product/cloud/crates/*`, `tools/check.d`, `slow.d`, `test-setup.d`, `test-teardown.d`, `tools/synth*.sh`, root `Cargo.lock`), the only shared path is `Taskfile.yml` with story:skeleton-workspace-gate. That story reads `implemented` in the skeleton worktree and its pull request #3 is merged, so the collision is spent and I raise no finding on it. Everything else is disjoint.

What I could not establish:
- **Stale skeleton copies:** this store's own skeleton-* files still cite the old `crates/…` paths (story:skeleton-workspace-gate cites `crates/weft-testkit`, and `product/` does not exist in this tree). I took the moved scopes from `the worktree of story:skeleton-generated-code`, whose store has uncommitted edits, so the cross-check rests on a working tree, not a commit. The stale copies are not an item of this set and not mine to judge.
- **Hook numbering:** `tools/check.d/20-cli-snapshots.sh` (story:docs-commands-check) and `20-conform.sh` (story:skeleton-conformance-suite) share a number, and so do `30-domain-map.sh` and `30-docs-spec-pages.sh`. They are different files, so I did not count them as collisions, but I could not confirm the order that `tools/run-hooks.sh` gives equal prefixes.
- **Out of my lane:** whether the kpi-* ratchet stories should be one story, and whether `docs-commands-check` should cite its paths instead of marking them inferred.

```findings
- file: .engineering/planning/story/anim-search-record.md
  line: 12
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares tools/sim/src/simulate.rs (cited) with story:kpi-sim-fields and story:kpi-flat-searches, and tools/sim/tests/main.rs (cited) with those two and story:kpi-repository; no depends_on edge or body sentence orders the anim-* chain against the kpi-* ones"
- file: .engineering/planning/story/anim-search-player.md
  line: 13
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares tools/sim/src/page.html (cited) with story:kpi-flat-searches and tools/sim/tests/main.rs (cited) with story:kpi-flat-searches, story:kpi-sim-fields and story:kpi-repository, and neither says so"
- file: .engineering/planning/story/anim-point-replay.md
  line: 16
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares simulate.rs, page.html and tests/main.rs (cited) with story:kpi-flat-searches, simulate.rs and tests/main.rs with story:kpi-sim-fields, and website/docs/simulator/run.md (cited) with story:docs-figure-search-animation, with no edge to any of them"
- file: .engineering/planning/story/kpi-repository.md
  line: 28
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares tools/sim/tests/main.rs (cited) with story:kpi-sim-fields and no edge orders the two"
- file: .engineering/planning/story/kpi-ci-runs.md
  line: 17
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "story:kpi-docs-site, kpi-flat-searches, kpi-platform-gates, kpi-specification and kpi-simulator all also edit quality/ratchet.yaml (cited) and no edge orders them against kpi-ci-runs; the only edges are a star out of story:kpi-repository"
- file: .engineering/planning/story/kpi-docs-site.md
  line: 18
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares quality/ratchet.yaml (cited) with story:kpi-flat-searches, kpi-platform-gates, kpi-specification and kpi-simulator, and repo-facts@1.assessor.yaml (inferred) with kpi-ci-runs, kpi-specification and kpi-simulator, with no edge among them; the assessor-file part rests on an inferred surface"
- file: .engineering/planning/story/kpi-flat-searches.md
  line: 15
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares quality/ratchet.yaml and weft-sim-report@1.assessor.yaml (cited) with story:kpi-specification, and ratchet.yaml with story:kpi-simulator, with no depends_on edge (kpi-simulator names it in prose only)"
- file: .engineering/planning/story/kpi-platform-gates.md
  line: 16
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares quality/ratchet.yaml and weft-sim-report@1.assessor.yaml (cited) with story:kpi-specification, and ratchet.yaml with story:kpi-simulator, with no depends_on edge"
- file: .engineering/planning/story/kpi-sim-fields.md
  line: 12
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "shares weft-sim-report@1.assessor.yaml (cited) with story:kpi-specification and (inferred, via the quality/assessors directory) with story:kpi-simulator, with no edge; the kpi-simulator half rests on an inferred surface"
- file: .engineering/planning/story/kpi-simulator.md
  line: 14
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scope names the whole directory quality/assessors (inferred), which collides with every assessor-editing kpi story; narrow it to the assessor files it writes (inferred surface)"
- file: .engineering/planning/story/docs-ux-rounds.md
  line: 13
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scope names the whole directory website/docs (inferred), which collides with every docs page the anim-*, docs-figures, docs-status-proofs, docs-figure-search-animation and docs-components-services stories land on, while its only edge is to story:docs-ux-harness; narrow it or order it after those (inferred surface)"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 13
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares Taskfile.yml with story:docs-claims-check and story:kpi-repository, .github/workflows/pages.yml with story:docs-claims-check and story:docs-ux-harness, and the sim/experiments directory with story:kpi-platform-gates (the new node-loss-climb.yaml does not exist yet), with no edge to any of them"
- file: .engineering/planning/story/docs-claims-check.md
  line: 12
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares Taskfile.yml and AGENTS.md (cited) with story:kpi-repository and .github/workflows/pages.yml (cited) with story:docs-ux-harness, with no edge"
- file: .engineering/planning/story/docs-components-services.md
  line: 67
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares website/sidebars.ts (cited) with story:anim-docs-published-run, and its acceptance reads the UX checks of story:docs-ux-harness with no depends_on edge to it (missing edge, or merge the two)"
```
