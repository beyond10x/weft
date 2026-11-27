---
format: aep.planning-md/3
id: design:quality-kpis
kind: design
status: draft
title: 'KPI catalogue: repository, CI, simulated platform, components, specification; baselines and search objectives'
summary: Measurable KPIs per subject with direction, unit, gate or ratchet role, command and 2026-11-27 baseline; mapping onto ratchet/1; what search over KPIs and its animation need
relations:
- serves: vision:weft
- informed_by: design:simulation-and-kpis
revision: 3
---
## Goal

The owner, 2026-11-27: "define kpis for when something is good (repo, ci, the cloud platform, the
individual components, etc) - and then run/exhaust the state space to find better scoring ones.
maybe we should start with kpis".

This design is the KPI catalogue: one list of measurable figures per subject, each with its
direction, unit, role (gate or ratchet or search objective), the command or file that measures it,
and a baseline measured on 2026-11-27 at `58172a7`. It extends the KPI ideas of
`design:simulation-and-kpis` § KPIs and § Hill climbing; it changes no code.

It serves objectives 1 (the specification valid and sound) and 2 (weft runs from it, conformance counted) of `vision:weft`: the `spec.*` KPIs measure the first, the component and `conformance.*` KPIs the second.

## Conventions

- **Role.** `gate`: a hard constraint, the value must be 0 or within its bound, or the state is
  rejected (in a search: infeasible). `ratchet`: may only improve against an accepted reference
  (ratchet/1 `accepted_value`). `objective`: a weight in weft-sim's `fit` fitness. `report`: shown,
  not judged.
- **Direction.** `lower` or `higher` is better.
- **Today.** `yes` (measured below), `probe` (measured with a scratch command that is not in the
  repository yet), `no` (not measurable yet, and what is missing).
- weft-sim's `fitness` maximises `sum(weight * kpi)` (`tools/sim/src/simulate.rs`, `fn fitness`), so
  a lower-is-better KPI takes a negative weight. A positive weight on a fault KPI
  (`duplicate-effects` in `sim/experiments/duplicate-delivery.yaml`) makes the search adversarial:
  it looks for the worst point. Both uses are kept; each experiment says which it is.
- Cheap and deterministic first: every `yes` figure below came from a run under 45 s, and the
  weft-sim figures are deterministic per seed (test `walks_do_not_depend_on_the_thread_count`).
- Ids are `<subject>.<name>`; the name part matches the weft-sim `Kpis` field (with `-` for `_`) where one
  exists, so `search.weights` and `search.gates` can name it as it stands.

## How the baseline was measured

All on 2026-11-27, worktree at `58172a7`, `ess 0.99.2`, rustc 1.98.1, cargo-nextest 0.9.143.
Builds and tests went through the shared build pool (`slot`); weft-sim runs ran without it (they
measure). Warm compile cache unless noted. weft-sim was built from `tools/sim` (`cargo build`, cold
target directory: 2 min 21 s).

| Ref | Command |
|---|---|
| M1 | `slot task check` (whole local gate): exit 0, 20 s wall |
| M2 | `slot ess specify validate --path ess --strict-requires`: `weft v1 — 25 file(s), valid`, 3.6 s |
| M3 | `tools/check-layers.sh` 1.6 s; `tools/check-stamped-time.sh` 1.2 s, `--negative` 1.4 s; `tools/test-planning-only.sh` 0.1 s; `slot tools/test-gate-budget.sh` 17.5 s, "30 cases pass" |
| M4 | `slot task test`: 3 s wall, 2 tests, slowest 0.008 s; `task sim-test` (`tools/sim`): build 4.0 s, run 3.2 s, 18 tests, slowest 0.089 s |
| M5 | `slot ess generate synthesize --path ess --target rust --out target/kpis-synth --format json`: exit 1 in 5 s, `ess-target-failure/1`; `jq '.causes|length'` = 8; capabilities `generated` 2737, `obligation` 285 |
| M6 | `weft-sim simulate --spec ess --experiment sim/experiments/<name>.yaml --out <dir> --threads 4 --no-search`, each of the 6 experiments; `jq '.run.result.kpis' <dir>/result.json` |
| M7 | the same without `--no-search` (the experiment's own search) |
| M8 | `weft-sim explore --spec ess --seed 1 --threads 8 --walks 2048 --steps 300`: 41 s |
| M9 | `weft-sim explore --spec ess --seed 1 --threads 8 --walks 64 --steps 300 --from sim/experiments/full-setup.yaml --settle 12`: 12.9 s (the ratchet's `explore-from-setup` uses 1024 walks; not run) |
| M10 | `gh run list --repo beyond10x/weft --limit 200` (21 runs, all of them 2026-11-27) and `gh run view <id> --json jobs` for each completed run |
| M11 | `slot ess specify protocol validate --path protocols/<name>.yaml`: both valid, 2.5-2.7 s |
| M12 | counts with `git ls-files`, `grep`, `jq` on the tree (commands in the rows) |

## 1. Specification (`spec.*`)

| Id | Definition | Dir | Unit | Role | Measured by | Today |
|---|---|---|---|---|---|---|
| `spec.validate-errors` | `ess specify validate --strict-requires` not valid | lower | 0/1 | gate | `task spec` | yes: 0 (M2) |
| `spec.validate-seconds` | wall time of the validation | lower | s | ratchet, bound 120 | M2 timed | yes: 3.6 |
| `spec.layer-violations` | references to a higher layer | lower | refs | gate | `tools/check-layers.sh` | yes: 0 (M3) |
| `spec.stamped-time-violations` | `StampedAt` commands granted outside `tools/stamped-time-actors` | lower | commands | gate | `tools/check-stamped-time.sh` | yes: 0; 7 inputs in 7 commands (M3) |
| `spec.protocol-validate-errors` | protocol documents not valid | lower | files | gate | M11 | yes: 0 of 2 |
| `spec.open-markers` | `# OPEN:` questions in `ess/` | lower | markers | ratchet | `grep -rn '# OPEN:' ess \| wc -l` | yes: 404 |
| `spec.unmapped-markers` | `UNMAPPED` markers in `ess/` | lower | markers | ratchet | `grep -rn UNMAPPED ess \| wc -l` | yes: 53 |
| `spec.rust-synthesis-refusals` | causes of `ess-target-failure/1` for `--target rust` | lower | causes | ratchet (gate at 0) | M5, `jq '.causes\|length'` | yes: 8, all `missing-representation` (`one_time_response`; gap E1 of `docs/implementation.md`) |
| `spec.obligations` | capabilities synthesis leaves to hand-written code | report | capabilities | report | M5 | yes: 285 (2737 generated) |
| `spec.outcomes-reached` | outcomes some explore step answered, of 2525 declared | higher | outcomes | ratchet `no_lost_items`, search floor | M8 / M9 `outcomes_reached` | yes: 1278 (M8), 1386 (M9) |
| `spec.states-reached` | lifecycle states held, of 328 | higher | states | ratchet | M8 / M9 | yes: 150 (M8), 186 (M9) |
| `spec.transitions-reached` | transitions taken, of 221 | higher | transitions | ratchet | M8 / M9 | yes: 89 (M8), 106 (M9) |
| `spec.commands-never-accepted` | commands with no accepting answer, of 439 | lower | commands | ratchet | `jq '.gaps.commands_never_accepted\|length'` | yes: 228 (M8), 163 (M9) |
| `spec.commands-blocked-by` | never-accepted commands a guard on stored rows refused every time | lower | commands | ratchet | `jq '.gaps.blocked_by_count'` | yes: 4 (M8), 11 (M9) |
| `spec.entities-never-held` | entities no walk created a row of | lower | entities | ratchet | `jq '.gaps.entities_never_held\|length'` | yes: 56 (M8), 38 (M9) |
| `spec.abstract-states` | distinct abstract states seen | lower, with the outcomes floor | states | objective of the spec hill climb | `abstract_states` | yes: 114836 (M8), 5005 (M9) |
| `spec.branch-points` | command at an abstract state answering two or more outcomes | report | points | report | `branch_points` | yes: 3296 (M8), 232 (M9) |
| `spec.branch-points-without-guard` | branch points no guard or invariant decides | lower | points | objective | needs a weft-sim field | no: weft-sim counts branch points but does not classify them by guard |
| `spec.invariant-violations` | system invariant violations in explore | lower | violations | gate | `violation_count` | yes: 0 (M8, M9) |
| `spec.unsupported-answers` | interpreter answers `unsupported` | lower | commands | ratchet `no_new_items` | `errors` | yes: 0 (M8, M9) |
| `spec.undeclared-refusals` | refusals no declared outcome covers | lower | commands | gate | `errors` | yes: 0 (M8, M9) |
| `spec.unowned-or-uncallable` | commands, views, events with no component or no actor and no comment saying why (`docs/goal.md` item 3) | lower | items | gate | needs a checker | no: no tool counts it |
| `spec.hardening-passes` | recorded passes `docs/hardening/pass-<n>.md` | higher | passes | ratchet, target 3 (`docs/goal.md` item 6) | `ls docs/hardening/pass-*.md \| wc -l` | yes: 2 |
| `spec.conformance-failed-interpreted` | failed scenarios of the synthesized suite on the interpreted target | lower | scenarios | gate | `ess verify conform run --target interpreted` | no: no committed suite (`story:skeleton-conformance-suite`) |
| `spec.suite-synthesis-seconds` | conformance suite synthesis per domain | lower | s | ratchet, bound 30 min per job | `tools/synth-domain.sh` timed | no: expensive; last recorded 451 s for 17 domains (`docs/goal.md`, 2026-11-26), workloads subset over 30 min (gap G5) |

## 2. Cloud platform as simulated by weft-sim (`platform.*`)

Per experiment, read from `run.result.kpis` of `result.json` (weft-sim `Kpis`,
`tools/sim/src/simulate.rs`). Every field is a deterministic count per seed.

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `platform.violations` | invariant violations (weft-sim and interpreter) | lower | violations | gate | yes |
| `platform.setup-failures` | setup steps not accepted as declared | lower | steps | gate (infeasible point) | yes |
| `platform.double-placements` | instance assigned to two nodes at once | lower | instances | gate | yes |
| `platform.quota-breaches` | quota drawn above its allocation | lower | breaches | gate | yes |
| `platform.cross-tenant-reach` | tenants sharing a kernel below `microvm`, secrets outside scope | lower | pairs | gate | yes |
| `platform.closed-link-pairs` | a formation's placements in groups a closed link joins | lower | pairs | gate | yes |
| `platform.duplicate-effects` | a redelivered event that changed something | lower | effects | gate (objective with + weight in an adversarial search) | yes |
| `platform.unconverged` | controllers still sending after `converge_within` | lower | runs | gate | yes |
| `platform.interpreter-gaps` | interpreter answers that are not an outcome | lower | answers | gate | yes |
| `platform.messages-unrouted` | traffic with no running instance at an end | lower | messages | gate | yes |
| `platform.pattern-matches` | matches of the experiment's bad-state patterns (`find.patterns`) | lower | matches | gate in the ratchet; objective (-10) in searches today | yes |
| `platform.steps-to-converge` | controller ticks until no work after the last fault | lower | ticks | objective | yes |
| `platform.link-crossings` | topology links the traffic's messages crossed | lower | crossings | objective | yes |
| `platform.crossings-per-message` | `link_crossings / messages` | lower | ratio | objective | no: new weft-sim field; today's absolute count also falls when the traffic falls |
| `platform.chain-reached` | leading commands of `chain` the model accepted | higher | commands | gate at the chain length | yes |
| `platform.outcomes-reached` | outcomes reached in the run | higher | outcomes | objective floor | yes |
| `platform.abstract-states` | distinct abstract states of the run | report | states | report | yes |
| `platform.instances-lost` | instances not running after a node loss and the settle | lower | instances | gate | no: new weft-sim field (named in `design:simulation-and-kpis`) |
| `platform.unschedulable-requests` | scheduling requests no node could take | lower | requests | ratchet | no: new weft-sim field |
| `platform.placement-imbalance` | max over mean active instances per node | lower | ratio | objective | no: new weft-sim field (rows exist in `weft.nodes.Assignments`) |
| `platform.node-hours-per-workload` | metered node time per workload | lower | node-ticks | objective (cost) | no: new weft-sim field over the metering rows |

Baseline, one run per experiment without its search (M6, wall time includes the start):

| Experiment | wall s | steps | steps-to-converge | link-crossings / messages | outcomes | abstract states | chain-reached | every gate KPI |
|---|---|---|---|---|---|---|---|---|
| closed-link | 0.95 | 38 | 0 | 0 / 0 | 36 | 35 | 5 | 0 |
| duplicate-delivery | 0.86 | 15 | 0 | 0 / 0 | 13 | 13 | 0 | 0 |
| node-loss | 0.99 | 29 | 2 | 0 / 0 | 10 | 11 | 0 | 0 |
| cluster-pool-lifecycle | 1.50 | 31 | 1 | 0 / 0 | 19 | 26 | 7 | 0 |
| stuck-placement | 3.13 | 357 | 6 | 948 / 672 | 50 | 74 | 33 | 0 |
| full-setup | 1.33 | 295 | 3 | 660 / 480 | 50 | 73 | 33 | 0 |

The same experiments with their own search (M7):

| Experiment | mode | points | wall s | fitness range | best point | what it shows |
|---|---|---|---|---|---|---|
| duplicate-delivery | fit, budget 12 | 12 | 1.97 | 1.3 to 1.3 | draw 1, duplicated create-tenant | flat: no point produces a duplicate effect; 2 points infeasible (draw 11 refused in setup) |
| node-loss | exhaustive | 9 | 1.89 | -10 to 0 | fault-tick 5, heartbeat-every 1 | heartbeat-every 3 matches pattern `healthy-node-lost` at every fault tick: a finding the ratchet does not hold, because the ratchet reads only `violations` |
| cluster-pool-lifecycle | exhaustive | 6 | 2.07 | -1 to -1 | first point | flat: every point converges in 1 tick |
| stuck-placement | exhaustive | 8 | 18.96 | -948 to -336 | Star, replicas 1, 4 nodes per pool | discriminates on link crossings |
| full-setup | exhaustive | 8 | 17.07 | -660 to -240 | Star or Ring, replicas 1, 4 nodes per pool | discriminates on link crossings; messages are 480 at every point |
| closed-link | none | - | 1.49 | - | - | no `search:` block |

## 3. Components

### Control host, the `weft` binary (`host.*`)

Not built yet (`story:skeleton-control-host`); the root workspace holds `crates/weft-testkit` only.

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `host.obligations-open-on-path` | obligations on the skeleton path still answering "not implemented" | lower | obligations | ratchet | no: no binary |
| `host.ready-seconds` | start to the first answered request, empty SQLite store | lower | s | ratchet | no: no binary |
| `host.request-p95-ms` | p95 latency of the conformance run's requests | lower | ms | ratchet | no: no binary |
| `host.binary-bytes` | size of the release binary | lower | bytes | report | no: no binary |

### Node agent, `weft-agent` (`node.*`)

Not built yet (`story:skeleton-node-join-session`, `-node-run-assignment`, `-node-confine`).

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `node.inbound-ports` | listening ports the agent opens (decision A5: none) | lower | ports | gate 0 | no: no agent |
| `node.join-seconds` | `JoinNode` sent to session open | lower | s | ratchet | no: no agent |
| `node.assignment-to-running-seconds` | assignment read to `RecordInstanceRunning` | lower | s | ratchet | no: no agent |
| `node.confined-exec-failures` | confined starts that did not reach `running` | lower | execs | gate | no: no agent |

### Controllers, `weft-controllers` (`controllers.*`)

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `controllers.steps-to-converge` | ticks to no work after a fault, on the real control host | lower | ticks | ratchet; compare with `platform.steps-to-converge` | no: no crate (`story:skeleton-controllers`); weft-sim's figure is the model's |
| `controllers.idle-sends` | requests sent in a tick with nothing to repair | lower | requests | gate 0 | no: no crate |
| `controllers.unchecked-against-spec` | level and time steps run by a hand-written controller with no binding or loop step in `ess/components.yaml` | lower | steps | ratchet | no: needs a checker (`design:components`: the steps are a comment table) |

### Generated crates (`generated.*`)

`generated/` does not exist yet (`story:skeleton-generated-code`). The synthesis figures are under
`spec.*` (M5).

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `generated.digest-stale` | committed digest differs from the digest of `ess/` | lower | 0/1 | gate (fast check) | no: no `generated/` |
| `generated.regen-byte-diff` | bytes that differ after regeneration (slow lane) | lower | files | gate in `task slow` | no: no `generated/` |
| `generated.hand-edits` | files under `generated/` changed by a commit that is not a regeneration | lower | files | gate | no: no `generated/` |
| `generated.build-seconds` | cold build of the generated crates in the pool | lower | s | ratchet, bound 10 min per job | no: no `generated/` |

### Conformance target (`conformance.*`)

| Id | Definition | Dir | Unit | Role | Today |
|---|---|---|---|---|---|
| `conformance.failed` | failed scenarios against the control host | lower | scenarios | gate 0 | no: no suite, no host (`story:skeleton-conformance-suite`) |
| `conformance.executed` | scenarios that executed | higher | scenarios | ratchet | no: same |
| `conformance.skipped` | scenarios skipped, by reason | lower | scenarios | ratchet `no_new_items` | no: same |
| `conformance.step-seconds` | conformance step in the gate | lower | s | gate, bound 110 (`docs/implementation.md`) | no: same |
| `conformance.suite-digest-stale` | committed suite's source digest differs from `ess/` | lower | 0/1 | gate | no: same |

### weft-sim itself (`sim.*`)

| Id | Definition | Dir | Unit | Role | Measured by | Today |
|---|---|---|---|---|---|---|
| `sim.test-step-seconds` | `task sim-test` run phase | lower | s | gate 120 | ratchet `simulator-tests` | yes: 3.2 (M4) |
| `sim.slowest-test-seconds` | slowest weft-sim test | lower | s | gate 60 | nextest | yes: 0.089 (M4) |
| `sim.tests` | tests in `tools/sim/tests/main.rs` | report | tests | report | nextest | yes: 18 |
| `sim.explore-steps-per-second` | throughput of a plain explore | higher | steps/s | ratchet | `steps_per_s` | yes: 15594 (M8); 1899 with `--from` (M9) |
| `sim.explore-from-seconds` | the ratchet's `explore-from-setup` run | lower | s | gate 300 (ratchet) | `elapsed_s` | no: the 1024-walk run was not run; 64 walks took 12.9 s (M9) |
| `sim.search-fitness-spread` | max minus min fitness over a search's feasible points | higher | fitness | report (0 means the search gave no signal) | `jq '[.search[]\|select(.feasible).fitness]'` | yes: 0 in duplicate-delivery and cluster-pool-lifecycle, 10 node-loss, 612 stuck-placement, 420 full-setup (M7) |
| `sim.ess-revisions` | distinct ESS versions in use (CLI of the gate, crates of weft-sim) | lower | versions | gate 1 | `ess --version`, `tools/sim/Cargo.toml` | probe: 2 (CLI 0.99.2; crates at rev `c0b5f560`, 0.99.5) |
| `sim.debug-sections` | `.debug_*` sections in the weft-sim binary | lower | sections | gate 0 | `readelf -S` | yes: 0 (binary); the test binaries are built in the pool and were not in the tree to check |

### Docs site, `website/` (`docs.*`)

| Id | Definition | Dir | Unit | Role | Measured by | Today |
|---|---|---|---|---|---|---|
| `docs.build-failures` | Docusaurus build or typecheck fails (`onBrokenLinks`, `onBrokenAnchors`, `onBrokenMarkdownLinks`: `throw`) | lower | 0/1 | gate | `pages.yml` | yes: 0 of 3 completed runs (M10) |
| `docs.build-seconds` | "Build the Docusaurus site" step in CI | lower | s | ratchet | M10 | yes: 34, 55, 70 |
| `docs.dead-repo-links` | links to `github.com/beyond10x/weft/(tree\|blob)/main/<path>` whose path does not exist | lower | links | gate | probe: `grep -rhoE` over `website/docs`, `test -e` | probe: 0 of 15 |
| `docs.planned-without-banner` | a `planned` item of `website/data/status.json` whose page neither has `:::caution[Planned]` nor says "Planned" at the anchor | lower | items | gate | probe: `jq` + `grep` | probe: 0 (2 pages with banners; `specification/read#the-files` says "Planned" inline) |
| `docs.status-claims-false` | a `shipped` status item whose claim does not hold on the tree (file counts, commands, components) | lower | items | gate | needs a checker | no: no checker; the 23-domain and 4-component claims were checked by hand and hold |
| `docs.commands-refused` | a command line in the docs that the named binary refuses (unknown flag, unknown subcommand) | lower | lines | gate | needs a checker over code blocks | no: no checker |
| `docs.status-age-days` | `asOf` of `status.json` against the date of `HEAD` | lower | days | ratchet, bound 7 | `jq -r .asOf`, `git log -1 --format=%cs` | yes: 0 |

## 4. Repository (`repo.*`)

| Id | Definition | Dir | Unit | Role | Measured by | Today |
|---|---|---|---|---|---|---|
| `repo.gate-seconds` | `slot task check`, warm cache | lower | s | ratchet, bound 600 (pool) | M1 | yes: 20 |
| `repo.test-step-seconds` | root `tools/test-step.sh`, after `task check` built the test binaries | lower | s | gate 120 | `slot tools/test-step.sh` | yes: 0.6 |
| `repo.slowest-test-seconds` | slowest root workspace test | lower | s | gate 60 | `target/nextest/slowest.txt` | yes: 0.008 |
| `repo.gate-budget-cases-failed` | cases of `tools/test-gate-budget.sh` that fail | lower | cases | gate | M3 | yes: 0 of 30 |
| `repo.test-binaries-per-crate` | integration test binaries per crate | lower | binaries | gate 1 | `ls <crate>/tests/*.rs` | yes: 1 (`weft-testkit`), 1 (`tools/sim`) |
| `repo.python-files` | committed `*.py` files and committed files whose first line is a python shebang, each counted once | lower | files | gate 0 | `git ls-files '*.py'` and `git grep -InE '^#!.*python'` at line 1 | yes: 0 |
| `repo.toolchain-pins-disagree` | `rust-toolchain.toml` channels that differ (root, `tools/sim`) | lower | files | gate 0 | `grep channel` | yes: 0 (both 1.98.1) |
| `repo.cache-bypass` | a setting of the shared compile cache's server (`SCCACHE_SERVER_PORT`, `SCCACHE_DIR`, `SCCACHE_CACHE_SIZE`), or an empty compiler wrapper: `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER` set to `""`, `''` or nothing in shell, YAML or TOML, or `rustc-wrapper = ""`; outside the planning store, the KPI hook and its test fixtures | lower | lines | gate 0 | `git grep -nE` with the pattern in `tools/kpis.d/10-repository.sh` | yes: 0 |
| `repo.agents-md-dead-paths` | repository paths named in `AGENTS.md` that do not exist | lower | paths | gate 0 | probe: backticked paths, `test -e` | probe: 2 (`surfaces/http/weft-api.yaml`, `surfaces/cli/weft.yaml`; the docs site says they are planned) |
| `repo.plan-validate-defects` | defects `aep plan artifact validate` reports | lower | defects | gate 0 | `aep plan artifact validate` | yes: 0 (18 artifacts, before this design) |

## 5. CI (`ci.*`)

From M10: 21 runs, all on 2026-11-27, so the rates below rest on few runs.

| Id | Definition | Dir | Unit | Role | Measured by | Today |
|---|---|---|---|---|---|---|
| `ci.gate-seconds-warm` | "Run the gate" step with a rust-cache hit | lower | s | ratchet, bound 120 for the test part | M10 | yes: 46, 46 |
| `ci.gate-seconds-cold` | the same with a cache miss | lower | s | ratchet | M10 | yes: 352, 816 |
| `ci.gate-job-seconds` | whole gate job | lower | s | report (job timeout 30 min) | M10 | yes: 80, 95, 376, 849; 5 for a planning-only pull request |
| `ci.failed-runs` | completed runs with conclusion `failure`, per workflow | lower | runs | ratchet | M10 | yes: Gate 0 of 5, Documentation validation 0 of 3, Shared source gates 1 of 6, Documentation site 0 of 1 |
| `ci.red-after-green-local` | forge runs red on a commit whose local gate was green | lower | runs | gate 0 (`~/beyond10x/AGENTS.md` "The local gate equals the forge" is the source rule) | needs a record of the local gate result per commit | no: no record |
| `ci.planning-only-misclassified` | cases of `tools/test-planning-only.sh` that fail, both sides | lower | cases | gate 0 | M3 | yes: 0 |
| `ci.unpinned-actions` | `uses:` lines not pinned to a 40-hex commit | lower | lines | gate 0 | `grep -h 'uses:' .github/workflows/*.yml \| grep -vE '@[0-9a-f]{40}'` | yes: 0 |

## Count

| Subject | KPIs | measured (`yes`) | probe | not yet |
|---|---|---|---|---|
| specification | 25 | 21 | 0 | 4 |
| platform (weft-sim) | 21 | 16 | 0 | 5 |
| control host | 4 | 0 | 0 | 4 |
| node agent | 4 | 0 | 0 | 4 |
| controllers | 3 | 0 | 0 | 3 |
| generated crates | 4 | 0 | 0 | 4 |
| conformance target | 5 | 0 | 0 | 5 |
| weft-sim | 8 | 6 | 1 | 1 |
| docs site | 7 | 3 | 2 | 2 |
| repository | 10 | 9 | 1 | 0 |
| CI | 7 | 6 | 0 | 1 |
| total | 98 | 61 | 4 | 33 |

## Mapping onto ratchet/1 (`quality/ratchet.yaml`)

Already measured by an assessment (subjects `specification` and `simulator`):

| KPI | Assessment, measurement |
|---|---|
| `spec.outcomes-reached` | `explore-from-setup`, `outcomes-reached` (`no_lost_items`) |
| `spec.invariant-violations` | `explore-from-setup`, `invariant-violations` (`lte 0`) |
| `spec.unsupported-answers` | `explore-from-setup`, `unsupported-answers` (`no_new_items`) |
| `spec.undeclared-refusals` | `explore-from-setup`, `undeclared-refusals` (`lte 0`) |
| `spec.abstract-states` | `explore-from-setup`, `abstract-states` (no requirement) |
| `sim.explore-from-seconds` | `explore-from-setup`, `explore-time` (`lte 300 s`) |
| `platform.violations` | `experiment-<name>`, `invariant-violations`, 5 experiments (`closed-link` has no assessment) |
| `sim.test-step-seconds` | `simulator-tests`, `test-time` (`lte 60 s` per test) |

What is missing for the rest:

- **weft-sim-report@1 outputs.** The assessor declares six outputs; every other `Kpis` field
  (`platform.*`), `states_reached`, `transitions_reached`, the gap lists, `blocked_by_count` and
  `steps_per_s` need an output each. Every `platform.*` gate needs an assessment per experiment and
  `closed-link` needs one at all. `platform.pattern-matches` is the urgent one: the node-loss search
  matches `healthy-node-lost` and no requirement reads it.
- **New weft-sim `Kpis` fields.** `crossings-per-message`, `instances-lost`,
  `unschedulable-requests`, `placement-imbalance`, `node-hours-per-workload`,
  `branch-points-without-guard`.
- **New assessors.** A `repo-facts` assessor (POSIX sh, counts of § 4 and the `spec.*` markers);
  an `ess-synthesis` assessor over `ess-target-failure/1` and the synthesis plan
  (`spec.rust-synthesis-refusals`, `spec.obligations`); a `ci-runs` assessor over `gh run view
  --json jobs` (§ 5; it reads the network, so it runs in a `slow` or scheduled lane, never the test
  step); a `docs-site` assessor (§ docs site); the preset `conformance-report@1` for
  `conformance.*`; the preset `timing-report@1` for the gate steps.
- **New subjects.** `repository`, `ci`, `platform`, `control-host`, `node-agent`, `controllers`,
  `generated`, `conformance`, `docs-site` beside `specification` and `simulator`.
- **Assessor binaries.** Neither `weft-sim-report` nor `ratchet` is on `PATH` here; the ratchet file
  is written ahead of `ratchet validate` (default 3 of `design:simulation-and-kpis`). Until then a
  `task kpis` can print the catalogue's `yes` values with the same commands, outside the gate.

## Search over KPIs

| State space | Mutation points | Scored by | Status |
|---|---|---|---|
| a weft installation, simulated | `var`, `vars`, `fault` mutations of an experiment (topology, replicas, pool size, fault tick and target, heartbeat interval, quota draw) | `platform.*`: gates as `search.gates`, objectives as `search.weights` | runs today: `fit` (hill climb over neighbours, random restart) and `exhaustive` (refused over `limit`) |
| the specification | a guard present or absent, a threshold, an invariant added, applied to a copy of `ess/` | `spec.abstract-states` down, `spec.outcomes-reached` as a floor, `spec.invariant-violations` and `spec.undeclared-refusals` as gates, `spec.validate-errors` 0 | not built: `apply_point` refuses any kind but `var`, `vars`, `fault` ("spec variants come later"); each point needs a catalog of its own copy |
| explore's own parameters | `--walks`, `--steps`, `--controller-share`, `--settle`, `--check-every` | `spec.outcomes-reached` per second of `sim.explore-from-seconds`, within the 300 s gate | possible with a small driver; it chooses the measurement's budget, not the system |
| repository and CI configuration | nextest threads, cache keys | `repo.gate-seconds`, `ci.gate-seconds-*` | not searched: few points, and a CI point costs a forge run with network; these stay ratchets |

Findings from the cheap runs (M7) that a search design must answer:

- Two of five searches are flat (fitness spread 0): their KPIs do not respond to their mutation
  points. `sim.search-fitness-spread` reports it; a flat search is a design defect of the
  experiment, not a result.
- Absolute counts reward less traffic; `crossings-per-message` removes that.
- In `fit`, the random restart after a round without improvement sets the current point without
  evaluating it, so that point is never recorded (`simulate.rs`, `None => { current = ...; current_score = f64::MIN }`).

What a first animated view of a hill-climb run needs, against what `result.json` `search` records
today (`record_point`: `point`, `kpis`, `fitness`, `feasible`, `error`, in evaluation order):

| Per point | Today |
|---|---|
| sequence number | implicit (array order) |
| round: which neighbours were evaluated together | missing |
| coordinates as value indices per mutation (for axes; values may be maps such as `topology`) | missing: only the values |
| the point it was a neighbour of | missing |
| decision: start, accepted, rejected, restart | missing |
| best so far after the point | missing (the page recomputes the final best only) |
| fitness, feasible, KPIs, error | recorded |
| walk number, to replay the point | missing (`base_walk + i` is not written) |
| run header: mutation axes with their value lists, weights, gates, mode, budget, seed | partly: `run` has the experiment path, not the axes |

The page (`tools/sim/src/page.html`, section Search) draws one table of points and marks the
final best; `--serve` serves it live. An animation needs the missing columns above, the restart
points evaluated and recorded, and a view that plays the rows in order over two chosen axes.

## Search objectives first

1. `platform.steps-to-converge` (lower): the repair speed of the reconciliation loops; it already
   separates points in node-loss (0 to 3).
2. `platform.pattern-matches` (lower, large weight): names bad states directly; the node-loss search
   found `healthy-node-lost` with it.
3. `platform.crossings-per-message` (lower): placement quality against the topology; the absolute
   `link-crossings` separates points today (240 to 948) but rewards less traffic.
4. `platform.outcomes-reached` (higher, small weight or floor): keeps a search from scoring well by
   doing less.
5. `spec.abstract-states` (lower, with the outcomes floor): the hill-climb target of
   `design:simulation-and-kpis`, once specification variants are a mutation kind.

Gates in every search: `platform.violations`, `setup-failures`, `cross-tenant-reach`,
`double-placements`, `quota-breaches`, `duplicate-effects`, `unconverged`, `messages-unrouted`,
`interpreter-gaps`. Today three experiments gate on `setup-failures` only, and stuck-placement and
full-setup name no gate.

## Proposed epics (not created)

1. **KPI catalogue in ratchet**
   - Every `yes` and `probe` KPI of this catalogue has an assessment in `quality/ratchet.yaml`, with
     its subject and assessor, and its baseline as the accepted reference.
   - weft-sim writes the new `Kpis` fields and `weft-sim-report@1` reads every field; every
     experiment gates on every `platform.*` gate.
   - `task kpis` prints every value with its command, outside the test step, until `ratchet`
     checks the file in the gate.
2. **Search animation**
   - weft-sim's `search` output records per point the round, the value indices, the parent point,
     the decision, the best so far and the walk; restart points are evaluated and recorded.
   - The page plays a search run step by step over two chosen axes, live with `--serve` and from a
     finished `result.json`.
   - A point of the animation replays by its walk number.
3. **Docs-site fidelity**
   - A checker holds `docs.status-claims-false`, `docs.commands-refused`,
     `docs.planned-without-banner`, `docs.dead-repo-links` and `repo.agents-md-dead-paths` at 0, in
     the docs build of `pages.yml` and in `task check`.
   - `status.json` items are checked against the tree, not written by hand only.
   - The 2 dead paths in `AGENTS.md` are fixed or marked planned.
