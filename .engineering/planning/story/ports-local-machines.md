---
format: aep.planning-md/3
id: story:ports-local-machines
kind: story
status: draft
title: 'The `machines` port with its local adapter: a node is a `weft-agent` process beside this host''s `substrate-daemon`'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-node-exec-facts
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: product/cloud/crates/weft-platform/src/cloud.rs
- confidence: inferred
  path: product/cloud/crates/weft-platform/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local/machines.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-testkit/src/node_agent.rs
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, acceptance 3 (the machines port makes a node). Design:
`docs/platform-ports.md` § 6 C1 (`machines`) and § 12.3. Depends on `story:ports-node-exec-facts`
(the crate and its shared refusals).

## Context

- `src/cloud.rs`: trait `Machines` and its types `MachineRequest`, `MachineState`.
  - `make(MachineRequest) -> Machine`; input `node_id`, the pool (`node_pool_id`, zone,
    `machine_type`, `image`, `ess/1-infrastructure/infrastructure.yaml:142-149`), the join token,
    the URL of the control host; output the observed machine `{node_id, state, detail}`.
  - `remove(node_id) -> Removed | Absent`; `observe(node_id) -> Starting | Running | Gone | Unknown`;
    `list() -> [(node_id, state)]`.
  - Refusals: `MachineTypeNotServed`, `ImageNotServed`, `ZoneNotServed`, `CapacityExhausted`,
    `NotServed`, `Unavailable`. Keyed by `node_id`: a `make` for a node whose machine runs starts
    none and answers that machine; `remove` of an absent machine answers `Absent`.
- `src/local/machines.rs`, the local adapter:
  - Configured with the `weft-agent` binary, the URL of the control host, a state directory
    (`<state file>.machines/`) and the substrate supply: `Socket(path)`, a daemon that already
    serves (tests: `WEFT_SUBSTRATE_SOCKET` from `tools/test-setup.d/20-substrate.sh`), or
    `Start(daemon binary)`, which starts one `substrate-daemon` for the host as the setup hook does
    (a transient user unit with `Delegate=yes`, `tools/test-setup.d/20-substrate.sh:144-174`) when
    none answers at `<state dir>/substrate.sock`. One daemon per host, shared by its nodes.
  - `make`: when `observe(node_id)` answers `Running`, answer it; else start
    `weft-agent --control <url> --node-id <id> --join-token <token> --substrate <socket> --report
    <state dir>/<node id>/report.jsonl`, stdin closed, stderr to `<state dir>/<node id>/agent.log`,
    with `PR_SET_PDEATHSIG`; write its PID to `<state dir>/<node id>/pid`. The token is passed on
    the command line only, never written to the state directory.
  - `observe`: `Running` when the PID lives and `/proc/<pid>/cmdline` holds `--node-id <id>`;
    `Gone` otherwise. weft keeps no record that a machine was made; it asks.
  - `remove`: `SIGTERM`, then `SIGKILL` after 2 s, reap, delete the directory; `Absent` when
    nothing runs.
- `weft-testkit`'s `NodeAgent::start_at` (`product/cloud/crates/weft-testkit/src/node_agent.rs:58-83`)
  starts its agent through this adapter, so the tests that start agents keep their text.
- Budget (design § 12.6): one test of `Start` may start a `substrate-daemon` of its own; its time is
  named in this story's report. Every other test uses the setup hook's daemon.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- The machine is no `ess/1` entity: the port keys it by the identity `node_id` of
  `weft.infrastructure.Node` (`ess/1-infrastructure/infrastructure.yaml`, entity
  `weft.infrastructure.Node`, identity `node_id`) and weft holds no row for it (design § 6 C1). No
  relation is declared or assumed.

## Scope

- `product/cloud/crates/weft-platform/`: `src/cloud.rs`, `src/local/machines.rs`, `tests/main.rs`
  module `machines`
- `product/cloud/crates/weft-testkit/`: `src/node_agent.rs`, `Cargo.toml`

## Acceptance

A second `make` for a running node starts no second process and answers the same machine,
`observe` answers `Running` while the agent lives and `Gone` after `remove` (the machine's
`Running` -> `Gone`), and the agents of `weft-testkit` start through the adapter while the join
tests still see their node move `Provisioning` -> `Ready` unchanged.

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
