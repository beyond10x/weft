---
format: aep.planning-md/3
id: review-result:epics-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, KPI/animation/docs epics, round 1
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

story:anim-search-player — item 4 joins play, pause and speed, three independent outcomes in one sentence, and the check Context names (a Rust render test plus screenshots at three frames) cannot show play, pause, speed or live follow — .engineering/planning/story/anim-search-player.md:79
story:anim-search-record — items 3, 4 and 5 each join independent outcomes: the restart row and the `budget` bound, `best` and distinct non-zero walks, and the live snapshot and `result.json` — .engineering/planning/story/anim-search-record.md:107
story:kpi-sim-fields — item 2 ends "and the same for the other four fields", so the observable for `crossings_per_message`, `unschedulable_requests` and `node_hours_per_workload` is left unstated and only `instances_lost` and `placement_imbalance` have one — .engineering/planning/story/kpi-sim-fields.md:84
story:kpi-docs-site — item 4 joins the consistency test with "no second implementation of the three docs checks exists in `tools/kpis.d/`", which names no command or state that shows it — .engineering/planning/story/kpi-docs-site.md:80
story:docs-ux-harness — item 3 joins four independent outcomes (runs the same checks, kills at 60 s, fails at 120 s, prints the ten slowest tests), and "runs the same checks" names no observable — .engineering/planning/story/docs-ux-harness.md:109
story:docs-claims-check — item 1 joins the three-line output and exit 0 with two CI wirings (`slot task check`, and `pages.yml` on every pull request, planning-only ones included), and the wiring names no command or state that shows it — .engineering/planning/story/docs-claims-check.md:91
story:docs-status-proofs — item 1 joins three independent outcomes: every item listed with status and detail, the date equal to `asOf`, and `status.md` holding no table — .engineering/planning/story/docs-status-proofs.md:73
story:docs-components-services — item 4 is conditional ("once that story is in"), so it cannot be checked when this story closes, and it joins nav links, the docs build and the UX checks — .engineering/planning/story/docs-components-services.md:66

**What I read:** 21 artifacts, all in full.
- 20 stories: 7 `kpi-*`, 4 `anim-*` and 9 `docs-*`. `docs-components-services` was not in my first `aep plan artifact list`; I found it in the store later and read it too.
- The 3 epics, read only for their Outcome sections.
- Commands: `aep plan artifact kinds`, `aep plan artifact list`, `aep plan artifact show` per story, plus the story files under `.engineering/planning/story/` for line numbers.

**What I could not establish:**
- Whether the named files, tasks and hooks (`tools/check-docs.sh`, `task kpis`, `tools/kpis.d/`, `ratchet/references/`) exist in the tree. I read the stories only and ran no `git grep`.
- The four kpi/docs/anim stories not named above are otherwise checkable and have a stated transition. They also join clauses, but each clause fails by the same command or on the same surface, so I did not flag them: `kpi-ci-runs`, `kpi-repository`, `kpi-simulator`, `kpi-specification`.
- Out of my lane: `story:docs-components-services` names `story:docs-ux-harness` in its item 4 but has no `depends_on` for it. That belongs to the design and parallel-safety critics.

```findings
- file: .engineering/planning/story/anim-search-player.md
  line: 79
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 4 joins play, pause and speed, three independent outcomes, and the check named in Context (a render test plus screenshots at three frames) cannot show play, pause, speed or live follow"
- file: .engineering/planning/story/anim-search-record.md
  line: 107
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "items 3, 4 and 5 each join independent outcomes: the restart row and the budget bound, best and distinct non-zero walks, and the live snapshot and result.json"
- file: .engineering/planning/story/kpi-sim-fields.md
  line: 84
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 2 ends 'and the same for the other four fields', so the observable for crossings_per_message, unschedulable_requests and node_hours_per_workload is left unstated"
- file: .engineering/planning/story/kpi-docs-site.md
  line: 80
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 4 joins the consistency test with 'no second implementation of the three docs checks exists in tools/kpis.d/', which names no command or state that shows it"
- file: .engineering/planning/story/docs-ux-harness.md
  line: 109
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 3 joins four independent outcomes (same checks, kill at 60 s, fail at 120 s, ten slowest tests) and 'runs the same checks' names no observable"
- file: .engineering/planning/story/docs-claims-check.md
  line: 91
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 1 joins the three-line output and exit 0 with two CI wirings (slot task check and pages.yml on every pull request, planning-only ones included), and the wiring names no command or state that shows it"
- file: .engineering/planning/story/docs-status-proofs.md
  line: 73
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 1 joins three independent outcomes: every item listed with status and detail, the date equal to asOf, and status.md holding no table"
- file: .engineering/planning/story/docs-components-services.md
  line: 66
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 4 is conditional ('once that story is in') so it cannot be checked when this story closes, and it joins nav links, the docs build and the UX checks"
```
