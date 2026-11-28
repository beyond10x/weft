---
format: aep.planning-md/3
id: story:skeleton-node-run-assignment
kind: story
status: implemented
title: weft-agent follows its assignment, starts the instance confined and records it running
relations:
- decomposes: epic:walking-skeleton
- depends_on: story:skeleton-node-join-session
- depends_on: story:skeleton-node-confine
- serves: vision:weft
- depends_on: story:skeleton-replica-instances
- depends_on: story:skeleton-controllers
- depends_on: story:ports-node-exec-facts
scope:
- confidence: cited
  path: product/cloud/crates/weft-agent/src/agent.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/follow.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/report.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/tests/seeds
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/record_artifact_verified.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/record_backend_ready.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/record_instance_running.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/restore_backend.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings
revision: 30
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T02:23:05Z", actor: "human:operator", revision: 28, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T02:23:06Z", actor: "human:operator", revision: 29, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T04:59:33Z", actor: "human:operator", revision: 30, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 5 (take the assignment, start, report running) and
outcome 3 (the obligations of the last node steps of the path). Depends on
`story:skeleton-node-join-session` (agent loop, binary, report file),
`story:skeleton-node-confine` (confined runs, now behind the `exec` port of `story:ports-node-exec-facts`) and `story:skeleton-replica-instances` (the
binding runner). Blocked by no ESS gap: it delivers its two edge bindings by hand, with their E2
policies.

## Context

- Work list: the agent reads `weft.nodes.AssignmentsOfNode` with its own `node_id` (the NodeAgent
  work list, `ess/5-runtime/nodes.yaml`) every 500 ms and takes each `Active` assignment whose
  instance reads `Scheduled` (an instance in `Starting` or later was started, by this agent or by
  one before a restart). Reading the view makes a restart of the agent and a seeded state work
  the same way. It reads the instance row from `weft.workloads.Instances` (`processes`,
  `artifact_digest`, `state`). An assignment of another node is never in its list.
- Verify: `weft.workloads.RecordArtifactVerified` as NodeAgent, `Scheduled` -> `Starting` under its
  guards (`wrong-node`, `digest-mismatch`, `no-artifact`, `revoked`;
  `ess/4-applications/workloads.yaml` from line 2483). The skeleton fetches no artifact: the snippet
  is the command of the instance process; artifact fetch and signature check are a later story.
- Start: each process through the `exec` port, `weft_platform::node::Exec::run`, which `agent::run(&dyn Exec, &dyn NodeFacts)` already receives (`story:ports-node-exec-facts`, weft#29); its local adapter is substrate. No code in `weft-agent` calls substrate's SDK (the boundary test of that story).
- Running: `weft.workloads.RecordInstanceRunning` as NodeAgent once the exec is `running`, or has
  `exited` before the agent saw `running` (it ran); its `running-ready` outcome moves `Starting` ->
  `Running` (from line 2578).
- Edge: the `running-ready` outcome emits `weft.workloads.InstanceReady`; on it the bindings
  `edge-backend-ready` and `edge-backend-restore` (`ess/components.yaml:2065-2095`) invoke
  `weft.edge.RecordBackendReady` and `weft.edge.RestoreBackend`. Both carry a per-refusal policy,
  so `tools/synth-input.sh` of `story:skeleton-generated-code` removes them (rule of the
  coordinator, 2026-11-27: every path binding with a per-refusal policy is hand-run), and this
  story adds their two entries to the binding runner of `story:skeleton-replica-instances`
  (`product/cloud/crates/weft-impl/src/bindings/`), each with a comment naming ESS
  `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`,
  invoked through `WeftEdge::record_backend_ready` and `WeftEdge::restore_backend`:
  - `edge-backend-ready`: `InstanceReady` -> `RecordBackendReady` (`instance_id`,
    `workload_id`); drops `known`, retries the rest.
  - `edge-backend-restore`: `InstanceReady` -> `RestoreBackend` (`instance_id`); drops
    `not-out-of-traffic`, retries the rest.
  On an instance's first readiness `RecordBackendReady` answers `added` and `RestoreBackend`
  refuses `not-out-of-traffic`, which the entry drops as declared.
- Obligations implemented here, each in its own file under `product/cloud/crates/weft-impl/src/behaviours/`:
  `weft.workloads.RecordArtifactVerified`, `weft.workloads.RecordInstanceRunning`,
  `weft.edge.RecordBackendReady` and `weft.edge.RestoreBackend` (the last 4 of the 24 command
  behaviours on the skeleton path).
- Exit: the exit code and output of `exec.exited` are written to the report file of
  `story:skeleton-node-join-session` (`weft-agent --report <path>`, a file the testkit names), one
  JSON line per finished process: `{"instance_id": ..., "exit_code": ..., "stdout": ...}`. The
  test reads that file; it is the one channel by which a test learns what the agent ran. Recording
  the exit (`RecordInstanceExited`, which starts R2 restarts) is outside the epic.
- Starts: when it starts a process the agent writes `{"started": <instance_id>, "pass": <n>}`, `n`
  the pass in which it started it.
- Work-list passes: after each pass over its work list the agent writes `{"pass": <n>}` to the same
  file, `n` counting from 1. A check that something did not happen waits for a pass line written
  after the agent finished the work it did do, so it is read after the agent looked at its list at
  least once more.
- Test state: seeded through the storage port by `weft-testkit` before the control host starts:
  this node with an open session, a second node, an Artifact row of each snippet digest, a Workload
  row for each instance (`RecordBackendReady` refuses `no-such-workload` without it), and for
  each node one Instance in state `Scheduled` with that `node_id` and an `Active` Assignment. The
  process of this node is `sh -c "echo weft"`, that of the other node `sh -c "echo other"`, so any
  exec of the other instance would show in the report by its output. Both instances start in
  `Scheduled`, so the guard of `RecordArtifactVerified` is exercised as well. Neither process
  carries a `Ready` check, so `RecordInstanceRunning` answers `running-ready` and `InstanceReady`
  is emitted. Seed files live in
  the crate tests.

## Domain relations

- `weft.nodes.Assignment` -> `weft.workloads.Instance`, many-to-one, references; the instance
  outlives each assignment — inferable: `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Assignment`,
  relation `instance`.
- `weft.nodes.Assignment` -> `weft.infrastructure.Node`, many-to-one, references — inferable:
  `ess/5-runtime/nodes.yaml`, entity `weft.nodes.Assignment`, relation `node`.
- `weft.nodes.Assignment` -> `weft.nodes.NodeSession`, many-to-one, references; the assignment
  outlives a lost session until its lease ends — inferable: `ess/5-runtime/nodes.yaml`, entity
  `weft.nodes.Assignment`, relation `session`.
- `weft.workloads.Instance` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Instance`, relation `workload`.
- `weft.edge.RouteBackend` -> `weft.workloads.Instance`, one backend per instance
  (`RecordBackendReady` refuses a second one, `known`), references — inferable: `ess/5-runtime/edge.yaml`, entity
  `weft.edge.RouteBackend`, relation `instance`.
- A process of the instance -> one confined exec: one per `weft.node.LocalProcess` — inferable for
  the LocalProcess side: `ess/5-runtime/node.yaml`, entity `weft.node.LocalProcess`, relations
  `instance` and `assignment`; the exec is no `ess/1` entity.

## Scope

- `product/cloud/crates/weft-agent/` (`src/follow.rs`, wiring in `src/agent.rs`, the process, started and pass
  lines in `src/report.rs`, `tests/main.rs` module `follow`,
  `tests/seeds/`, and `Cargo.toml` with `Cargo.lock` for the dev-dependencies the seeds need:
  `weft-impl` rows and the generated `weft-types` snapshots); `Exec::run` blocks until the exec
  ends, so the follower runs each start on a thread of its own and the work-list passes go on
- `product/cloud/crates/weft-impl/` (`src/behaviours/`: `record_artifact_verified.rs`,
  `record_instance_running.rs`, `record_backend_ready.rs`, `restore_backend.rs`; the two `edge-*`
  entries of the binding runner in `src/bindings/`)

## Acceptance

1. From that seeded state, after `weft-agent` starts, its instance reads `Running` in
   `weft.workloads.Instances`.
2. In the same run, the report file holds the line
   `{"instance_id": <its instance>, "exit_code": 0, "stdout": "weft\n"}`, the exec having run
   through the `exec` port on `substrate-daemon`.
3. Once the report file holds a `pass` line whose number is greater than the `pass` of the
   `started` line of its own instance, the instance assigned to the other node still reads
   `Scheduled` in `weft.workloads.Instances`.
4. At that point the report file holds no line with the other instance's `instance_id` and no line
   with `stdout` `other`.
5. In the control host log of that run, the one request line of `RecordInstanceRunning` has a
   status in 200-299.
6. In the same log, after the `seq` read before the agent started, the first readiness of its
   instance gives one binding line `edge-backend-ready` with result `applied`.
7. In the same log and range, there is one binding line `edge-backend-restore` with refusal
   `not-out-of-traffic` and result `dropped`.
8. In the same log and range, there is no `edge-backend-restore` line with result `retried`.

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
