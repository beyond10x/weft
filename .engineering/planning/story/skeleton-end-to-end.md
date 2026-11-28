---
format: aep.planning-md/3
id: story:skeleton-end-to-end
kind: story
status: active
title: 'End-to-end: PlaceInstance leads to a confined snippet that runs and an instance view that answers Running'
relations:
- decomposes: epic:walking-skeleton
- depends_on: story:skeleton-controllers
- depends_on: story:skeleton-node-join-session
- depends_on: story:skeleton-node-run-assignment
- serves: vision:weft
scope:
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
revision: 18
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T03:35:17Z", actor: "human:operator", revision: 17, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T03:35:17Z", actor: "human:operator", revision: 18, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 6. Depends on `story:skeleton-controllers`,
`story:skeleton-node-join-session` and `story:skeleton-node-run-assignment` (and through them on
the control host, the workload definitions, the replica step and the confined start). Blocked by no
ESS gap: every piece of the path is in those stories, the hand-written E1-E3 restorations included,
except the behaviour of `RecordBackendNotReady`, which this story adds (Context).

## Context

The acceptance of the skeleton (`docs/implementation.md` § Walking skeleton): one control host and
one node on this machine; a tenant and an account; `PlaceInstance` leads to a confined code snippet
that runs and is recorded running.

The path, after `sim/experiments/full-setup.yaml` lines 38-70, deployed into the default cluster
(the tenant cluster, pool and topology steps of that chain, and `AddLink`, are left out, as the
globex site of the experiment does):

- operator: `CreateCloud`, `DeclareZone`, `DefinePriorityClass`, `RegisterRuntime`
  (`package_kinds: [Snippet]`), `CreateNodePool`, `ProvisionPoolNode`, `CreateDefaultCluster`,
  `CreateTenant`, `GrantTopQuota`;
- node (`weft-agent` and the node-channel endpoint): `JoinNode`, `RecordNodeObservation`,
  `OpenNodeSession`;
- tenant: `CreateAccount`, `CreateFormation`, `AddComponent`, `CommitRevision`,
  `CreateEnvironment`, `RegisterArtifact` (kind `Snippet`), `DefineWorkload` (package kind
  `Snippet`, one process `sh -c "echo weft"`, isolation `Container`), `DefineService` (1 replica);
- controllers: `RequestReplica`, `PlaceInstance`;
- bindings, all nine removed from the synthesis input and delivered by the binding runner of
  `story:skeleton-replica-instances` with their declared policies: `r1-create-replica` ->
  `CreateInstance`, `r1-submit-scheduling-request` -> `SubmitSchedulingRequest`,
  `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`, `r4-record-placement-assigned`;
- node: `RecordArtifactVerified` (on its `InstanceHealthStarting` the binding
  `edge-backend-restarting` -> `RecordBackendNotReady`), `RecordInstanceRunning`, then on
  `InstanceReady` the bindings
  `edge-backend-ready` -> `RecordBackendReady` and `edge-backend-restore` -> `RestoreBackend`.

The test composes the `weft-testkit` starters that already exist (`ControlHost` from
`story:skeleton-control-host`, the `weft-agent` child process from
`story:skeleton-node-join-session`) and the substrate-daemon the setup hook of
`story:skeleton-node-confine` starts; it adds no starter and changes none. It starts the processes
once for this test; there is one such test in the step. The test is timed before it is added; over
5 s its time is named in the report, over 60 s it is a defect of this story, not a timeout to raise.

## Domain relations

- `weft.accounts.Tenant` -> `weft.accounts.Account`, one-to-many, tenant owns account — inferable:
  `ess/2-tenancy/accounts.yaml`, entity `weft.accounts.Tenant`, relation `accounts`.
- `weft.workloads.Workload` -> `weft.accounts.Account`, `weft.formations.Formation` and
  `weft.workloads.Runtime`, each many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Workload`, relations `account`,
  `formation`, `runtime`.
- `weft.workloads.Service` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Service`, relation `workload`.
- `weft.workloads.Instance` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Instance`, relation `workload`.
- `weft.nodes.Assignment` -> `weft.workloads.Instance`, `weft.infrastructure.Node`,
  `weft.nodes.NodeSession`, each many-to-one, references — inferable: `ess/5-runtime/nodes.yaml`,
  entity `weft.nodes.Assignment`, relations `instance`, `node`, `session`.

- The path's `RecordArtifactVerified` publishes `weft.workloads.InstanceHealthStarting`; its
  generated binding `edge-backend-restarting` (`ess/components.yaml:2116`) invokes
  `weft.edge.RecordBackendNotReady`, which no behaviour implements yet, so the request answers
  `501` with `committed: true` (`product/cloud/crates/weft-agent/tests/follow_adversary.rs:366`).
  This story adds that behaviour: on a first start no backend is `Ready`, the command refuses
  `not-ready`. The binding carries a per-refusal policy (drop `not-ready`, retry the rest,
  `ess/components.yaml:2111-2115`), which `tools/synth-input.sh` rewrites to plain retry for every
  binding it does not remove; so, by the rule of 2026-11-27 (every path binding with a per-refusal
  policy is hand-run), this story removes `edge-backend-restarting` from synthesis
  (`removed_bindings` in `tools/synth-input.sh`) and adds it to the binding runner, like
  `edge-backend-ready` and `edge-backend-restore`, with the same comment naming the ESS story.

## Scope

- `product/cloud/crates/weft-testkit/tests/main.rs`: the module `skeleton` only (it composes the existing
  starters)
- `product/cloud/crates/weft-impl/src/behaviours/record_backend_not_ready.rs`
- `product/cloud/crates/weft-impl/src/bindings/edge_backend_restarting.rs` and its `ENTRIES` line in
  `bindings/mod.rs`
- `tools/synth-input.sh` (`edge-backend-restarting` in `removed_bindings`, and the id list in its
  comment), `tools/test-generated.sh` (its list of removed binding ids, run by
  `tools/check.d/05-generated-selftest.sh`), the id list in the doc comment of `bindings/mod.rs`
- `product/cloud/crates/weft-agent/tests/follow_adversary.rs` (`:366-368`, `:405` expect the
  `501 committed` answer of `RecordArtifactVerified` that this story removes; they then expect its
  `starting` outcome) and the comment in `product/cloud/crates/weft-agent/src/follow.rs:14`

## Acceptance

1. `slot task check` runs one test that starts the control host and `weft-agent` (with
   `--report <path>`) beside `substrate-daemon`, sends the operator and tenant commands of the path
   over HTTP, and within 30 s of its first command reads state `Running` for the instance in
   `weft.workloads.Instances`.
2. In the same run the report file holds the line
   `{"instance_id": <that instance>, "exit_code": 0, "stdout": "weft\n"}`.
3. The control host log after the `seq` read before the first command holds a request line with a
   2xx status for each command of the path that is sent as a request (the operator, tenant, node
   and controller commands listed above, `OpenNodeSession` and the controller commands included).
4. The same log lines hold no request line with status `501`.
5. The same log lines hold a binding line with result `applied` for each of `r1-create-replica`,
   `r1-submit-scheduling-request`, `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`,
   `r4-record-placement-assigned` and `edge-backend-ready`.
6. The same log lines hold a binding line `edge-backend-restore` with refusal `not-out-of-traffic`
   and result `dropped`.
7. The same log lines hold a binding line `edge-backend-restarting` with refusal `not-ready` and
   result `dropped`.

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
