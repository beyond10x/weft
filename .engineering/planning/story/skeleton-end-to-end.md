---
format: aep.planning-md/3
id: story:skeleton-end-to-end
kind: story
status: draft
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
revision: 12
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 6. Depends on `story:skeleton-controllers`,
`story:skeleton-node-join-session` and `story:skeleton-node-run-assignment` (and through them on
the control host, the workload definitions, the replica step and the confined start). Blocked by no
ESS gap: every piece of the path is in those stories, the hand-written E1-E3 restorations included.

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
- bindings, all eight removed from the synthesis input and delivered by the binding runner of
  `story:skeleton-replica-instances` with their declared policies: `r1-create-replica` ->
  `CreateInstance`, `r1-submit-scheduling-request` -> `SubmitSchedulingRequest`,
  `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`, `r4-record-placement-assigned`;
- node: `RecordArtifactVerified`, `RecordInstanceRunning`, then on `InstanceReady` the bindings
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

## Scope

- `product/cloud/crates/weft-testkit/tests/main.rs`: the module `skeleton` only (it composes the existing
  starters)

## Acceptance

1. `slot task check` runs one test that starts the control host and `weft-agent` (with
   `--report <path>`) beside `substrate-daemon`, sends the operator and tenant commands of the path
   over HTTP, and within 30 s of its first command reads state `Running` for the instance in
   `weft.workloads.Instances` and the report line
   `{"instance_id": <that instance>, "exit_code": 0, "stdout": "weft\n"}`.
2. The control host log after the `seq` read before the first command holds a request line with a
   2xx status for each command of the path that is sent as a request (the operator, tenant, node
   and controller commands listed above, `OpenNodeSession` and the controller commands included),
   the positive control of statement 3.
3. The same log lines hold no request line with status `501`, and a binding line with result
   `applied` for each of `r1-create-replica`, `r1-submit-scheduling-request`, `r4-bind-placement`,
   `r4-assign`, `r4-schedule-instance`, `r4-record-placement-assigned` and `edge-backend-ready`,
   and a binding line `edge-backend-restore` with refusal `not-out-of-traffic` and result
   `dropped`: all eight path bindings answered, and no obligation on the path is left unmet.
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
