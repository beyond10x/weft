---
format: aep.planning-md/3
id: story:ports-boundary-test
kind: story
status: draft
title: A test fails when code outside an adapter calls substrate's SDK or starts a node process
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-exec-facts
- depends_on: story:ports-local-machines
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/tests/fixtures/boundary
- confidence: cited
  path: product/cloud/crates/weft-platform/tests/main.rs
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, acceptance 2 (a test fails when code outside an adapter calls
substrate's SDK or spawns a node process). Design: `cloudspec@2ed72c2:docs/platform-ports.md` § 3.5 and § 12.5.
Depends on `story:ports-node-exec-facts` and `story:ports-local-machines`: until both moved the
SDK and the agent start behind adapters, the repository tree itself breaks the rules.

## Context

- `product/cloud/crates/weft-platform/tests/main.rs`, module `boundary`. It reads files only, no
  build.
- Allowed paths: `product/cloud/crates/weft-platform/src/local/` and
  `product/cloud/crates/weft-platform/tests/`.
- Rules:
  - fails when a `Cargo.toml` under `product/` other than
    `product/cloud/crates/weft-platform/Cargo.toml` names `b10x-substrate-sdk`;
  - fails when a `.rs` file under `product/` outside the allowed paths names `b10x_substrate_sdk`;
  - fails when a `.rs` file under `product/` outside the allowed paths calls `Command::new` and
    names `weft-agent`, `substrate-daemon`, `node_agent_binary` or `WEFT_AGENT`.
- Each rule has a fixture tree holding one violation (`tests/fixtures/boundary/fail-*`) on which
  the check fails; the repository tree passes.
- Not covered and allowed: `tools/test-setup.d/20-substrate.sh`, the test-run fixture outside
  `product/` that starts the one daemon of a test run.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None: the test reads source files, no domain state.

## Scope

- `product/cloud/crates/weft-platform/tests/main.rs` (module `boundary`)
- `product/cloud/crates/weft-platform/tests/fixtures/boundary/`

## Acceptance

The module `boundary` passes on the repository tree and turns from pass to fail on each of its
`fail-*` fixture trees (a manifest, an SDK path, a node start).

## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh.
- The test step makes no request that leaves the host; no test starts a `substrate-daemon` of its
  own except where this story says so (design § 12.6).
