---
format: aep.planning-md/3
id: story:skeleton-conformance-suite
kind: story
status: draft
title: Conformance suite of the skeleton domains committed with a source digest and run by ess verify conform against the control host in the gate
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
- depends_on: story:skeleton-controllers
- depends_on: story:skeleton-node-join-session
- depends_on: story:skeleton-node-run-assignment
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: conformance
- confidence: cited
  path: crates/weft-bin/src/main.rs
- confidence: cited
  path: crates/weft-bin/src/test_routes.rs
- confidence: cited
  path: crates/weft-conform-target
- confidence: cited
  path: tools/check-conform-counts.sh
- confidence: cited
  path: tools/check.d/20-conform.sh
- confidence: cited
  path: tools/conformance-suite.sh
- confidence: cited
  path: tools/slow.d/20-conform-regenerate.sh
revision: 10
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 7. Depends on `story:skeleton-controllers`,
`story:skeleton-node-join-session` and `story:skeleton-node-run-assignment`, the last stories that
implement obligations of the path, so the held counts do not move under later stories of this epic.
Blocked by no ESS gap of the Rust target: the suite is synthesized from the unmodified `ess/`. G5
applies (below).

## Context

Decision A7 (`docs/implementation.md`): the ESS conformance runner against the HTTP surface of weft; the
suite committed with a digest of its sources and regenerated in the `slow` lane. This is the
committed-generated-file form of `~/beyond10x/AGENTS.md` § Tests stay fast: synthesis never runs in
the gate.

- How the runner reaches a target (`ess verify conform run --help`, ESS 0.99.2): `--target process
  -- <command>` starts a program that answers `ess-conformance-target/1`, one JSON request per line
  on standard input and one answer per line on standard output; `ess verify conform serve --target
  billing` is the reference peer for an adapter. There is no HTTP target, so this story writes the
  adapter.
- `crates/weft-conform-target`: the adapter. It starts one test build of `weft-bin` on a temporary
  state file, and for each request of the protocol: a command or a view goes over HTTP through
  `weft-client` to the control host, as the actor and caller the scenario names; a setup, reset or
  time request goes to test-only routes of `weft-bin` under `/__weft/test/` (reset the store, write
  arranged rows through the storage port, set the context clock, answer an `external:` branch a
  scenario forces through the decider), compiled only with the cargo
  feature `test-callers` of `story:skeleton-control-host` and absent from a build without it. A
  `501` whose body names an unmet obligation is answered as not supported by the target, so an
  obligation off the skeleton path counts as not executed, never as passed or failed.
- Suite: the domains of the skeleton path, `weft.infrastructure`, `weft.accounts`, `weft.cluster`,
  `weft.formations`, `weft.artifacts`, `weft.workloads`, `weft.scheduling`, `weft.nodes`, with
  `weft.shared` and `weft.actors`, cut from the unmodified `ess/` into a subset tree by
  `tools/conformance-suite.sh` (POSIX sh, as `tools/synth-domain.sh` does) and synthesized with
  `ess verify conform synthesize`. The subset tree, the suite and `conformance/SOURCES.sha256` (the
  digest of the subset tree and the script) are committed.
- G5: synthesis of the `workloads` subset ran over 30 min (`docs/ess-gaps.md` gap 24). The script
  synthesizes once, timed, through `slot --timeout 30m` with `--jobs 2`, one synthesis at a time
  (2026-11-26: one held 22 GB). If it exceeds the 30-minute bound, the suite is cut to the domains
  that synthesize within it, and the report names the time and the domains cut.
- Check hook `tools/check.d/20-conform.sh`, in this order:
  1. the digest check of the suite sources (fast, fails when a source changed without
     regeneration);
  2. `timeout ${CONFORM_LIMIT:-110} ess verify conform run --suite <committed suite> --target process
     --report-format 2 --report-out target/conform/report.json -- target/debug/weft-conform-target`.
     The step passes only on exit 0 of that run; exit 124 (the bound) or any other non-zero exit
     fails it. The step is timed; over 120 s it is a defect of this story;
  3. `tools/check-conform-counts.sh` (POSIX sh, `yq -p json`): prints the counts of the report
     (executed, passed, failed, not supported) and fails when they differ from
     `conformance/counts.toml`.
- Slow hook `tools/slow.d/20-conform-regenerate.sh`: regenerates the subset tree and the suite and
  compares bytes with the committed files, run only when its sources changed.

## Domain relations

None assumed: the suite is synthesized from `ess/` as written; this story writes no entity.

## Scope

- `conformance/` (subset tree, suite, `SOURCES.sha256`, `counts.toml`)
- `crates/weft-conform-target/`
- `crates/weft-bin/` (`src/test_routes.rs` behind the feature `test-callers`)
- `tools/conformance-suite.sh`, `tools/check-conform-counts.sh`
- `tools/check.d/20-conform.sh`, `tools/slow.d/20-conform-regenerate.sh`

## Acceptance

1. `slot task check` fails when a source of the committed conformance suite changed without
   regeneration.
2. The check hook passes only when `ess verify conform run` against `weft-conform-target` exits 0
   within `timeout 110`, and fails on its exit 124 (shown with `CONFORM_LIMIT=1`).
3. The check hook prints the executed, passed, failed and not-supported counts of the run, and
   they equal the values `conformance/counts.toml` holds.
4. In the report of that run, every scenario of a command of the skeleton path is executed and none
   of them is counted not supported: `CreateCloud`, `DeclareZone`, `DefinePriorityClass`,
   `RegisterRuntime`, `CreateNodePool`, `ProvisionPoolNode`, `JoinNode`, `RecordNodeObservation`,
   `OpenNodeSession`, `CreateDefaultCluster`, `CreateTenant`, `CreateAccount`, `GrantTopQuota`,
   `CreateFormation`, `AddComponent`, `CommitRevision`, `CreateEnvironment`, `RegisterArtifact`,
   `DefineWorkload`, `DefineService`, `RequestReplica`, `CreateInstance`, `SubmitSchedulingRequest`,
   `PlaceInstance`, `BindPlacement`, `Assign`, `ScheduleInstance`, `RecordPlacementAssigned`,
   `RecordArtifactVerified`, `RecordInstanceRunning`, `RecordBackendReady` and `RestoreBackend`
   (where G5 cut a domain from the suite, the report of this story names the commands that left
   with it).
5. In the same report, the scenarios of `weft.cluster.CreateCluster`, an obligation off the path,
   are counted not supported, and none of them failed.
6. With the test-surface switch off (the value a build without the feature `test-callers` passes),
   `POST /__weft/test/reset` answers `404` and empties nothing; with it on, it empties the store
   (positive control). Both settings are checked in one test build, as the caller switch of
   `story:skeleton-control-host` is.
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
