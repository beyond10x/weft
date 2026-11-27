---
format: aep.planning-md/3
id: story:skeleton-control-host
kind: story
status: implemented
title: 'Control host: one weft binary serving the generated HTTP surface, SQLite storage port, context port, and the operator and tenancy obligations'
relations:
- decomposes: epic:walking-skeleton
- depends_on: story:skeleton-workspace-gate
- depends_on: story:skeleton-node-confine
- depends_on: story:skeleton-generated-code
- serves: vision:weft
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: product/cloud/crates/weft-bin
- confidence: cited
  path: product/cloud/crates/weft-client
- confidence: cited
  path: product/cloud/crates/weft-impl/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-impl/build.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_account.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_default_cluster.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_node_pool.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/create_tenant.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/define_priority_class.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/grant_top_quota.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/provision_pool_node.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/context
- confidence: cited
  path: product/cloud/crates/weft-impl/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/log.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/storage
- confidence: cited
  path: product/cloud/crates/weft-testkit
revision: 42
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T16:03:31Z", actor: "human:operator", revision: 40, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T16:03:31Z", actor: "human:operator", revision: 41, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-27T17:18:35Z", actor: "human:operator", revision: 42, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 3 (the control host, its ports, and the obligations of
the operator and tenancy part of the path). Depends on `story:skeleton-workspace-gate`,
`story:skeleton-generated-code` (the generated crates) and `story:skeleton-node-confine` (lands
first, so the `Cargo.lock` edits of the two come one after the other). Blocked by no ESS gap: the
generated code comes from the transformed input of `story:skeleton-generated-code`; this story
restores by hand the one E1 transformation on its part of the path.

## Context

Decisions A2, A3 and A4 (`docs/implementation.md`): weft runs its domains as generated Rust; the
control host is one weft binary serving `weft-api` and `weft-control` in one process; SQLite behind
the generated storage port.

- `product/cloud/crates/weft-bin`: the control host binary. It builds one `weft_system::System` from the
  generated components, the behaviours of `product/cloud/crates/weft-impl` and the obligations, and owns the
  accept loop itself: each connection is read with `weft_server::http::read` and answered by the
  `dispatch_with_events` of the module whose routes hold the path, `weft_server::weft_api` or
  `weft_server::weft_control` (the generated `serve` loop takes the system for
  good, while the controllers and the node-channel endpoint of later stories must share it). One
  writer: the system sits behind one lock, commands run one at a time.
- `product/cloud/crates/weft-impl`, the storage port: one SQLite table per entity the generated behaviours read
  or write (load, store, replace, remove, list in store order), one database file given by
  `--state <path>`.
- `product/cloud/crates/weft-impl`, the context port:
  - the clock, a parameter (system clock in the binary; tests read the wall clock around a call);
  - identities and assigned values: UUIDv4 from a mint;
  - external decisions: a decider, for the skeleton a table in code; every `external:` branch the
    table does not list answers its default branch (not taken). The one listed branch on this part
    of the path is `weft.infrastructure.JoinNode/token-invalid`: it is taken when the token is not
    the one weft issued for the node (below);
  - caller attributes: read from the request header `X-Weft-Caller`, `{"actor": "<qualified actor>",
    "caller": {...}}`. This is test-only: the binary trusts the header only when built with the
    cargo feature `test-callers`; without it every request is answered as no caller
    (`403`, `not granted`). The authenticate function takes that switch as a parameter, so one test
    build checks both settings. Authentication proper (mTLS, tokens) is a later epic.
- Obligations implemented here, each in its own file under `product/cloud/crates/weft-impl/src/behaviours/`
  (`product/cloud/crates/weft-impl/build.rs` collects every file of that directory into the behaviour type, so a
  later story adds its own files and edits no shared list): `weft.infrastructure.DefinePriorityClass`,
  `weft.infrastructure.CreateNodePool`, `weft.infrastructure.ProvisionPoolNode`,
  `weft.cluster.CreateDefaultCluster`, `weft.accounts.CreateTenant`, `weft.accounts.CreateAccount`,
  `weft.accounts.GrantTopQuota` (7 of the 24 command behaviours on the skeleton path; contracts in
  `generated/PLAN.md` § Obligations). `CreateCloud`, `DeclareZone`, `RegisterRuntime` and `JoinNode`
  are generated.
- E1, restored by hand (comment naming ESS `story:the-rust-target-writes-a-one-time-response-outcome-as-an-obligation-that-carries-its-contract-instead-of-refusing-the-workspace`): the one-time
  response of `ProvisionPoolNode/provisioned`. The join token is minted once, answered once, and
  stored only as its SHA-256 beside the node; the `JoinNode/token-invalid` decider compares
  against that hash. Removed together with the E1 transformation when ESS ships.
- Every other obligation answers the generated stub, `UnmetObligation`: the route answers `501`
  with `committed: false` and the plan entry it names, and writes nothing.
- Control host log: `--log <path>` writes one JSON line per answered request
  `{"seq", "kind": "request", "operation", "actor", "status"}`, one per published event
  `{"seq", "kind": "event", "name"}`, and, from `story:skeleton-replica-instances` on, one per
  binding answer `{"seq", "kind": "binding", "binding", "refusal", "result"}` (`result` one of
  `applied`, `dropped`, `retried`; `refusal` the refusing outcome or absent). `seq` counts up from 1
  over the life of the file. The writer is `product/cloud/crates/weft-impl/src/log.rs`, so an in-process system
  of the tests writes the same lines; a request handled in-process logs as a request too. Later
  stories read it by delta: the highest `seq` before an act, then only the lines after it.
- `product/cloud/crates/weft-client`: the HTTP client the tests, the node agent and the conformance adapter
  share: send a command as an actor with its caller, read a view, read the log after a `seq`.
- `product/cloud/crates/weft-testkit`: `ControlHost` starts the test build of `weft-bin` with a temporary state
  file, `--port 0`, the log in its temporary directory, reads the listening line and stops it on
  drop; `seed(rows)` writes rows into a state file through the storage port before a start; and the
  operator and tenant steps of the path as functions, input as in
  `sim/experiments/full-setup.yaml`.

## Domain relations

- `weft.accounts.Tenant` -> `weft.accounts.Account`, one-to-many, tenant owns account (`owns`, via
  `tenant_id`); an account is created after its tenant — inferable: `ess/2-tenancy/accounts.yaml`,
  entity `weft.accounts.Tenant`, relation `accounts`.
- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references; the pool
  exists before the node `ProvisionPoolNode` makes in it — inferable:
  `ess/1-infrastructure/infrastructure.yaml`, entity `weft.infrastructure.Node`, relation `pool`.

## Scope

- `product/cloud/crates/weft-bin/`
- `product/cloud/crates/weft-impl/` (`src/storage/`, `src/context/`, `src/log.rs`, `build.rs`, and one file per
  obligation listed above)
- `product/cloud/crates/weft-client/`
- `product/cloud/crates/weft-testkit/` (`src/`, `tests/main.rs` module `control_host`)

## Acceptance

1. A test in `product/cloud/crates/weft-testkit/tests/main.rs` starts the control host, sends `CreateCloud`,
   `DeclareZone`, `DefinePriorityClass`, `CreateNodePool`, `ProvisionPoolNode`,
   `CreateDefaultCluster`, `CreateTenant`, `CreateAccount` and `GrantTopQuota` through `weft-client`,
   each as its own actor with the input of `sim/experiments/full-setup.yaml`, and reads each created
   row back from its view with the identity the command answered and each input field equal to the
   value sent (for example `weft.accounts.Tenants`: `name` `acme`; the quota row: `resource_class`
   `cpu`, `limit` 64; the node pool: `name` `shared`, `zone` `z1`, `max_size` 8).
2. In the same test, the control host log after the `seq` read before `CreateTenant` holds a request
   line `{"kind": "request", "operation": "weft.accounts.CreateTenant", "actor":
   "weft.actors.ClusterOperator", "status": <2xx>}` and an event line `{"kind": "event", "name":
   "weft.accounts.TenantCreated"}`, each with the stated fields, the event line with a higher `seq`
   than the request line, and every `seq` higher than the one read before.
3. In the same test, `JoinNode` as NodeAgent with a token other than the one `ProvisionPoolNode`
   answered gets outcome `token-invalid` and the node row still reads `Provisioning`
   (`ess/1-infrastructure/infrastructure.yaml` line 247); then, as the positive control, `JoinNode`
   with the issued token gets `joined` and the row reads `Joining`.
4. After the control host is stopped and started again on the same state file, the rows of
   statement 1 read back with the same field values.
5. A command whose behaviour is an obligation off the path, `weft.cluster.CreateCluster`, answers
   `501` with `committed: false` naming its plan entry, and `weft.cluster.Clusters` holds no new row.
6. With the caller switch off, a `CreateTenant` request carrying `X-Weft-Caller` for ClusterOperator
   is answered `403` `not granted` and `weft.accounts.Tenants` holds no new row; with it on, the same
   request is answered with a 2xx status and outcome `created`, and the tenant row appears (positive
   control).
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
