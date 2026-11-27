---
format: aep.planning-md/3
id: review-result:epics-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, KPI/animation/docs epics, round 2
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

- story:anim-docs-published-run — item 7 names "the gate list check of that story" (`kpi-platform-gates`), but that story defines no such check, so the item cites a check that does not exist — .engineering/planning/story/anim-docs-published-run.md:133 (`grep -rn "gate list check" .engineering/planning/story` hits only this line)
- story:anim-docs-published-run — item 8 names the `task sim-test` consistency test as the check for "a measurement for each of the 21 `platform.*` KPIs", but that test (`kpi-repository`) verifies only that references resolve, so a missing measurement still passes — .engineering/planning/story/anim-docs-published-run.md:136, test scope at .engineering/planning/story/kpi-repository.md:96
- story:anim-docs-published-run — items 2 and 3 name no command or state: item 2 joins "plays with play, pause, step and speed" to "loads no resource from any URL other than itself", and item 3 joins three decision kinds to a fitness-differs claim; the only check named for the page (item 5) runs at write time and the Playwright spec drives the fixture page, not this one — .engineering/planning/story/anim-docs-published-run.md:123
- story:anim-docs-published-run — item 4 requires the docs page to name "the commit of the recording", and no item or command shows that the named commit is the one that recorded the file (a committed file cannot hold its own hash, and `task sim-publish` does not write the docs page) — .engineering/planning/story/anim-docs-published-run.md:127
- story:kpi-platform-gates — item 1 exempts the KPIs "an adversarial search maximises, which its file names with the reason", and the Shape places that reason in "a comment of the experiment". A YAML comment is not machine-readable and the format refuses unknown fields, so a program cannot tell a real exemption from an omission — .engineering/planning/story/kpi-platform-gates.md:106, Shape at :76
- story:kpi-platform-gates — item 2 ("an assessment for each of the six experiments with a measurement for each of the 21 `platform.*` KPIs") names no command, and the only consistency test in the set (`kpi-repository`) cannot see a missing measurement — .engineering/planning/story/kpi-platform-gates.md:109
- story:kpi-platform-gates — item 5 joins what `task kpis` prints to the negative "`task check` runs no experiment" and names no command that shows the negative — .engineering/planning/story/kpi-platform-gates.md:116
- story:anim-search-player — items 2 ("Stepping the player forward one step adds exactly one row to the landscape, the path and the best-so-far curve") and 7 (axis picker redraw) name no check; the Playwright spec reads only `data-frame`, `data-playing` and `data-speed`, which cannot show what is drawn — .engineering/planning/story/anim-search-player.md:108, :122
- story:anim-search-player — items 1 and 3 (cells, labels, edges, marks, curve end) have no check; the Context says the screenshots "show the drawing, not the behaviour", so the only evidence is by eye — .engineering/planning/story/anim-search-player.md:105, :110
- story:anim-search-player — item 10 joins two outcomes ("shows the points table and a line naming that the run recorded no climb, without a script error") and names no test — .engineering/planning/story/anim-search-player.md:130
- story:anim-point-replay — item 4 (the player shows the walk number and the replay command line) names no test or command, and the Tests paragraph covers only the replay command — .engineering/planning/story/anim-point-replay.md:92
- story:anim-point-replay — item 5 ("shows how to replay a search point by its walk number") names no observable, such as a section heading or the command line that `grep` finds in `run.md` — .engineering/planning/story/anim-point-replay.md:94
- story:docs-components-services — item 3 requires `docs.status-claims-false` to "stay 0", but that KPI exists only once `story:docs-status-proofs` lands and this story has no dependency on it. The item also joins three outcomes: banner, which parts run today, and two KPIs — .engineering/planning/story/docs-components-services.md:66
- story:docs-components-services — item 1 joins three outcomes (page content, generation from `ess/components.yaml`, a gate that fails on drift). The "plus the weft CLI from `design:components`" part is not in `components.yaml`, so the regeneration check cannot cover it — .engineering/planning/story/docs-components-services.md:56
- story:docs-figures — item 1 (the built figure holds every domain once, by layer, with every `--graph` reference) names no command that reads it. Item 2 compares the partial only with its own generator's output, so an incomplete generator passes both — .engineering/planning/story/docs-figures.md:78
- story:docs-figures — item 3 (four components, two channels, control host, node agent, `substrate-daemon`, Planned caption naming two files) names no command or state, and the Context says "No check of the names in the figure is added" — .engineering/planning/story/docs-figures.md:83
- story:docs-commands-check — item 3 says the snapshot hook "exits 1 when one line of a committed help file differs", but no fixture or test is named (item 2 covers only `50-commands.sh`) — .engineering/planning/story/docs-commands-check.md:75
- story:docs-commands-check — item 1 joins the count 0 to "in `pages.yml` without installing `ess` or building weft-sim", and names no state that shows the second half — .engineering/planning/story/docs-commands-check.md:70
- story:docs-claims-check — item 3 joins a static fact about `pages.yml` (step position, no `if:`) to a state that exists only after the pull request runs (the log shows the three lines) — .engineering/planning/story/docs-claims-check.md:101
- story:kpi-sim-fields — item 8 ("declares one output for every field of `Kpis`") names no command, and the consistency test checks only the reverse direction (measurement to declared output) — .engineering/planning/story/kpi-sim-fields.md:110
- story:kpi-flat-searches — item 4 joins two outcomes (a measurement for every search, and a `merge` requirement above 0 for every unmarked one) — .engineering/planning/story/kpi-flat-searches.md:102
- story:kpi-flat-searches — item 5 joins what `task kpis` prints to the negative "`task check` runs no search" and names no command that shows the negative — .engineering/planning/story/kpi-flat-searches.md:104
- story:kpi-repository — item 6 joins "`task check` runs no hook and no KPI measurement" to "its test steps stay within 60 s per test and 120 s per step", and names no command for the negative — .engineering/planning/story/kpi-repository.md:132
- story:kpi-specification — item 5 ("`task check` runs no explore, no synthesis and no other measurement of this story") names no command that shows it — .engineering/planning/story/kpi-specification.md:112
- story:kpi-simulator — item 5 ("`task check` runs none of these measurements") names no command that shows it — .engineering/planning/story/kpi-simulator.md:99
- story:kpi-ci-runs — item 4 ("No step of `task check` and no CI gate job calls `gh` or the network") names no command or file search, and "no CI gate job" has no named scope — .engineering/planning/story/kpi-ci-runs.md:94

Read 20 of 20 stories (`kpi-*` 8, `anim-*` 4, `docs-*` 8) in full with `aep plan artifact show`, plus the 3 epics. Also `aep plan artifact kinds`, `aep plan artifact lifecycle story`, `aep plan artifact list --kind story`, `git grep`-style greps of the store, and the `spec.*` rows of `design:quality-kpis`.

Not established:
- Round-1 finding text was not given to me, so I cannot say which findings repeat. Each finding above is judged on the current bodies.
- No acceptance names a path I could test against source. `tools/sim/src/` symbols and the line numbers in Context sections were not checked, because the acceptance lane does not need them.
- I did not establish whether Docusaurus Mermaid output can be read from built HTML. Both `docs-figures` findings assume the acceptance must name the command that reads the figure.
- The "holds a measurement for each; and gate KPIs are requirements in `merge`" shape appears as two statements in every `kpi-*` story. I flagged it only on `kpi-flat-searches`. The same split applies to `kpi-repository`, `kpi-specification`, `kpi-simulator`, `kpi-docs-site` and `kpi-ci-runs`.
- Out of lane: `story:docs-components-services` says in its Order that it depends on `story:anim-docs-published-run`, but its relations omit that edge. `story:anim-docs-published-run` lists `depends_on story:docs-components-services` while its own Order says the reverse. That is a possible dependency cycle for the design and parallel-safety critics. It did not set my verdict.

```findings
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 133
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 7 names the gate list check of story:kpi-platform-gates, which that story does not define, so the item cites a check that does not exist"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 136
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 8 names the task sim-test consistency test as the check for a measurement of each of the 21 platform.* KPIs, but that test verifies only that references resolve, so a missing measurement still passes"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 123
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "items 2 and 3 name no command or state, and item 2 joins play, pause, step and speed to loading no resource from any other URL; the only check named for the page runs at write time"
- file: .engineering/planning/story/anim-docs-published-run.md
  line: 127
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 4 requires the docs page to name the commit of the recording and no item or command shows the named commit is the one that recorded the file"
- file: .engineering/planning/story/kpi-platform-gates.md
  line: 106
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 1 exempts adversarial KPIs by a reason in a comment of the experiment, which no program can read, so an exemption cannot be told from an omission"
- file: .engineering/planning/story/kpi-platform-gates.md
  line: 109
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 2 (an assessment per experiment with a measurement for each of the 21 platform.* KPIs) names no command, and the consistency test of story:kpi-repository cannot see a missing measurement"
- file: .engineering/planning/story/kpi-platform-gates.md
  line: 116
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 5 joins what task kpis prints to the negative that task check runs no experiment, and names no command that shows the negative"
- file: .engineering/planning/story/anim-search-player.md
  line: 108
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "items 2 and 7 (one step adds exactly one row to landscape, path and curve; axis picker redraws) name no check, and the Playwright spec reads only data-frame, data-playing and data-speed, which cannot show what is drawn"
- file: .engineering/planning/story/anim-search-player.md
  line: 105
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "items 1 and 3 (cells, labels, edges, marks, curve end) have no check; the Context says screenshots show the drawing not the behaviour, so the only evidence is by eye"
- file: .engineering/planning/story/anim-search-player.md
  line: 130
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 10 joins the points table and the no-recorded-climb line in one item and names no test"
- file: .engineering/planning/story/anim-point-replay.md
  line: 92
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 4 (the player shows the walk number and replay command line) names no test or command, and the Tests paragraph covers only the replay command"
- file: .engineering/planning/story/anim-point-replay.md
  line: 94
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 5 (run.md shows how to replay a search point by its walk number) names no observable such as a heading or command line a grep finds"
- file: .engineering/planning/story/docs-components-services.md
  line: 66
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "item 3 requires docs.status-claims-false to stay 0, a KPI that exists only after story:docs-status-proofs, which this story does not depend on, and it joins three outcomes (banner, parts that run today, two KPIs)"
- file: .engineering/planning/story/docs-components-services.md
  line: 56
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 1 joins page content, generation from ess/components.yaml and a drift gate, and the weft CLI from design:components is not in components.yaml so the regeneration check cannot cover it"
- file: .engineering/planning/story/docs-figures.md
  line: 78
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 1 (the built figure holds every domain once by layer and every --graph reference) names no command that reads it, and item 2 compares only against the generator's own output"
- file: .engineering/planning/story/docs-figures.md
  line: 83
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 3 (the architecture figure's parts and a Planned caption naming two files) names no command or state and the Context says no check of the names is added"
- file: .engineering/planning/story/docs-commands-check.md
  line: 75
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 3 says the snapshot hook exits 1 when a committed help line differs but names no fixture or test that shows it"
- file: .engineering/planning/story/docs-commands-check.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 1 joins the count 0 to running in pages.yml without installing ess or building weft-sim, and names no state that shows the second half"
- file: .engineering/planning/story/docs-claims-check.md
  line: 101
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 3 joins a static fact about pages.yml (step position, no if:) to a state that exists only after the pull request run (the log shows the three lines)"
- file: .engineering/planning/story/kpi-sim-fields.md
  line: 110
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 8 (one declared output for every field of Kpis) names no command, and the consistency test checks only the reverse direction"
- file: .engineering/planning/story/kpi-flat-searches.md
  line: 102
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 4 joins two outcomes: a measurement of the spread for every search and a merge requirement above 0 for every unmarked one"
- file: .engineering/planning/story/kpi-flat-searches.md
  line: 104
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 5 joins what task kpis prints to the negative that task check runs no search, and names no command that shows the negative"
- file: .engineering/planning/story/kpi-repository.md
  line: 132
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 6 joins task check running no hook or measurement to the 60 s and 120 s bounds, and names no command for the negative"
- file: .engineering/planning/story/kpi-specification.md
  line: 112
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 5 (task check runs no explore, no synthesis and no other measurement) names no command that shows it"
- file: .engineering/planning/story/kpi-simulator.md
  line: 99
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 5 (task check runs none of these measurements) names no command that shows it"
- file: .engineering/planning/story/kpi-ci-runs.md
  line: 94
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "item 4 (no step of task check and no CI gate job calls gh or the network) names no command or file search and gives no scope for CI gate job"
```
