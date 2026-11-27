---
format: aep.planning-md/3
id: review-result:epics-design-round-2
kind: review-result
status: active
title: Design critic, KPI/animation/docs epics, round 2
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

story:anim-docs-published-run — the frontmatter edge `depends_on: story:docs-components-services` runs the opposite way to what its own body says, and the opposite way to the body of the other story. Its body says components "edits it after this story and depends on it". Either flip the edge (components `depends_on` anim, which pushes components behind the whole 11-link chain) or keep the edge and rewrite both bodies so components lands before anim. The second option is the cheaper: components needs nothing from anim except the order on `website/sidebars.ts` and `website/ux/baseline.json`, and that order can run either way — `.engineering/planning/story/anim-docs-published-run.md:15,57`, `.engineering/planning/story/docs-components-services.md:91-93`

story:docs-components-services — its Order section says it depends on `story:anim-docs-published-run`, which "edits `website/sidebars.ts` and `website/ux/baseline.json` before it". Its frontmatter has no such edge, and the graph holds the reverse edge. Following the body (adding that edge while the reverse one stays) makes a cycle `components → anim → components`; the graph is acyclic today only because the body is not followed — `.engineering/planning/story/docs-components-services.md:9-11,91-93`, `aep plan artifact graph`

**What you read:** 46 artifacts in the store, which holds 65 `depends_on` edges in all. I ran `aep plan artifact show` on the stories I judged (the 20 `kpi-`, `anim-` and `docs-` stories, plus the three epics), `relations`, `graph` and `validate`, and fed the 65 `depends_on` edges to `tsort`.
- **Edges and cycles:**
  - I walked all 65 `depends_on` edges across the whole store, the skeleton stories included. `tsort` found no cycle in the declared edges.
  - `aep plan artifact validate` reports valid.
  - A cycle appears only if the edge named in the findings is added without removing the reverse one.
- **Shared surfaces:** I compared the scope paths of the 20 stories. 15 shared paths:
  - Ordered by an edge, directly or through the chain: `repo-facts` and `weft-sim-report` assessors, `quality/ratchet.yaml`, `ratchet/references`, `pages.yml`, `Taskfile.yml`, `AGENTS.md`, `page.html`, `simulate.rs`, `tools/sim/tests/main.rs`, `tests/ratchet.rs`, `results.md`, `run.md`, `package.json`, `theme`, `custom.css`.
  - `sidebars.ts` and `baseline.json` for the components/anim pair are ordered by an edge, but it runs the wrong way (the findings above).
- **Acceptance reads:** I traced about 14 reads to a producer in the set and each one has an edge: the `docs-site` subject, the experiment `node-loss-climb`, the search row `walk`, the `baseline.json` values, and the replay command shown on the published page. The published page's read of the replay command is recorded only by a transitive edge (`anim-docs-published-run` → `kpi-platform-gates` → `kpi-sim-fields` → `anim-point-replay`).
- **Round 1 findings:** all four are closed. The `docs-site` subject has one owner (`kpi-repository`). `anim-docs-published-run` has edges to `kpi-platform-gates` and `docs-ux-harness`, and `baseline.json` is now in its scope. `kpi-flat-searches` is ordered after `anim-search-record` through `kpi-platform-gates`.

**What I could not establish:**
- Not a finding: `story:kpi-sim-fields` depends on `story:anim-point-replay` only to order edits to `tools/sim/src/simulate.rs` and `tools/sim/tests/main.rs`. Each edge in the spine has its file reason written beside it, so I did not raise it.
  - The cost is that the `kpi-` outcomes 1 and 4, and `anim-docs-published-run` and everything behind it in `epic:docs-fidelity`, wait on one 11-link spine.
  - Splitting the shared `simulate.rs` is the alternative, and it is the drafter's call.
- Out of my lane (scope critic): `docs.status-claims-false` and `docs.commands-refused` still have no story that holds them in `quality/ratchet.yaml` (`.engineering/planning/story/kpi-docs-site.md:65-67`).

```findings
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 15
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the frontmatter edge depends_on story:docs-components-services runs opposite to its body (line 57: components edits sidebars.ts after this story and depends on it); flip the edge, or keep it and rewrite both bodies so components lands before anim, which avoids putting components behind the 11-link kpi chain"
- file: .engineering/planning/story/docs-components-services.md
  line: 91
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its Order section says it depends on story:anim-docs-published-run (which edits sidebars.ts and baseline.json before it) but its relations hold no such edge and the graph holds the reverse edge; following the body and adding that edge while the reverse stays would make a cycle components, anim, components"
```
