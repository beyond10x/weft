---
format: aep.planning-md/3
id: story:skeleton-replica-instances
kind: story
status: draft
title: 'Replica step and instance creation: R1 controller, restored r1-create-replica, CreateInstance and SubmitSchedulingRequest'
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
- depends_on: story:skeleton-workload-definitions
- depends_on: story:skeleton-node-join-session
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/weft-bin/Cargo.toml
- confidence: cited
  path: crates/weft-bin/src/main.rs
- confidence: cited
  path: crates/weft-controllers/Cargo.toml
- confidence: cited
  path: crates/weft-controllers/src/lib.rs
- confidence: cited
  path: crates/weft-controllers/src/replicas.rs
- confidence: cited
  path: crates/weft-controllers/tests/main.rs
- confidence: cited
  path: crates/weft-impl/src/behaviours/create_instance.rs
- confidence: cited
  path: crates/weft-impl/src/behaviours/submit_scheduling_request.rs
- confidence: cited
  path: crates/weft-impl/src/bindings
- confidence: inferred
  path: crates/weft-impl/src/lib.rs
- confidence: cited
  path: crates/weft-testkit/src
revision: 10
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 4 (the replica step) and outcome 3 (the obligations and
restored bindings of the instance part of the path). Depends on `story:skeleton-workload-definitions`
(a defined service to scale). Blocked by no ESS gap: it restores by hand what the E2 and E3
transformations of `story:skeleton-generated-code` removed on its part of the path.

## Context

- `crates/weft-controllers` (created here): the level and time steps of `ess/components.yaml`
  (bindings block, lines 1103-1150) that no binding runs, as functions over the same ports the
  generated behaviours use: `tick(system, clock)` reads a gap view through the generated query and
  sends one command per row through `weft_server::weft_control::handle` as the actor the step names.
  `weft-bin` runs `tick` in its process every `--tick-ms` (default 200) under the one system lock of
  `story:skeleton-control-host`; tests call `tick` in-process on a system the testkit builds.
- Step R1 level: `weft.workloads.ServicesBelowScale` -> `weft.workloads.RequestReplica` as
  Scheduler; the replica index is the lowest index below `replicas` with no live instance.
- Binding runner (created here, `crates/weft-impl/src/bindings/`). The generated System has no seam
  for a per-refusal policy (its `deliver_*` functions are private, a declared refusal goes to a
  private `held_*` list, and `run_*` drops `take_invocations()`; design critic, 2026-11-27). So
  `tools/synth-input.sh` of `story:skeleton-generated-code` removes the eight bindings of the path
  that carry a per-refusal policy from the synthesis input, and this runner delivers them by hand:
  it reads the events `dispatch_with_events` and `handle_with_events` return, maps each event of a
  hand-run binding to its command as `ess/components.yaml` declares (its `where:` filter included), invokes it
  in-process through the generated method of the accepting component (for example
  `WeftControl::create_instance`), without an actor, as the generated delivery does; the events that
  invocation publishes are dispatched like those of a request. It applies the binding's
  declared per-refusal policy: a refusal in its `drop:` list is dropped; any other refusal, and a
  transport failure, is kept and attempted again on the next tick. `weft-bin` runs the runner after
  every request and every tick under the one system lock. Each answer goes to the control host log
  as a binding line of `story:skeleton-control-host`, `{"seq", "kind": "binding", "binding",
  "refusal", "result"}` with result `applied`, `dropped` or `retried`. The events of a hand-run
  invocation publishes go through the runner again. Tests replay or construct an event through the
  runner in-process to reach a refusal.
- The two entries of this part, each with a comment naming its ESS story id:
  - `r1-create-replica` (ESS `story:rust-binding-wraps-a-plain-copy-into-an-optional-member`, removed by E3):
    `weft.workloads.ReplicaRequested` -> `weft.workloads.CreateInstance` with `workload_id`,
    `service_id` and `replica_index`, the plain values wrapped into the Optional members by hand;
    drops `replica-live` and `workload-retired`, retries every other refusal;
  - `r1-submit-scheduling-request` (ESS `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`, removed for E2):
    `weft.workloads.InstanceCreated` (where `job_id` is not defined) ->
    `weft.scheduling.SubmitSchedulingRequest`; drops `already-requested`, retries every other
    refusal.
  `story:skeleton-controllers` adds the four `r4-*` entries.
- Obligations implemented here, each in its own file under `crates/weft-impl/src/behaviours/`:
  `weft.workloads.CreateInstance` and `weft.scheduling.SubmitSchedulingRequest` (2 of the 24 command
  behaviours on the skeleton path). `RequestReplica` is generated.
- When ESS ships E2 or E3, the removal in `tools/synth-input.sh` and its runner entry
  here are removed in one change.

## Domain relations

- `weft.workloads.Service` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Service`, relation `workload`.
- `weft.workloads.Instance` -> `weft.workloads.Workload`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Instance`, relation `workload`.
- `weft.scheduling.SchedulingRequest` -> `weft.workloads.Instance`, many-to-one, references; the
  instance outlives its request — inferable: `ess/5-runtime/scheduling.yaml`, entity
  `weft.scheduling.SchedulingRequest`, relation `instance`.

## Scope

- `crates/weft-controllers/` (`src/lib.rs`, `src/replicas.rs`, `tests/main.rs` module `replicas`)
- `crates/weft-impl/` (`src/behaviours/`: `create_instance.rs`,
  `submit_scheduling_request.rs`; `src/bindings/`: the runner and its two `r1-*` entries)
- `crates/weft-bin/` (`src/main.rs`: the controller loop, `--tick-ms`)
- `crates/weft-testkit/` (`src/`: an in-process system on a seeded state file)

## Acceptance

1. Against the running control host, a service defined over HTTP with `replicas: 1` has, within
   2 s, one instance with `replica_index` 0 in `weft.workloads.InstanceList` and one pending request
   for that instance in `weft.scheduling.SchedulingRequests`.
2. In-process, from a seeded state holding that service and its live instance, a replayed
   `ReplicaRequested` for replica 0 makes `CreateInstance` answer `replica-live`; the log after the
   `seq` read before the replay holds a binding line `r1-create-replica` with refusal
   `replica-live` and result `dropped` and no `retried` line for it, and
   `weft.workloads.InstanceList` still holds one instance.
3. In-process, from a seeded state whose account quota for `cpu` is used up, a `ReplicaRequested`
   makes `CreateInstance` answer `over-quota`, a refusal off the drop list; the log after the `seq`
   read before it holds a binding line `r1-create-replica` with refusal `over-quota` and result
   `retried`, and no `dropped` line for it.
4. In-process, a replayed `InstanceCreated` of an instance whose scheduling request exists makes
   `SubmitSchedulingRequest` answer `already-requested`; the log after the `seq` read before the
   replay holds a binding line `r1-submit-scheduling-request` with refusal `already-requested` and
   result `dropped`, and `weft.scheduling.SchedulingRequests` still holds one request for it.
5. In-process, one `tick` on a seeded state whose service is at scale sends no `RequestReplica`
   (the log after the `seq` read before the tick holds no request line for it), while the same tick
   on a state whose service is below scale sends one (positive control).
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
