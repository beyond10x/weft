---
format: aep.planning-md/3
id: story:ports-node-workloads-port
kind: story
status: active
title: The Workloads port and an in-memory stub adapter, before any substrate adapter
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/node.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/tests/main.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T14:29:24Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T14:29:24Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:platform-ports`. Split out of `story:ports-node-workloads-local` so that
`story:slot-node-process` can be built against the port before any substrate adapter exists: the
owner's pick D-S1 = A runs isolation None through substrate's host-process family, and substrate
gap 1 (confined workloads) is still draft.

## Outcome

1. `weft-platform/src/node.rs` declares a `Workloads` port beside `Exec` and `NodeFacts`: start a
   long-lived supervised workload (command, environment, working directory, restart policy, stop
   grace and kill scope, prestart commands), stop it with its grace, and observe its state
   (starting, running, exited with code, restarting).
2. A stub adapter in `weft-platform` (test support) implements the port in memory, with hooks to
   make a start fail, a process exit with a code and a stop exceed its grace.
3. The node agent can be built with the port (no caller uses it yet); `Exec` and `NodeFacts` are
   unchanged.

## Scope

`product/cloud/crates/weft-platform/src/node.rs`, `product/cloud/crates/weft-platform/src/local.rs`,
`product/cloud/crates/weft-platform/tests/main.rs`.

## Acceptance

1. A weft-platform test drives the stub through start, running, exit with code 3, restart, and a
   stop whose grace runs out (kill scope applied), and reads each state through the port.
2. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task test'` exits 0 within
   120 s; `tools/check.d/20-rust-lint.sh` exits 0.
