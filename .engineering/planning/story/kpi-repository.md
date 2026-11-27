---
format: aep.planning-md/3
id: story:kpi-repository
kind: story
status: implemented
title: task kpis prints every repository KPI with its command, and quality/ratchet.yaml holds them with accepted references and a fast consistency test
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: quality/assessors/repo-facts@1.assessor.yaml
- confidence: cited
  path: quality/ratchet.yaml
- confidence: inferred
  path: ratchet/references
- confidence: inferred
  path: tools/kpis.d/10-repository.sh
- confidence: inferred
  path: tools/kpis.d/README.md
- confidence: inferred
  path: tools/kpis.sh
- confidence: cited
  path: tools/sim/tests/main.rs
- confidence: cited
  path: tools/sim/tests/ratchet/mod.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T13:14:20Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"approval":1,"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-27T13:14:20Z", actor: "human:operator", revision: 9, decided_on: {"recorded":{"approval":1,"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-11-27T15:01:08Z", actor: "human:operator", revision: 13, decided_on: {"recorded":{"test_result":1,"approval":1,"review_outcome":2}}}
---
## Epic

Decomposes `epic:kpi-ratchet`, outcomes 1 and 4 for the subject `repository`, and lays the frame
every other `kpi-` story adds to: the task `kpis`, its hook directory and the place of the accepted
references. First of the epic.

Order: first of the one chain of `depends_on` edges over the shared weft-sim and ratchet surfaces
(`tools/sim/src/simulate.rs`, `tools/sim/src/page.html`, `tools/sim/tests/main.rs`,
`quality/ratchet.yaml`, `quality/assessors/`, `ratchet/references/`): `story:kpi-repository` ->
`story:anim-search-record` -> `story:anim-search-player` -> `story:anim-point-replay` ->
`story:kpi-sim-fields` -> `story:kpi-specification` -> `story:kpi-platform-gates` ->
`story:kpi-flat-searches` -> `story:kpi-simulator` -> `story:kpi-ci-runs` -> `story:kpi-docs-site`.
This story adds the `mod ratchet;` line to `tools/sim/tests/main.rs` before every later story of
the chain edits that file. `story:docs-claims-check` depends on this story too: both edit
`Taskfile.yml` and `AGENTS.md`.

## Outcome

A developer runs `task kpis` and reads every repository KPI of `design:quality-kpis` § 4 with the
command that measured it, and `quality/ratchet.yaml` holds the same KPIs, so a later `ratchet`
run and today’s printout read one set of values.

## Context

- `design:quality-kpis` § 4 lists ten `repo.*` KPIs; nine are measured (`yes`), one is a probe
  (`repo.agents-md-dead-paths`, held by `story:kpi-docs-site`, which reads the docs checker).
  Baselines of 2026-11-27 at `58172a7`: gate 20 s, test step 3 s, slowest test 0.008 s, 0 of 30
  budget cases failed, 1 test binary per crate, 0 Python files, 0 toolchain pins that disagree,
  0 cache bypass lines, 0 planning defects.
- Neither `ratchet` nor any assessor binary is on `PATH` yet; the file is written ahead of
  `ratchet validate` (default 3 of `design:simulation-and-kpis`). Until then `task kpis` prints the
  values; this story does not make `ratchet` run.
- Accepted values are not in `quality/ratchet.yaml`. The ratchet/1 format keeps them in one record
  per requirement, `ratchet/references/<requirement>.json`, format `ratchet-reference/1`
  (github.com/beyond10x/ratchet, `docs/design.md` § 5 "Reference records"). This story writes the
  first such records by hand, `last_change.kind: first_adoption`, because `ratchet reference
  adopt` does not run yet.

Shape (taken here; later `kpi-` stories add to it without editing the task):

- `task kpis` runs `tools/kpis.sh`, which runs every executable file of `tools/kpis.d/` in name
  order (the `tools/run-hooks.sh` form) and prints one line per KPI: id, value with unit, command.
  A KPI that cannot be measured on this machine prints `unavailable` and the reason, never a
  guessed value.
- `tools/kpis.d/README.md` states the hook contract: POSIX sh or Rust, runs from the repository
  root, writes only under `target/kpis/`, measures and so starts without the `slot` prefix; a
  command inside it that builds or tests (`task check`, `task test`, the gate self-test) gets the
  `slot` prefix there (`~/beyond10x/AGENTS.md` § Scripts that do not build stay off slot). A time
  read through `slot` is the run time of the job, not its wait in the queue.
- `tools/kpis.d/10-repository.sh` measures § 4 with the commands the catalogue names (M1, M3, M4,
  M12) and is the command of a new assessor declaration
  `quality/assessors/repo-facts@1.assessor.yaml`.
- `quality/ratchet.yaml` gains the subject `repository` and one assessment `repository-facts`
  with a measurement per KPI. It also gains the subject `docs-site`, with no assessment of its own
  here: this story is its sole owner, so `story:kpi-ci-runs` (the `docs.build-*` KPIs) and
  `story:kpi-docs-site` (the KPIs measured on the tree), which both come after it, only add
  assessments under it. Gate KPIs (`repo.test-step-seconds` lte 120,
  `repo.slowest-test-seconds` lte 60, `repo.test-binaries-per-crate` lte 1, and at 0:
  `repo.gate-budget-cases-failed`, `repo.python-files`, `repo.toolchain-pins-disagree`,
  `repo.cache-bypass`, `repo.plan-validate-defects`) get a requirement in the `merge` gate.
  The ratchet KPI `repo.gate-seconds` (bound 600) gets a requirement with
  `reference: {kind: accepted_value}` and a reference record holding the baseline this story
  measures.
- A consistency test, cheap and file-only: every requirement names an assessment and measurement
  that exist, every `merge` entry names a requirement, every measurement of an assessor declared in
  `quality/assessors/` names an output it declares, and every `accepted_value` requirement has its
  record under `ratchet/references/`. It reads files and measures nothing. It lives in weft-sim’s
  test binary as a module (`tools/sim/tests/ratchet/mod.rs`, one `mod` line in
  `tools/sim/tests/main.rs`), so it runs in `task sim-test` inside `task check`. Its name is
  `ratchet::the_ratchet_file_is_consistent`.
- Coverage tests in the same module, with helpers every later `kpi-` story calls with its own
  list (each later story adds its own named tests, it changes no test of this story):
  - `ratchet::repository_kpis_have_measurements`: the nine measured `repo.*` ids of
    `design:quality-kpis` § 4, a list in the test, each has a measurement in an assessment under
    the subject `repository`; the test fails naming each id without one.
  - `ratchet::repository_gate_kpis_are_merge_requirements`: the eight gate ids among them, each
    is a requirement listed in the `merge` gate; the test fails naming each id that is not.
  - `ratchet::check_runs_no_kpi_hook`: reads the `check` and `test` tasks of `Taskfile.yml` with
    every task they call, and the files of `tools/check.d/`, and fails when one names
    `tools/kpis.sh`, `tools/kpis.d/` or `task kpis`. The helper that reads that text takes the
    list of names, so a later story holds its own measuring commands out of the gate with a test
    of its own.

## Domain relations

No ESS entity is read. The relations between the ratchet/1 nouns this story writes:

- assessment -> subject, many-to-one, the subject outlives its assessments — inferable (inferred
  from `quality/ratchet.yaml:14`, `subject: specification`; no ess/1 document declares this
  relation)
- requirement -> (assessment, measurement), many-to-one — inferable (inferred from
  `quality/ratchet.yaml:136-139`; no ess/1 document declares this relation)
- gate `merge` -> requirement, one-to-many, the gate lists requirements it does not own —
  inferable (inferred from `quality/ratchet.yaml:184-197`; no ess/1 document declares this
  relation)
- reference record -> requirement, one-to-one, zero for a requirement with a fixed `value` —
  inferable (inferred from github.com/beyond10x/ratchet `docs/design.md:597`, "one JSON file per
  requirement in the tree"; no ess/1 document declares this relation)

## Acceptance

1. `task kpis` prints one line for each of the nine measured `repo.*` KPIs of
   `design:quality-kpis` § 4, each with its value, its unit and the command that produced it.
2. `quality/ratchet.yaml` has the subject `repository` and a measurement for each of those nine
   KPIs: `ratchet::repository_kpis_have_measurements` passes on the tree and fails on a copy of
   the file with one of them removed (`task sim-test`).
3. Every gate KPI among them is a requirement listed in the `merge` gate:
   `ratchet::repository_gate_kpis_are_merge_requirements` passes on the tree and fails on a copy
   with one `merge` entry removed (`task sim-test`).
4. `quality/ratchet.yaml` has the subject `docs-site`, and `ratchet::the_ratchet_file_is_consistent`
   passes with it (`task sim-test`).
5. Each ratchet-role `repo.*` requirement has a `ratchet/references/<requirement>.json` record
   whose value is the one `task kpis` printed at the adopting commit.
6. `ratchet::the_ratchet_file_is_consistent` fails on a fixture where a requirement names a
   missing measurement, a `merge` entry names a missing requirement, or an `accepted_value`
   requirement has no record, and passes on the committed files; it runs in `task sim-test` in
   under 1 s.
7. `task check` runs no `tools/kpis.d/` hook and no KPI measurement:
   `ratchet::check_runs_no_kpi_hook` passes on the tree and fails on a copy of `Taskfile.yml`
   whose `check` task calls `task: kpis` (`task sim-test`).
8. `slot task check` exits 0 with this story's tests in it. A pass shows both bounds held: nextest
   kills a test at 60 s (`slow-timeout` with `terminate-after = 2` in
   `tools/sim/.config/nextest.toml`) and the `sim-test` task runs under `timeout 120`.

## Out of Scope

- Making `ratchet` or an assessor binary run, and evaluating the `merge` gate with it.
- Every other subject but `docs-site` (the other `kpi-` stories), the assessments under
  `docs-site`, and `repo.agents-md-dead-paths`
  (`story:kpi-docs-site`).
- Fixing a KPI that is off its bound.

## Rules for the work

- No KPI measurement in the test step; no test over 60 s; the step under 120 s
  (`~/beyond10x/AGENTS.md` § Tests stay fast). Time the new test and report it.
- A build or test job in the pool ends within 10 minutes.
- No Python in anything committed: POSIX sh or Rust.
- The repository is public: no person, host, session or private repository name in any file.

## Ambiguities

- Where the consistency test lives — inferable: the only crate that already parses YAML is
  weft-sim (`tools/sim/src/experiment.rs:596`, `ess_yaml::from_str`), and its test binary already
  reads committed files (`tools/sim/tests/main.rs:113`, `the_committed_experiments_parse`). Taken:
  a module of that binary.
- `authorized_by` of a first adoption — taken: it names `epic:kpi-ratchet`, the owner’s order of
  2026-11-27 that the baselines become the references.

## Open Questions

None.
