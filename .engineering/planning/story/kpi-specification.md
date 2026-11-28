---
format: aep.planning-md/3
id: story:kpi-specification
kind: story
status: draft
title: Specification KPIs (validation, markers, synthesis, explore coverage and gaps) held in quality/ratchet.yaml and printed by task kpis
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-repository
- depends_on: story:kpi-sim-fields
scope:
- confidence: inferred
  path: quality/assessors/ess-synthesis@1.assessor.yaml
- confidence: inferred
  path: quality/assessors/repo-facts@1.assessor.yaml
- confidence: cited
  path: quality/assessors/weft-sim-report@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: tools/kpis.d/30-specification.sh
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
revision: 12
---
## Epic

Decomposes `epic:kpi-ratchet`, outcomes 1 and 4 for the subject `specification`. Depends on
`story:kpi-repository` (the task `kpis`, its hook directory, the `repo-facts@1` assessor and the
reference records).

Order: sixth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> this story -> `story:kpi-platform-gates` ->
`story:kpi-flat-searches` -> `story:kpi-simulator` -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). It depends on `story:kpi-sim-fields`, which edits
`quality/assessors/weft-sim-report@1.assessor.yaml` before it; `story:kpi-platform-gates`,
`story:kpi-flat-searches`, `story:kpi-simulator`, `story:kpi-ci-runs` and `story:kpi-docs-site`
edit `quality/ratchet.yaml` and `ratchet/references/` after it.

## Outcome

A developer runs `task kpis` and reads the 21 measured `spec.*` KPIs of `design:quality-kpis` § 1
with their commands, and `quality/ratchet.yaml` holds each of them, so a change that loses
coverage, adds a refusal or breaks validation shows as a number against its reference.

## Context

`quality/ratchet.yaml` holds five `spec.*` KPIs today, all from the assessment
`explore-from-setup` (`design:quality-kpis` § Mapping onto ratchet/1). The other sixteen measured
ones, with the 2026-11-27 baselines at `58172a7`:

| KPI | Role | Command (catalogue ref) | Baseline |
|---|---|---|---|
| `spec.validate-errors` | gate 0 | `slot ess specify validate --path ess --strict-requires` (M2) | 0 |
| `spec.validate-seconds` | ratchet, bound 120 | M2 timed | 3.6 s |
| `spec.layer-violations` | gate 0 | `tools/check-layers.sh` (M3) | 0 |
| `spec.stamped-time-violations` | gate 0 | `tools/check-stamped-time.sh` (M3) | 0 |
| `spec.protocol-validate-errors` | gate 0 | `slot ess specify protocol validate` per file of `protocols/` (M11) | 0 of 2 |
| `spec.open-markers` | ratchet | `grep -rn "# OPEN:" ess` count | 404 |
| `spec.unmapped-markers` | ratchet | `grep -rn UNMAPPED ess` count | 53 |
| `spec.rust-synthesis-refusals` | ratchet | `slot ess generate synthesize --path ess --target rust --out target/kpis/synth --format json`, causes of `ess-target-failure/1` (M5) | 8 |
| `spec.obligations` | report | M5 | 285 |
| `spec.states-reached` | ratchet (higher) | explore (M8), explore from setup (M9) | 150, 186 |
| `spec.transitions-reached` | ratchet (higher) | M8, M9 | 89, 106 |
| `spec.commands-never-accepted` | ratchet | M8, M9, `.gaps.commands_never_accepted` | 228, 163 |
| `spec.commands-blocked-by` | ratchet | M8, M9, `.gaps.blocked_by_count` | 4, 11 |
| `spec.entities-never-held` | ratchet | M8, M9, `.gaps.entities_never_held` | 56, 38 |
| `spec.branch-points` | report | M8, M9 | 3296, 232 |
| `spec.hardening-passes` | ratchet (higher), target 3 | `ls docs/hardening/pass-*.md` count | 2 |

Shape:

- `tools/kpis.d/30-specification.sh` runs the commands above and the two explore runs that exist
  as tasks (`task sim-explore`, 2048 walks, about 41 s; `task sim-explore-from`, 1024 walks, the
  run of the existing assessment, never timed at 1024 walks: this story measures it and records the
  time). The explore runs measure and so start without `slot`; the `ess` calls inside the hook take
  the `slot` prefix. A time read through `slot` is the run time of the job, not the wait in the
  queue.
- A new assessment `explore` beside `explore-from-setup`, both with the `weft-sim-report@1`
  assessor. The assessor declares the outputs the new measurements need: `states_reached`,
  `transitions_reached`, `commands_never_accepted` (identity set by command),
  `blocked_by_count`, `entities_never_held` (identity set by entity), `branch_points`.
- A new assessor declaration `quality/assessors/ess-synthesis@1.assessor.yaml` for
  `ess-target-failure/1` and the synthesis plan (refusal causes as an identity set, obligations as a
  number). Exit status 1 with an `ess-target-failure/1` document is a measurement, not an error of
  the hook.
- The validation, layer, stamped-time and marker counts are outputs of `repo-facts@1`.
- Tests in `cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs` (the module and helpers of `story:kpi-repository`,
  file-only, `task sim-test`): `ratchet::specification_kpis_have_measurements` (the 21 ids of the
  table above and § Mapping, a list in the test, each with a measurement under the subject
  `specification`), `ratchet::specification_gate_kpis_are_merge_requirements` (the six gate ids,
  each a requirement listed in `merge`), and `ratchet::check_runs_no_specification_measurement`
  (the `check` and `test` tasks of `Taskfile.yml` with every task they call, and the files of
  `tools/check.d/`, name none of `sim-explore`, `weft-sim explore`, `ess generate synthesize` and
  `tools/kpis.d/30-specification.sh`).
- Requirements: the gate KPIs in the `merge` gate; the ratchet KPIs with
  `reference: {kind: accepted_value}` (identity sets with `no_new_items` or `no_lost_items`,
  counts with the direction of the catalogue) and a reference record each; report KPIs as
  measurements without a requirement.

## Domain relations

No ESS entity relation is read: the explore figures are counts weft-sim already writes. The ratchet/1
relations are those recorded with their citations in `story:kpi-repository` § Domain relations.

## Acceptance

1. `task kpis` prints a line for each of the 21 measured `spec.*` KPIs of `design:quality-kpis` § 1,
   from the plain explore and from the explore from setup where the catalogue gives both, each
   with its value and command.
2. `quality/ratchet.yaml` holds a measurement for each of those KPIs:
   `ratchet::specification_kpis_have_measurements` passes on the tree and fails on a copy of the
   file with one measurement removed (`task sim-test`).
3. The six gate KPIs are requirements listed in the `merge` gate:
   `ratchet::specification_gate_kpis_are_merge_requirements` passes (`task sim-test`).
4. Each ratchet-role requirement of this story has a reference record under `ratchet/references/`
   with the value `task kpis` printed at the adopting commit, the 1024-walk explore-from-setup
   figures included.
   This includes the two requirements `no-lost-outcomes` and `no-new-unsupported-answers`, which
   `story:kpi-repository` lists in `RECORDS_PENDING` of `cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs`: this story
   writes their records and removes both from that list, which is then empty.
5. The `weft-sim-report@1` and `ess-synthesis@1` declarations declare every output a measurement
   of this story names: `ratchet::the_ratchet_file_is_consistent` of `story:kpi-repository`
   passes (`task sim-test`).
6. `task check` runs no explore, no synthesis and no other measurement of this story:
   `ratchet::check_runs_no_specification_measurement` passes on the tree and fails on a copy of
   `Taskfile.yml` whose `check` task calls `task: sim-explore` (`task sim-test`).

## Out of Scope

- `spec.branch-points-without-guard`, `spec.unowned-or-uncallable`,
  `spec.conformance-failed-interpreted` and `spec.suite-synthesis-seconds` (catalogue `no`).
- Lowering a marker count or a refusal count.
- An assessor binary that turns `result.json` into ratchet evidence.

## Rules for the work

- No KPI measurement in the test step; no test over 60 s; the step under 120 s.
- A job in the pool ends within 10 minutes; synthesis goes through `slot`, one run at a time.
- No Python in anything committed: POSIX sh or Rust.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- Which explore run carries the coverage KPIs — taken: both, as two assessments, since the
  catalogue gives a baseline for each (M8, M9) and they answer different questions (from an empty
  store, from a full setup).

## Open Questions

None.
