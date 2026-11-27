---
format: aep.planning-md/3
id: story:kpi-ci-runs
kind: story
status: draft
title: CI and docs-build KPIs from the forge's run records held in quality/ratchet.yaml and printed by task kpis
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
- depends_on: story:kpi-repository
- depends_on: story:kpi-simulator
scope:
- confidence: inferred
  path: quality/assessors/ci-runs@1.assessor.yaml
- confidence: inferred
  path: quality/assessors/repo-facts@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: tools/kpis.d/20-ci.sh
- confidence: inferred
  path: tools/sim/tests/ratchet.rs
revision: 8
---
## Epic

Decomposes `epic:kpi-ratchet`, outcomes 1 and 4 for the subject `ci` and the two docs-build
figures that come from the same run records. Depends on `story:kpi-repository` (the task `kpis`,
its hook directory, the reference records and the subject `docs-site`).

Order: tenth link of the one chain of `depends_on` edges over the shared weft-sim and ratchet
surfaces (`story:kpi-repository` -> `story:anim-search-record` -> `story:anim-search-player` ->
`story:anim-point-replay` -> `story:kpi-sim-fields` -> `story:kpi-specification` ->
`story:kpi-platform-gates` -> `story:kpi-flat-searches` -> `story:kpi-simulator` -> this story ->
`story:kpi-docs-site`). It depends on `story:kpi-simulator`; every other story that edits
`quality/ratchet.yaml`, `quality/assessors/repo-facts@1.assessor.yaml` or `ratchet/references/`
lands before it, except `story:kpi-docs-site`, which depends on it.

## Outcome

A developer runs `task kpis` and reads the CI figures of `design:quality-kpis` § 5 and the docs
build figures, each with the command that read it, and `quality/ratchet.yaml` holds them.

## Context

- Measured (`yes`) in `design:quality-kpis`, M10 (`gh run list --repo beyond10x/weft --limit 200`,
  then `gh run view <id> --json jobs` for each completed run; 21 runs, all of 2026-11-27):
  - `ci.gate-seconds-warm` (46, 46 s), `ci.gate-seconds-cold` (352, 816 s),
    `ci.gate-job-seconds` (report), `ci.failed-runs` per workflow (Gate 0 of 5, Documentation
    validation 0 of 3, Shared source gates 1 of 6, Documentation site 0 of 1);
  - `docs.build-failures` (0 of 3, gate) and `docs.build-seconds` (34, 55, 70 s) from the
    "Build the Docusaurus site" step of `pages.yml`.
- Measured locally: `ci.planning-only-misclassified` (`tools/test-planning-only.sh`, 0, gate) and
  `ci.unpinned-actions` (`uses:` lines not pinned to a 40-hex commit, 0, gate).
- `ci.red-after-green-local` is `no` in the catalogue (no record of the local gate per commit) and
  is not part of this story.
- The run records come over the network. `~/beyond10x/AGENTS.md` § Tests stay fast: no network in
  the test step. The reading runs only in `task kpis` (a measuring lane), never in `task check`.

Shape:

- `tools/kpis.d/20-ci.sh` (POSIX sh, `gh` and `jq`) reads the last 200 runs, keeps the completed
  ones, and prints the eight figures; with `gh` missing or not signed in it prints `unavailable`
  and the reason for the six forge figures and still prints the two local ones.
- A new assessor declaration `quality/assessors/ci-runs@1.assessor.yaml` names that command and
  its outputs (assessment `ci-runs`). The two local figures are a second assessment `ci-files`
  under the `repo-facts@1` assessor of `story:kpi-repository`, so no forge failure hides them.
- `quality/ratchet.yaml` gains the subject `ci`; the two `docs.build-*` assessments go under the
  subject `docs-site`, which `story:kpi-repository` adds and owns (this story does not add it).
  It gains the assessments and requirements: the gate KPIs
  (`ci.planning-only-misclassified`, `ci.unpinned-actions`, `docs.build-failures`) in the `merge`
  gate; the ratchet KPIs (`ci.gate-seconds-warm` with its bound 120, `ci.gate-seconds-cold`,
  `ci.failed-runs`, `docs.build-seconds`) with `reference: {kind: accepted_value}` and a reference
  record each. `ci.gate-job-seconds` is a report: a measurement with no requirement.
- A sampled time with an accepted value needs a tolerance and an adoption rule in ratchet/1
  (github.com/beyond10x/ratchet `docs/design.md` § 3); the story names both for each time.
- Tests in `tools/sim/tests/ratchet.rs` (the module and helpers of `story:kpi-repository`,
  file-only, `task sim-test`): `ratchet::ci_kpis_have_measurements` (the eight ids above, a list
  in the test, each with a measurement under `ci` or `docs-site`),
  `ratchet::ci_gate_kpis_are_merge_requirements` (the three gate ids, each a requirement listed
  in `merge`), and `ratchet::gate_reads_no_run_records`. The last reads
  `.github/workflows/gate.yml`, the `check` and `test` tasks of `Taskfile.yml` with every task
  they call, and the files of `tools/check.d/`, and fails when one holds `gh run`, `gh api`,
  `api.github.com`, `curl`, `wget` or `tools/kpis.d/20-ci.sh`. The gate job's install of `ess`
  with `gh release download` (`.github/workflows/gate.yml:86`) is a download of a pinned release,
  not a read of run records, and stays.

## Domain relations

No ESS entity is read. The ratchet/1 relations (assessment -> subject, requirement ->
measurement, `merge` -> requirement, reference record -> requirement) are those recorded with
their citations in `story:kpi-repository` § Domain relations; this story adds no other.

## Acceptance

1. `task kpis` prints a line for each of the six `ci.*` KPIs and the two `docs.build-*` KPIs named
   above, each with its value and the command that produced it, or `unavailable` with the reason
   for a forge figure when `gh` cannot read the runs.
2. `quality/ratchet.yaml` holds a measurement for each of those eight KPIs:
   `ratchet::ci_kpis_have_measurements` passes on the tree and fails on a copy of the file with
   one measurement removed (`task sim-test`).
3. `ci.planning-only-misclassified`, `ci.unpinned-actions` and `docs.build-failures` are
   requirements listed in the `merge` gate: `ratchet::ci_gate_kpis_are_merge_requirements` passes
   (`task sim-test`).
4. Each ratchet-role requirement of this story has a reference record under `ratchet/references/`
   with the value `task kpis` printed at the adopting commit, and each sampled one names its
   tolerance and adoption.
5. No file of the gate reads run records or calls the network for these figures:
   `ratchet::gate_reads_no_run_records` passes on the tree (scope: `.github/workflows/gate.yml`,
   the `check` and `test` tasks of `Taskfile.yml` with the tasks they call, `tools/check.d/`) and
   fails on a copy of `Taskfile.yml` whose `check` task runs `gh run list` (`task sim-test`).

## Out of Scope

- `ci.red-after-green-local` (needs a record of the local gate per commit).
- A scheduled CI job that runs `task kpis`.
- Changing a workflow, a ruleset or branch protection.

## Rules for the work

- No KPI measurement and no network in the test step; no test over 60 s; the step under 120 s.
- No Python in anything committed: POSIX sh or Rust.
- The repository is public: no person, host, session or private repository name in any file,
  and no token or credential written anywhere.

## Ambiguities

- Window of runs — taken: the last 200 runs, as M10 did; the figures rest on few runs today and the
  window is a parameter of the hook.

## Open Questions

None.
