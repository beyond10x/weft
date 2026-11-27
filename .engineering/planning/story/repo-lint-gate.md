---
format: aep.planning-md/3
id: story:repo-lint-gate
kind: story
status: draft
title: The gate runs cargo fmt --check and clippy -D warnings on both workspaces
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
scope:
- confidence: cited
  path: tools/check.d
- confidence: cited
  path: tools/sim/src
revision: 2
---
## Context

No step of the gate runs `cargo fmt --check` or `cargo clippy` (`tools/check.d/`, `Taskfile.yml`
and `.github/workflows/` name neither, read 2026-11-27). Each story's implementor ran both on the
crates it touched and reported exit 0; nothing holds them on the rest.

Found by `story:anim-search-player` on main 290862e, in code that story did not touch:

- `tools/sim`: `cargo fmt --check` exits 1 (392 hunks in 14 files);
- `tools/sim`: `cargo clippy --all-targets -- -D warnings` exits 101: `coverage.rs:310` and
  `:336`, `report.rs:206` (unnecessary_sort_by), `gen.rs:23` (type_complexity), `main.rs:94`
  (ptr_arg).

## Scope

- `tools/sim/src/` (formatting and the five clippy findings)
- `tools/check.d/` (one hook that runs both on the root workspace and on `tools/sim`)
- root workspace crates under `product/cloud/crates/`, if the hook finds anything there

## Acceptance

1. `cargo fmt --check` exits 0 in the root workspace and in `tools/sim`.
2. `cargo clippy --all-targets -- -D warnings` exits 0 in the root workspace and in `tools/sim`,
   with no `allow` added for the five lints above.
3. `slot task check` runs both through a hook under `tools/check.d/`, and fails when a
   formatting change or a clippy warning is put back into either workspace (shown with one
   red run of each).
4. The hook's wall time is named in the report; the gate's test step stays within its budget.

## Rules for this work

- Builds and tests run through `slot`; a pool job ends within 10 minutes unless it names its
  time (`SLOT_TIMEOUT=30m`, at most 30 minutes).
- No test runs over 60 s; the test step ends within 120 s.
- No Python in anything committed.
