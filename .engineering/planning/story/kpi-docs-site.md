---
format: aep.planning-md/3
id: story:kpi-docs-site
kind: story
status: draft
title: Docs-site and AGENTS.md path KPIs held in quality/ratchet.yaml and printed by task kpis, read from the docs checker
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-repository
- depends_on: story:docs-claims-check
- depends_on: story:kpi-ci-runs
scope:
- confidence: inferred
  path: quality/assessors/docs-check@1.assessor.yaml
- confidence: inferred
  path: quality/assessors/repo-facts@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: tools/kpis.d/60-docs-site.sh
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
revision: 11
---
## Epic

Decomposes `epic:kpi-ratchet`, outcomes 1 and 4 for the subject `docs-site` (the KPIs measured on
the tree) and the probe `repo.agents-md-dead-paths`. Depends on `story:kpi-repository` (task
`kpis`, references) and on `story:docs-claims-check` of `epic:docs-fidelity`, whose checks measure
three of the four KPIs. The docs build figures from CI runs are in `story:kpi-ci-runs`.

Order: last link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> `story:kpi-flat-searches` -> `story:kpi-simulator` ->
`story:kpi-ci-runs` -> this story). It depends on `story:kpi-ci-runs`, so every other `kpi-`
story that edits `quality/ratchet.yaml`, `quality/assessors/repo-facts@1.assessor.yaml` or
`ratchet/references/` lands before it. `story:anim-docs-published-run` depends on this story and
adds its experiment's assessment after it.

## Outcome

A developer runs `task kpis` and reads the docs-site figures measured on the tree with their
commands, and `quality/ratchet.yaml` holds them, reading the same checks the docs build runs, so
the ratchet and the docs check cannot disagree on a count.

## Context

`design:quality-kpis` § Docs site and § Repository, 2026-11-27:

| KPI | Role | Measured by | Today |
|---|---|---|---|
| `docs.status-age-days` | ratchet, bound 7 | `jq -r .asOf website/data/status.json` against `git log -1 --format=%cs` | 0 |
| `docs.dead-repo-links` | gate 0 | probe; the hook of `story:docs-claims-check` | 0 |
| `docs.planned-without-banner` | gate 0 | probe; the hook of `story:docs-claims-check` | 0 |
| `repo.agents-md-dead-paths` | gate 0 | probe; the hook of `story:docs-claims-check` | 2, fixed by that story to 0 |

- `story:docs-claims-check` adds `tools/docs-check.d/`, one POSIX sh hook per KPI that prints
  `<kpi-id> <count>` as its last line, and `tools/check-docs.sh`; it leaves the ratchet assessment to
  this epic. This story reads those hooks and does not write a second probe.
- `docs.status-claims-false` and `docs.commands-refused` are `no` in the catalogue (their checkers
  are stories of `epic:docs-fidelity`) and join the ratchet when those exist.

Shape (taken here):

- `tools/kpis.d/60-docs-site.sh` runs `tools/check-docs.sh` (or each hook) and takes the last line of
  each hook, computes `docs.status-age-days`, and prints the four KPIs with their commands. A hook
  that exits 1 because its count is above 0 is a measurement, not an error of the hook.
- A new assessor declaration `quality/assessors/docs-check@1.assessor.yaml` names the docs check
  command and its outputs; `docs.status-age-days` is an output of `repo-facts@1`.
- `quality/ratchet.yaml`: the assessments under the subject `docs-site`, which
  `story:kpi-repository` adds and owns (this story does not add it), the three gate KPIs as
  requirements in the `merge` gate, `docs.status-age-days` with its bound 7 and a reference record.
- Tests in `cloudspec@2ed72c2:crates/cloudspec-sim/tests/ratchet/mod.rs` (the module and helpers of `story:kpi-repository`,
  file-only, `task sim-test`): `ratchet::docs_site_kpis_have_measurements` (the four ids above, a
  list in the test, each with a measurement under `docs-site` or `repository`) and
  `ratchet::docs_site_gate_kpis_are_merge_requirements` (the three gate ids, each a requirement
  listed in `merge`).

## Domain relations

None: no ESS entity is read. The ratchet/1 relations are those of `story:kpi-repository`
§ Domain relations.

## Acceptance

1. `task kpis` prints `docs.status-age-days`, `docs.dead-repo-links`,
   `docs.planned-without-banner` and `repo.agents-md-dead-paths` with their values and commands,
   the last three taken from the hooks of `tools/docs-check.d/`.
2. `quality/ratchet.yaml` holds a measurement for each of the four under the subjects `docs-site`
   and `repository`: `ratchet::docs_site_kpis_have_measurements` passes on the tree and fails on
   a copy of the file with one measurement removed (`task sim-test`).
3. The three gate KPIs are requirements listed in the `merge` gate:
   `ratchet::docs_site_gate_kpis_are_merge_requirements` passes (`task sim-test`).
4. `docs.status-age-days` has a reference record under `ratchet/references/` with the value
   `task kpis` printed at the adopting commit.
5. `ratchet::the_ratchet_file_is_consistent` of `story:kpi-repository` passes on the committed
   files (`task sim-test`).
6. `tools/kpis.d/60-docs-site.sh` calls `tools/check-docs.sh` (`grep -n tools/check-docs.sh
   tools/kpis.d/60-docs-site.sh` prints the line), and no hook of `tools/kpis.d/` carries a pattern
   of the three docs checks (`grep -nE 'beyond10x/weft/(tree|blob)|caution\[Planned\]'
   tools/kpis.d/*.sh` prints nothing).

## Out of Scope

- The docs checks themselves (`story:docs-claims-check`) and the fix of the two `AGENTS.md`
  paths.
- `docs.status-claims-false`, `docs.commands-refused` (catalogue `no`), and the docs build
  figures from CI runs (`story:kpi-ci-runs`).

## Rules for the work

- No KPI measurement in the test step; no test over 60 s; the step under 120 s.
- No Python in anything committed.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

None open: the measuring command of the three probes is settled by `story:docs-claims-check`
§ Shape (each hook prints `<kpi-id> <count>`).

## Open Questions

None.
