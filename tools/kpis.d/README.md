# KPI hooks

`task kpis` (`tools/kpis.sh`) runs every executable file of this directory in name order
(`tools/run-hooks.sh`); the first that fails ends the run. Each hook measures the KPIs of one
subject of the catalogue (`design:quality-kpis`) and prints one line per KPI: its id, its value
with the unit, and the command that measured it. `quality/ratchet.yaml` holds the same KPIs, and
the hook is the command of the assessor declaration in `quality/assessors/` that reads them.

Contract of a hook:

- POSIX sh or a Rust binary; no Python.
- Runs from the repository root, without arguments, and writes only under `target/kpis/`.
- A KPI that cannot be measured on this machine prints `unavailable` and the reason, never a
  guessed value. A hook exits 0 when it printed a line for each of its KPIs.
- It measures, so `task kpis` starts without the `slot` prefix (the organization's rule "Scripts
  that do not build stay off slot"). A command inside it that builds or tests (`task check`,
  `tools/test-step.sh`, the gate self-test) gets the prefix there, as `slot --here`, so the values
  come from this host. A time read through `slot` is the run time of the job, not its wait in the
  queue: the hook takes the time inside the job.
- It is never part of `task check` or `task test`, and no file of `tools/check.d/`,
  `tools/test-setup.d/` or `tools/test-teardown.d/` calls it (test `ratchet::check_runs_no_kpi_hook`
  of `tools/sim/tests/ratchet/mod.rs`).
- A pool job ends within 10 minutes; one that needs longer names its time with
  `slot --timeout`, at most 30 minutes.

Hooks: `10-repository.sh`, the repository KPIs (`repo.*`), the command of
`quality/assessors/repo-facts@1.assessor.yaml`.
