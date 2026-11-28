---
format: aep.planning-md/3
id: story:slot-node-single-rules
kind: story
status: draft
title: Limits, priority, environment files and parity of an architecture run on the node (outline)
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:slot-node-process
- depends_on: story:slot-node-pool-placement
scope:
- confidence: inferred
  path: product/cloud/crates/weft-agent/src
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: inferred
  path: product/cloud/crates/weft-controllers/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
revision: 3
---
## Epic

Decomposes `epic:slot-node-daemon`, the last step of its Order (the remaining single rules).
Outline: to be scoped before it is proposed. cloudspec rules this lets cloudspec map:
`weft_limits_unsupported` (1: `open_files: 65536`), `weft_priority_unsupported` (1:
`cpu: background`), `weft_environment_files_unsupported` (1: an optional file),
`weft_parity_unsupported` (1: rustc commit, targets, components, go and node versions between
`workstation` and `build-node`): 4 of 44. PlanFromSource is needed first, through
`story:slot-node-process`.

## Outcome

1. `limits` (`cpu_millis`, `memory_mib`, `open_files`) and `process_priority.cpu` reach the port as
   ceilings and a CPU share of the process.
2. Each `environment_files` entry is read on the node at start; a missing file with
   `optional: true` is no error, without it the start is refused by name.
3. `parity`: a workload of one of the named scopes is placed on a node only when the named facts
   (from `weft.infrastructure.Node` facts) equal those of the other scope; a mismatch is a named
   refusal of placement.
4. Not refused by any rule, yet needed to run the node daemon (the generator maps them without a
   coverage refusal): `upgrade: {reload: {signal: HUP}}` sends the signal and keeps the process,
   and `checks.ready` of kind `exec` (`slot status`) gates `Running`.

## Scope

Worked out: `product/cloud/crates/weft-cloudspec/src/`, `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-agent/src/`, `product/cloud/crates/weft-controllers/src/`.

## Acceptance

A planned process with `limits`, `priority`, an optional environment file and a parity between two
scopes starts with its ceilings and share at the port, is placed only on a node whose facts match,
reloads on `HUP` without a restart, and reads `Running` only after its exec ready check passes.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- No network request in the test step. No Python in anything committed.
