---
format: aep.planning-md/3
id: story:kpi-simulator
kind: story
status: draft
title: weft-sim's own KPIs (test step, slowest test, throughput, debug sections, ESS versions) held in quality/ratchet.yaml and printed by task kpis
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-repository
- depends_on: story:kpi-specification
- depends_on: story:kpi-flat-searches
scope:
- confidence: inferred
  path: quality/assessors/simulator-facts@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: tools/kpis.d/50-simulator.sh
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
revision: 11
---
## Epic

Decomposes `epic:kpi-ratchet`, outcomes 1 and 4 for the subject `simulator` (weft-sim itself).
Depends on `story:kpi-repository` (task `kpis`, references, consistency test) and
`story:kpi-specification` (the plain explore assessment whose throughput this story reads).

Order: ninth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> `story:kpi-flat-searches` -> this story -> `story:kpi-ci-runs` ->
`story:kpi-docs-site`). It depends on `story:kpi-flat-searches`, the last story before it to edit
`quality/ratchet.yaml`; `story:kpi-ci-runs` and `story:kpi-docs-site` edit it after it. Of
`quality/assessors/` it writes one file, `simulator-facts@1.assessor.yaml` (new), and no other
assessor declaration.

## Outcome

A developer runs `task kpis` and reads weft-sim’s own health (test step, slowest test, test count,
explore throughput, debug sections, ESS versions in use) with commands, and
`quality/ratchet.yaml` holds each figure.

## Context

`design:quality-kpis` § weft-sim itself, baselines of 2026-11-27:

| KPI | Role | Command | Baseline |
|---|---|---|---|
| `sim.test-step-seconds` | gate 120 | `task sim-test` run phase (M4) | 3.2 s |
| `sim.slowest-test-seconds` | gate 60 | nextest output of `task sim-test` | 0.089 s |
| `sim.tests` | report | nextest | 18 |
| `sim.explore-steps-per-second` | ratchet (higher) | `steps_per_s` of the plain explore (M8) | 15594 |
| `sim.debug-sections` | gate 0 | `readelf -S` on the weft-sim binary | 0 |
| `sim.ess-revisions` | gate 1 | `ess --version` against the ESS version of the crates in `tools/sim/Cargo.lock` | probe: 2 (CLI 0.99.2, crates 0.99.5 at rev `c0b5f560`) |

- `sim.search-fitness-spread` is held by `story:kpi-flat-searches`; `sim.explore-from-seconds`
  (catalogue `no`) is already the `explore-time` measurement of `explore-from-setup`.
- The existing assessment `simulator-tests` names the assessor `command-timing@1`, which neither
  `quality/assessors/` nor the presets of github.com/beyond10x/ratchet declare (the preset is
  `timing-report@1`).
- `sim.ess-revisions` is 2 today against a gate of 1; aligning the versions is not this story.

Shape (taken here):

- `tools/kpis.d/50-simulator.sh` builds the test binary through `slot` (the shim does it for
  `cargo`), times the run phase, reads the ten slowest tests and the count, reads `steps_per_s` from
  the plain explore result `story:kpi-specification` writes, runs `readelf -S` on
  `tools/sim/target/debug/weft-sim`, and compares the ESS versions.
- A new assessor declaration `quality/assessors/simulator-facts@1.assessor.yaml` names
  `tools/kpis.d/50-simulator.sh` as its command and declares its six outputs. `simulator-tests`
  names it in place of the undeclared `command-timing@1`, and the test step time is a measurement
  beside the per-test time. No other file of `quality/assessors/` changes.
- Requirements: `sim.test-step-seconds` lte 120, `sim.slowest-test-seconds` lte 60 (the existing
  `test-within-budget`), `sim.debug-sections` at 0 in the `merge` gate; `sim.explore-steps-per-second`
  with `reference: {kind: accepted_value}`, a tolerance and an adoption rule (a sampled figure);
  `sim.ess-revisions` with `reference: {kind: accepted_value}` at 2, so it may not rise, until the
  versions agree; `sim.tests` a report.
- Tests in `tools/sim/tests/ratchet/mod.rs` (the module and helpers of `story:kpi-repository`,
  file-only, `task sim-test`): `ratchet::simulator_kpis_have_measurements` (the six ids above, a
  list in the test, each with a measurement under the subject `simulator`),
  `ratchet::simulator_gate_kpis_are_merge_requirements` (`sim.test-step-seconds`,
  `sim.slowest-test-seconds`, `sim.debug-sections`, each a requirement listed in `merge`), and
  `ratchet::check_runs_no_simulator_measurement` (the `check` and `test` tasks of `Taskfile.yml`
  with every task they call, and the files of `tools/check.d/`, name neither
  `tools/kpis.d/50-simulator.sh` nor `readelf`).

## Domain relations

None: no ESS entity is read. The ratchet/1 relations are those of `story:kpi-repository`
§ Domain relations.

## Acceptance

1. `task kpis` prints a line for each of the six `sim.*` KPIs above with its value and command.
2. `quality/ratchet.yaml` holds a measurement for each of the six, under the subject `simulator`:
   `ratchet::simulator_kpis_have_measurements` passes on the tree and fails on a copy of the file
   with one measurement removed (`task sim-test`).
3. `sim.test-step-seconds`, `sim.slowest-test-seconds` and `sim.debug-sections` are requirements
   listed in the `merge` gate: `ratchet::simulator_gate_kpis_are_merge_requirements` passes
   (`task sim-test`).
4. Every assessor a `simulator` assessment names is declared in `quality/assessors/`
   (`simulator-facts@1` for `simulator-tests`): `ratchet::the_ratchet_file_is_consistent` of
   `story:kpi-repository` checks it and passes (`task sim-test`).
5. `sim.explore-steps-per-second` and `sim.ess-revisions` have reference records under
   `ratchet/references/` with the values `task kpis` printed at the adopting commit.
6. `task check` runs none of these measurements: `ratchet::check_runs_no_simulator_measurement`
   passes on the tree and fails on a copy of `Taskfile.yml` whose `check` task runs
   `tools/kpis.d/50-simulator.sh` (`task sim-test`).

## Out of Scope

- Making the ESS CLI and the crates of weft-sim agree (the gate at 1).
- `sim.explore-from-seconds` beyond the existing `explore-time` measurement.
- `sim.search-fitness-spread` (`story:kpi-flat-searches`).

## Rules for the work

- No KPI measurement in the test step; no test over 60 s; the step under 120 s.
- A build or test job in the pool ends within 10 minutes; the hook measures and so starts without
  `slot`, and its build goes through the pool.
- No Python in anything committed.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- `sim.ess-revisions` is a gate at 1 in the catalogue and measures 2 — taken: held as a ratchet that
  may not rise, since a gate at 1 would fail on the day it lands; the gate follows the change that
  aligns the versions. Reversible.

## Open Questions

None.
