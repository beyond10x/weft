---
format: aep.planning-md/3
id: story:ports-node-exec-facts
kind: story
status: active
title: The `weft-platform` crate with the `exec` and `node-facts` ports; `confine` moves behind them
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: product/cloud/crates/weft-agent/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-agent/src/agent.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/bin/weft-probe.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/confine.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/main.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-platform
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T01:05:46Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T01:05:46Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:platform-ports`, acceptance 2 on the node side: the node reaches the place it runs
only through ports. Design: `docs/platform-ports.md` § 5 (N1 `exec`, N2 `node-facts`), § 12.1 and
§ 12.2. First story of the first build; depends on none.

## Context

- New crate `product/cloud/crates/weft-platform`, a workspace member through the
  `product/*/crates/*` glob of `Cargo.toml:3`:
  - `src/lib.rs`: the shared refusals `Unavailable { detail }`, `NotServed { operation }`,
    `Invalid { field, reason }` (design § 3.4).
  - `src/node.rs`: traits `Exec` and `NodeFacts` and their types `Process`, `HostFile`, `Limits`,
    `Exited`, `ExecState`, `NodeFacts`. `ExecState` is weft's own enum, not substrate's
    re-exported type (`product/cloud/crates/weft-agent/src/confine.rs:23` today).
  - `src/local/substrate.rs`: the local adapter, the only source file that names
    `b10x_substrate_sdk`.
  - `src/bin/weft-probe.rs`: moved from `product/cloud/crates/weft-agent/src/bin/weft-probe.rs`
    (`CARGO_BIN_EXE_weft-probe` is set only for the tests of the crate that builds it).
  - `tests/main.rs`, module `exec`: the confine tests, moved.
  - Dependencies: `b10x-substrate-sdk` (moved from `product/cloud/crates/weft-agent/Cargo.toml:29`,
    same rev), `tokio`, `serde_json`, `libc`. No dependency on `weft-impl` or generated code, so it
    builds without `target/generated/`.
- `confine.rs` moves unchanged in behaviour; `Confine` implements `Exec` (design § 5 N1: states
  `accepted`, `running`, `exited`, `cancelled`, `expired`, `unknown`; refusals `EmptyCommand`,
  `HostPath`, `Ended`, `NoExit`, `Refused { code }`, `Unavailable`; one `run` is one substrate
  operation and an unanswered start is reported `Unavailable`, never retried).
- `agent.rs:194-227` (`capacity`, `isolation_facts`, the second direct SDK call) moves into the same
  adapter as `NodeFacts`. A fact the daemon withholds stays absent, never `false` (design § 5 N2).
- `agent::run` takes `&dyn Exec` and `&dyn NodeFacts`; `main.rs` builds the local adapter from
  `--substrate <socket>`. The arguments of the agent do not change (`agent.rs:56-57`).
- `weft-agent`'s manifest drops `b10x-substrate-sdk` and depends on `weft-platform`.
- The traits are synchronous: `weft-agent` is a blocking program (`agent.rs:107-164`); the
  adapter keeps its own current-thread runtime, as `agent.rs:208-219` does today.
- `story:skeleton-node-run-assignment` (draft) starts its process "through `confine`"; after this
  move it calls the `Exec` port. Its acceptance does not change.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None is added or assumed. `NodeFacts` answers what `weft.infrastructure.Node` already holds in
  its own fields `capacity` and `isolation_facts` (`ess/1-infrastructure/infrastructure.yaml`,
  entity `weft.infrastructure.Node`); an exec is no `ess/1` entity.

## Scope

- `product/cloud/crates/weft-platform/` (new)
- `product/cloud/crates/weft-agent/`: `Cargo.toml`, `src/agent.rs`, `src/main.rs`, `src/lib.rs`,
  `src/confine.rs` (removed), `src/bin/weft-probe.rs` (moved), `tests/main.rs`
- `Cargo.lock`

## Acceptance

`product/cloud/crates/weft-platform/Cargo.toml` is the one manifest under `product/` that names
`b10x-substrate-sdk`, and in `slot task check` the moved confine tests pass and the join tests of
`weft-agent` still see their node move `Joining` -> `Ready` on the observation the `NodeFacts`
port answered.

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
