---
format: aep.planning-md/3
id: story:slot-node-process
kind: story
status: draft
title: A process part of a cloudspec architecture runs on a node as a supervised Daemon workload with its environment, prestart, stop and restart
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:ports-node-workloads-local
- depends_on: story:cloud-blocks-layer-refactor-pin
- depends_on: story:cloudspec-submission-plan-from-source
- depends_on: story:cloudspec-submission-apply-workloads
scope:
- confidence: cited
  path: product/cloud/crates/weft-agent/src/follow.rs
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
- confidence: cited
  path: product/cloud/crates/weft-platform/src/node.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit
revision: 4
---
## Epic

Decomposes `epic:slot-node-daemon`, outcome 1 and the first step of its Order (process). cloudspec
rules this lets cloudspec map in `generate weft` (cloudspec `59129eb:crates/cloudspec/src/generate/weft/runs.rs`):
`weft_process_unsupported` (4 refusals of slot), `weft_process_environment_unsupported` (2),
`weft_prestart_unsupported` (1), `weft_stop_unsupported` (2), `weft_restart_unsupported` (1): 10 of 44.

## Dependencies

- PlanFromSource is needed first: a process part reaches weft only through a planned submission.
  `story:cloudspec-submission-plan` (drafted with this story under `epic:cloudspec-submission`).
- `story:cloud-blocks-layer-refactor-pin`: the workload fields `prestart`, `stop`, `restart`,
  `run_as` arrive with cloudspec 59129eb (`blocks/5-workloads/workloads.yaml:341-376,628-631`);
  the vendored copy at f573374 has none of `prestart`, `stop`, `restart`.
- `story:ports-node-workloads-local`: the `workloads` port (start, stop with grace, observe) and
  its local adapter on substrate gap 1. Gap 1 (`substrate` `story:host-driver-supervised-workload`)
  is draft, not implemented, and serves confined workloads only; which substrate family runs
  isolation None is `decision-blocker:slot-node-isolation-none-family`. This story is written
  against the port and tested with a stub adapter, so it does not wait for that answer.

## Outcome

1. `PlanFromSource` maps a part `block: workloads, kind: process` to a workload of package kind
   `Process`, shape `Daemon`, isolation `None`, `run_as` a named user (`deployer` is
   `story:slot-node-host-paths`), one main process with `command` and `environment`, and the
   fields `prestart`, `stop`, `restart` copied unchanged.
2. weft-agent runs the `prestart` steps in list order before the start; a step that exits non-zero
   stops the start and the instance does not reach `Running`. A step with `privileged: true` is
   passed to the port as such; an adapter that cannot run it refuses `NotServed` by name.
3. A stop sends the grace (`<n>s|m|h`) and the kill scope (`Main`, `Group`, `Mixed`) to the port.
4. After an exit the agent restarts by `restart.policy` (`Never`, `OnFailure`, `Always`), after
   `delay`, and keeps the process down on an exit code in `keep_down_on_exit`; each restart is
   reported with `weft.node.RestartProcess` (`ess/5-runtime/node.yaml:415`).

## Domain relations

- process part → `weft.workloads.Workload` (package kind `Process`), one-to-one, the architecture
  owns the part: inferable (inferred from cloudspec `59129eb:crates/cloudspec/src/generate/weft/runs.rs`,
  row `PROCESS`, "a program run on its scope's host"; no ess/1 document declares this relation).
- `weft.node.LocalProcess` → `weft.nodes.Assignment` and → `weft.workloads.Instance`, many-to-one,
  references: inferable from `ess/5-runtime/node.yaml`, entity `weft.node.LocalProcess`, relations
  `assignment` and `instance`.

## Scope

`product/cloud/crates/weft-agent/src/follow.rs`, `product/cloud/crates/weft-platform/src/node.rs`
(the kill scope and prestart in the port types, after `story:ports-node-workloads-local`); worked
out: `product/cloud/crates/weft-cloudspec/src/`, `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-testkit/`.

## Acceptance

A submission whose architecture has one `kind: process` part with `environment`, one `prestart`
step, `stop: {grace: 2s, kill: group}` and `restart: {policy: always, delay: 1s, keep_down_on_exit: [2]}`
is planned and its instance reaches `Running` through a stub `workloads` port after the prestart
step ran; an exit 1 is restarted after 1 s and reported, an exit 2 keeps it down, and a stop
reaches the port with grace 2 s and kill `Group`.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- The test step makes no network request and starts no `substrate-daemon`: the port is a stub.
  No Python in anything committed.
