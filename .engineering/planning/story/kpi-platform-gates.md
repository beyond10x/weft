---
format: aep.planning-md/3
id: story:kpi-platform-gates
kind: story
status: draft
title: Every experiment gates on every platform gate KPI and is assessed in quality/ratchet.yaml on every platform KPI, the node-loss pattern match included
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-repository
- depends_on: story:kpi-sim-fields
- depends_on: story:kpi-specification
scope:
- confidence: cited
  path: quality/assessors/weft-sim-report@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: cited
  path: sim/experiments
- confidence: inferred
  path: tools/kpis.d/40-platform.sh
- confidence: cited
  path: tools/sim/src/experiment.rs
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
revision: 12
---
## Epic

Decomposes `epic:kpi-ratchet`, outcome 2, second half (every experiment gates on every
`platform.*` gate KPI, the node-loss pattern match held), and outcomes 1 and 4 for the subject
`platform`. Depends on `story:kpi-repository` (task `kpis`, references) and `story:kpi-sim-fields`
(the new fields and `chain-missing`).

Order: seventh link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` -> this story ->
`story:kpi-flat-searches` -> `story:kpi-simulator` -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). It depends on `story:kpi-specification`, which edits
`quality/ratchet.yaml` and `quality/assessors/weft-sim-report@1.assessor.yaml` before it;
`story:kpi-flat-searches` and `story:kpi-simulator` edit `quality/ratchet.yaml` after it.

Of the weft-sim sources it edits `cloudspec@2ed72c2:crates/cloudspec-sim/src/experiment.rs` (the `search.maximised` field) and
`cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs` (its tests), both on the same chain: `story:kpi-flat-searches` edits
both after it.

`sim/experiments/` holds six experiments when this story lands. `story:anim-docs-published-run`
adds a seventh, `node-loss-climb.yaml`, and depends on this story: that story gives its experiment
every gate KPI of this story in `search.gates` and an assessment in `quality/ratchet.yaml` with a
measurement for every `platform.*` KPI and its gate requirements in the `merge` gate, so the rule
of this story holds for every experiment in the directory.

## Outcome

A search over an experiment can no longer score a point that breaks a platform gate, and
`quality/ratchet.yaml` holds every `platform.*` KPI of every experiment, so the finding of
2026-11-27 (node-loss with `heartbeat-every 3` matches `healthy-node-lost`, which no requirement
reads) is held by a requirement.

## Context

- `design:quality-kpis` § 2: 21 `platform.*` KPIs, 16 measured today and five added by
  `story:kpi-sim-fields`; every one is a field of weft-sim `Kpis`, read per experiment from
  `run.result.kpis` of `result.json` (M6) and per search point from `search[].kpis` (M7).
- Today three experiments gate on `setup-failures` only, stuck-placement and full-setup name no
  gate, and closed-link has no `search:` block (§ Search objectives first). `quality/ratchet.yaml`
  reads only `violations`, for five experiments, under the subject `specification`; closed-link has
  no assessment.
- Baselines (M6, one run per experiment without its search): every gate KPI 0 in all six; the
  objective figures in the § 2 baseline table.
- The node-loss search (M7, exhaustive, 9 points) matches `healthy-node-lost` at every fault tick
  with `heartbeat-every 3`; weft-sim records each match with its pattern, walk and seed
  (`cloudspec@2ed72c2:crates/cloudspec-sim/src/simulate.rs:676`).

Shape (taken here):

- Gate KPIs of every search: `violations`, `setup-failures`, `double-placements`, `quota-breaches`,
  `cross-tenant-reach`, `closed-link-pairs`, `duplicate-effects`, `unconverged`,
  `interpreter-gaps`, `messages-unrouted`, `pattern-matches`, `instances-lost`, and
  `chain-missing` in an experiment with a `chain`. An adversarial search (a fault KPI with a
  positive weight, today duplicate-delivery: `duplicate-effects`, `quota-breaches`) keeps the KPI it
  looks for as an objective and not as a gate, and says so in a field weft-sim parses
  (`design:quality-kpis` § Conventions: "each experiment says which it is"):
  `search.maximised: [{kpi: <id>, reason: <words>}]`. The experiment format refuses unknown fields
  (`cloudspec@2ed72c2:crates/cloudspec-sim/src/experiment.rs:231`, `Search`, `#[serde(deny_unknown_fields)]`), so this story
  adds the field to `Search`; weft-sim refuses an entry without a reason, an entry whose KPI is
  also in `gates`, and an entry whose KPI has no positive weight.
- Tests in `cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs` (the module of `story:kpi-repository`, run by
  `task sim-test`, file-only, each under 1 s):
  - `ratchet::every_search_gates_every_platform_gate_kpi`: reads every file of `sim/experiments/`
    and fails when one with a `search:` block lacks a gate KPI of the list above in
    `search.gates` and does not name it in `search.maximised`;
  - `ratchet::every_experiment_measures_every_platform_kpi`: enumerates the `platform.*` KPIs from
    the fields of `weft_sim::simulate::Kpis::default()` (serialised to JSON, numeric fields, minus
    the fields the test lists as not a `platform.*` KPI of `design:quality-kpis` § 2, and
    asserting that 21 remain) and every file of `sim/experiments/`, and fails naming each pair of
    experiment and KPI without a measurement in that experiment's assessment;
  - `ratchet::every_experiment_gate_kpi_is_a_merge_requirement`: fails naming each pair of
    experiment and gate KPI (maximised ones included: the run without search holds them at 0)
    that is not a requirement listed in the `merge` gate;
  - `ratchet::check_runs_no_experiment`: reads the `check` and `test` tasks of `Taskfile.yml` with
    every task they call, and the files of `tools/check.d/`, and fails when one names
    `sim-experiments`, `weft-sim simulate` or `tools/kpis.d/40-platform.sh`.
- `quality/ratchet.yaml`: a subject `platform`; one assessment per experiment, the six of
  `sim/experiments/`, closed-link included, under `platform` (the five existing ones move from
  `specification`), each with a measurement for every `platform.*` KPI.
- Requirements: each gate KPI of each experiment at 0 in the `merge` gate; the ratchet and
  objective KPIs (`steps-to-converge`, `link-crossings`, `crossings-per-message`,
  `outcomes-reached` higher, `unschedulable-requests`, `placement-imbalance`,
  `node-hours-per-workload`) with `reference: {kind: accepted_value}` and a reference record each;
  `abstract-states` a report.
- Pattern matches are held twice: the run without search at 0 (`lte 0`), and the matches over every
  evaluated search point as an identity set by pattern and point with `no_new_items` against an
  accepted set that holds today’s node-loss matches, each item with the reason "finding of
  2026-11-27, open". A new matching point fails; fixing the finding is a tightening.
- `tools/kpis.d/40-platform.sh` runs `task sim-experiments` (with their searches; measures, so no
  `slot`) and prints every `platform.*` value per experiment, `<experiment>.<kpi>` with the command.
- `weft-sim-report@1` gets the outputs this needs that `story:kpi-sim-fields` did not declare (the
  search-point pattern matches as an identity set).

## Domain relations

No ESS entity relation beyond those `story:kpi-sim-fields` records with their citations. The
ratchet/1 relations are those of `story:kpi-repository` § Domain relations, plus:

- `platform.*` KPI -> assessment, one-to-many, one measurement per experiment — inferable
  (inferred from `quality/ratchet.yaml:52-130`, one assessment `experiment-<name>` per experiment;
  no ess/1 document declares this relation)

## Acceptance

1. Every experiment of `sim/experiments/` with a `search:` block lists every gate KPI named above in
   `search.gates` or names it in `search.maximised`:
   `ratchet::every_search_gates_every_platform_gate_kpi` passes on the tree and fails on a fixture
   experiment that drops `violations` from `gates` (`task sim-test`).
2. weft-sim refuses a `search.maximised` entry without a `reason`, one whose KPI is also in
   `gates`, and one whose KPI has no positive weight, each with a message naming the KPI (three
   weft-sim tests, `task sim-test`); `sim/experiments/duplicate-delivery.yaml` names
   `duplicate-effects` and `quota-breaches` there with a reason.
3. `quality/ratchet.yaml` has the subject `platform` and an assessment for each experiment of
   `sim/experiments/` with a measurement for each of the 21 `platform.*` KPIs:
   `ratchet::every_experiment_measures_every_platform_kpi` passes on the tree and fails on a copy
   of the file with one measurement removed (`task sim-test`).
4. Each gate KPI of each experiment is a requirement listed in the `merge` gate:
   `ratchet::every_experiment_gate_kpi_is_a_merge_requirement` passes on the tree and fails on a
   copy with one `merge` entry removed (`task sim-test`).
5. A requirement in the `merge` gate reads the pattern matches of every evaluated node-loss search
   point, and its reference record holds the `healthy-node-lost` matches found on 2026-11-27.
6. Each ratchet-role requirement of this story has a reference record under `ratchet/references/`
   with the value `task kpis` printed at the adopting commit.
7. `task kpis` prints every `platform.*` value of every experiment with its command.
8. `task check` runs no experiment: `ratchet::check_runs_no_experiment` passes (`task sim-test`).
9. The committed experiments still parse (`the_committed_experiments_parse`) and
   `ratchet::the_ratchet_file_is_consistent` of `story:kpi-repository` passes (`task sim-test`).

## Out of Scope

- Fixing the `healthy-node-lost` finding.
- New mutation points (`story:kpi-flat-searches`).
- `platform.*` objective weights beyond keeping the existing ones; the search objectives of
  `design:quality-kpis` § Search objectives first stay a design input.

## Rules for the work

- No KPI measurement in the test step; no test over 60 s; the step under 120 s.
- No Python in anything committed.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- An adversarial search and the gate rule — inferable: `design:quality-kpis` § Conventions keeps
  both uses and has each experiment say which it is. Taken: the maximised KPI is not a gate of that
  search, named in `search.maximised` with the reason; the ratchet still holds it at 0 on the run
  without search.
- How the known finding is held without a red gate — taken: an accepted set with the reason, so the
  finding stays visible and may not grow.

## Open Questions

None.
