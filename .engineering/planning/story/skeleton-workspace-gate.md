---
format: aep.planning-md/3
id: story:skeleton-workspace-gate
kind: story
status: active
title: Root Cargo workspace, pinned toolchain and a nextest gate with a 60 s kill and a 120 s step
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
scope:
- confidence: cited
  path: .config/nextest.toml
- confidence: cited
  path: .github/workflows/gate.yml
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/weft-testkit
- confidence: cited
  path: rust-toolchain.toml
- confidence: cited
  path: tests/fixtures/generated-probe
- confidence: cited
  path: tools/budget-fixture
- confidence: cited
  path: tools/check.d/README.md
- confidence: cited
  path: tools/run-hooks.sh
- confidence: cited
  path: tools/slow.d/README.md
- confidence: cited
  path: tools/test-gate-budget.sh
- confidence: cited
  path: tools/test-setup.d/README.md
- confidence: cited
  path: tools/test-step.sh
- confidence: cited
  path: tools/test-teardown.d/README.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T09:54:36Z", actor: "human:operator", revision: 10, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T09:54:36Z", actor: "human:operator", revision: 11, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 1. First of the epic: every other story of it builds
in this workspace and is held by this gate. Blocked by no ESS gap.

## Context

weft has no implementation code yet. `tools/sim` (weft-sim) is a Cargo project of its own with
`rust-toolchain.toml` 1.98.1 and `.config/nextest.toml`; it stays its own crate
(`docs/implementation.md` § Shape). This story adds the one Cargo workspace at the repository root
that weft code lands in (`crates/`: `weft-impl`, `weft-bin`, `weft-client`, `weft-controllers`,
`weft-agent`, `weft-testkit`, `weft-conform-target`), makes room for the generated crates under
`generated/`, and puts the tests in `slot task check` and the CI Gate job with the budget of
`~/beyond10x/AGENTS.md` § Tests stay fast enforced by the runner.

- `members = ["crates/*"]`, so a later story adds a crate without editing the root manifest.
- Generated crates: ESS writes `generated/Cargo.toml` with a `[workspace]` of its own
  (`generated/crates/weft-types`, `weft-server`, `weft-control`, `weft-system`, `weft-api`,
  `weft-edge`, `weft-node`; synthesized sample, 2026-11-27), and generated files are never edited.
  Listing those crates as members of the root workspace would give them two workspace roots, so the
  root manifest sets `exclude = ["generated"]` and weft crates depend on them by path; they build
  in the root `target/` and are locked by the root `Cargo.lock`. Lints and `-D warnings` of the root
  apply to `crates/` only. This story proves the form with a probe: `tests/fixtures/generated-probe/`
  holds a two-crate tree with its own `[workspace]` manifest, and `crates/weft-testkit` depends on
  one of its crates by path in a dev-dependency test that the step runs.
- The generated `weft-node` crate takes that package name, so the node agent of this epic is the
  package `weft-agent`.
- One integration-test binary per crate (`tests/main.rs` with modules).
- Toolchain: the root `rust-toolchain.toml` pins the same rustc as `tools/sim` and the CI step
  (1.98.1); CI reads the pin, it does not repeat a second number that can drift.
- No `debug` setting, no `RUSTC_WRAPPER`, `CARGO_BUILD_JOBS` or sccache variable anywhere in the
  repository (the host sets them).
- The test task builds first (`cargo nextest run --workspace --no-run`), then runs the tests through
  `tools/test-step.sh` (POSIX sh), which ends the run after `WEFT_STEP_LIMIT` seconds (default 120)
  with exit 124 and then prints the ten slowest tests from the JUnit output nextest writes under
  `target/nextest/` (also to `target/nextest/slowest.txt`). Before the tests it prints `rustc -V`
  and fails when the version differs from the pinned channel.
- `crates/weft-testkit` starts as a library with one trivial test and the probe test.
- Hooks, so later stories add a step without editing `Taskfile.yml` or the workflow:
  `tools/run-hooks.sh <dir>` (POSIX sh) runs every executable file of a directory in name order and
  stops at the first that fails. Four directories, each with a `README.md` naming its contract:
  - `tools/test-setup.d/` before the tests and `tools/test-teardown.d/` after them, also after a
    failed or killed run (the `test` task);
  - `tools/check.d/`: further gate steps of `check`, run after `test` (the generated-code digest
    check and the conformance run land there);
  - `tools/slow.d/`: the `slow` lane (task `slow`), regeneration that compares bytes, never part of
    `check`.
  The CI Gate job reaches the first three through `task check`. A hook may download or install
  (it is no test, so the no-network rule of the test step does not bind it), writes its files under
  `target/` (a CI-only runner preparation, such as a sysctl, is written in the hook that needs
  it), and is idempotent.
- Gate self-test, in the style of `tools/test-planning-only.sh`: `tools/test-gate-budget.sh` (POSIX
  sh) runs a fixture Cargo project outside the workspace, `tools/budget-fixture/` (one test that
  passes at once, one that sleeps 10 s), with lowered limits, and checks both sides:
  - the passing test under the fixture nextest profile exits 0;
  - the sleeping test under a nextest profile with `slow-timeout = { period = "1s",
    terminate-after = 2 }` is killed (nextest reports it `TIMEOUT`) and the run exits non-zero;
  - `tools/test-step.sh` with `WEFT_STEP_LIMIT=3` around the sleeping test run without the kill
    exits 124;
  - `tools/run-hooks.sh` on fixture hook directories: a hook that exits 1 stops the run non-zero
    before the next hook, and the teardown hooks run after a run the step limit killed;
  - the real values: the root `.config/nextest.toml` `[profile.default]` `slow-timeout` period
    times `terminate-after` is at most 60 s, and the default `WEFT_STEP_LIMIT` of
    `tools/test-step.sh` is at most 120; read from the files, so a raised limit fails the
    self-test.
  The self-test runs in `check` as task `gate-budget`; it builds the fixture once and its five
  cases end within 30 s together.

## Domain relations

None. This story touches no entity of `ess/`.

## Scope

- `Cargo.toml` (workspace root, `exclude = ["generated"]`), `Cargo.lock`
- `rust-toolchain.toml` (root)
- `.config/nextest.toml` (root: `slow-timeout = { period = "30s", terminate-after = 2 }`, JUnit
  output)
- `crates/weft-testkit/` (`Cargo.toml`, `src/lib.rs`, `tests/main.rs`)
- `tests/fixtures/generated-probe/`
- `tools/test-step.sh`, `tools/test-gate-budget.sh`, `tools/budget-fixture/`, `tools/run-hooks.sh`
- `tools/test-setup.d/README.md`, `tools/test-teardown.d/README.md`, `tools/check.d/README.md`,
  `tools/slow.d/README.md`
- `Taskfile.yml` (tasks `test` with its hooks, `gate-budget` and the `check.d` hooks in `check`;
  task `slow`)
- `.github/workflows/gate.yml` (toolchain from `rust-toolchain.toml`, `rust-cache` for the root
  workspace beside `tools/sim`)

## Acceptance

1. `slot task check` and the CI Gate job both run `cargo nextest run --workspace` on the root
   workspace between the setup and teardown hooks.
2. Before the tests, `tools/test-step.sh` prints `rustc -V` to the output of the step and fails the
   step when its version differs from the `channel` of the root `rust-toolchain.toml`, locally and
   in the CI Gate job.
3. After the tests, `tools/test-step.sh` prints the ten slowest tests, name and time, to the output
   of the step and writes the same lines to `target/nextest/slowest.txt`.
4. `tools/test-gate-budget.sh`, run by `slot task check`, passes only when the fixture test past the
   lowered kill limit is reported `TIMEOUT` with a non-zero exit, the step past the lowered step
   limit exits 124, the passing fixture test exits 0, a failing hook stops the run non-zero, and
   the teardown hooks run after a killed run.
5. `tools/test-gate-budget.sh` fails unless the root `.config/nextest.toml` `slow-timeout` period
   times `terminate-after` is at most 60 s and the default `WEFT_STEP_LIMIT` of `tools/test-step.sh`
   is at most 120.
6. The probe test of `crates/weft-testkit` builds and passes in `slot task check` while depending
   by path on a crate of `tests/fixtures/generated-probe/`, whose own manifest declares a
   `[workspace]`.
## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh (`yq` v4 may be called from sh).
- No load tests, no VMs, no GPU workloads.
- The test step makes no request that leaves the host: the control host, the node agent and
  substrate-daemon run on loopback and a Unix socket; seeded rows or fixtures stand in for
  anything else.
