---
format: aep.planning-md/3
id: review-result:epics-parallel-round-2
kind: review-result
status: active
title: Parallel-safety critic, KPI/animation/docs epics, round 2
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

Findings:
- story:docs-commands-check — its snapshot of `weft-sim --help` (committed `website/data/cli/weft-sim.txt`, held equal to the binary by `tools/check.d/20-cli-snapshots.sh`) reads the one `USAGE` text in `tools/sim/src/main.rs`. story:anim-point-replay edits that usage text (cited, its Scope list). No `depends_on` edge orders the two, so the snapshot file and the new flags can land in either order and the gate fails when they do not agree. Add an edge from docs-commands-check to anim-point-replay, or merge them — .engineering/planning/story/docs-commands-check.md:41, .engineering/planning/story/anim-point-replay.md:78
- story:docs-commands-check — "a refused line found on the tree is fixed in its page", but the Scope names no page under `website/docs` (inferred). `$ ` lines sit in `website/docs/simulator/run.md` and `specification/layers.md`, which anim-point-replay, docs-figure-search-animation and docs-figures edit (cited). docs-ux-rounds scopes the whole of `website/docs`. docs-commands-check has an edge to none of them. Name the pages and add an ordering edge that records the shared page, or split the surface — .engineering/planning/story/docs-commands-check.md:55
- story:docs-components-services — its acceptance 3 reads `docs.status-claims-false` staying 0. That count comes from the `40-status-claims.sh` hook that story:docs-status-proofs creates, on different files. Its only edges are to docs-claims-check and docs-ux-harness, so one wave holds both and the check does not exist yet. Add the edge to docs-status-proofs, or merge the two — .engineering/planning/story/docs-components-services.md:68, .engineering/planning/story/docs-status-proofs.md (Scope: `tools/docs-check.d/40-status-claims.sh`)

What I read:
- 20 stories (every kpi-*, anim-*, docs-*), plus the four round-1 review results.
- Commands: `aep plan artifact waves`, `graph`, `relations`, `show` on each story.
- I took the tool's 103 colliding pairs and checked each against the transitive `depends_on` closure with a scratch script.
- I grepped each body for paths missing from its Scope.

Round 1 fixed:
- The kpi/anim chain is now one linear `depends_on` chain (kpi-repository to kpi-docs-site).
- Every other pair that collides within the set is ordered transitively: `ratchet.yaml`, `simulate.rs`, `page.html`, `tests/main.rs`, `Taskfile.yml`, `pages.yml`, `sidebars.ts`, `baseline.json`, `search.md`.

What I could not establish:
- **Surfaces:** I placed 20 of 20 items from their Scope sections, 0 unplaced. The store marks many individual paths inferred because the files do not exist yet. docs-commands-check shows all four of its paths as inferred, but its body names them.
- **Acceptance reads:** I traced about 12 to a producer in the set. 10 are recorded by an edge and 2 are not (the findings above).
- **Skeleton:** the tool still lists `Taskfile.yml` shared by story:skeleton-workspace-gate (active in this store) with anim-docs-published-run, docs-claims-check and kpi-repository, with no edge. That item is outside my set. Round 1 reported it as spent, since its pull request is already merged. Not a finding.
- **Out of my lane:** the 20-story set may be too broad for the kpi-docs-site reading of "each hook"; I did not judge it.

```findings
- file: .engineering/planning/story/docs-commands-check.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its committed weft-sim help snapshot mirrors the USAGE text in tools/sim/src/main.rs, which story:anim-point-replay edits (cited), and no depends_on edge orders the two, so the 20-cli-snapshots gate and the new flags can land in either order"
- file: .engineering/planning/story/docs-commands-check.md
  line: 55
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it fixes refused command lines in unnamed website/docs pages (inferred: run.md and layers.md carry the $ lines), which story:anim-point-replay, docs-figure-search-animation, docs-figures and docs-ux-rounds also edit, and it has no edge to any of them"
- file: .engineering/planning/story/docs-components-services.md
  line: 68
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance 3 reads docs.status-claims-false, produced by the hook story:docs-status-proofs creates, and no depends_on edge runs from it to docs-status-proofs, so one wave schedules both"
```
