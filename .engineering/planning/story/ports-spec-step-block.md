---
format: aep.planning-md/3
id: story:ports-spec-step-block
kind: story
status: draft
title: '`ess/components.yaml` lists every port step with its gap view and recording command (S5)'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-port-names
scope:
- confidence: cited
  path: ess/9-actors/actors.yaml
- confidence: cited
  path: ess/components.yaml
- confidence: inferred
  path: quality/spec-diff-acknowledgements.json
- confidence: inferred
  path: tools/check-ports-fixture
- confidence: cited
  path: tools/check-ports.sh
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "what the specification declares for them". Design:
`docs/platform-ports.md` § 9, spec change S5. Depends on `story:ports-spec-port-names`
(`tools/check-ports.sh`).

## Context

- `ess/components.yaml`, after the level-step block: a comment block "Platform ports" with one line
  per pattern-A step (design § 4), `gap view -> port operation -> recording command -> actor`, for
  N3, N5, N6, C1, C4, C5, C7, C8.
- Where the actor of the recording command is wrong for a cloud-layer step, the grant moves:
  `ReportBucketProvisioned` and `ReportFilesystemProvisioned` (a cloud resource; granted to
  NodeAgent at `ess/9-actors/actors.yaml:702-703`) are also granted to ControlPlane.
- `tools/check-ports.sh` checks that every view and command a line names exists in `ess/`.
- This is the workaround for ESS gap 28 (`story:ports-ess-gaps`).
- The change passes `ess specify validate --path ess --strict-requires` and is acknowledged with
  `tools/check-spec-diff.sh --acknowledge`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None: the block is a comment; the grants change actors, not relations.

## Scope

- `ess/components.yaml`, `ess/9-actors/actors.yaml`
- `tools/check-ports.sh`

## Acceptance

`tools/check-ports.sh` passes on the tree and turns to failing when a step line names a view or
command that `ess/` does not declare.

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
