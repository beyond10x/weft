---
format: aep.planning-md/3
id: story:ports-spec-port-names
kind: story
status: draft
title: Every `external:` branch a port answers names its port, and a check holds it (S4)
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
scope:
- confidence: cited
  path: ess/3-resources/resources.yaml
- confidence: cited
  path: ess/4-applications/formations.yaml
- confidence: cited
  path: ess/5-runtime/claims.yaml
- confidence: cited
  path: ess/5-runtime/node.yaml
- confidence: cited
  path: ess/5-runtime/secrets.yaml
- confidence: inferred
  path: quality/spec-diff-acknowledgements.json
- confidence: inferred
  path: tools/check-ports-fixture
- confidence: cited
  path: tools/check-ports.sh
- confidence: inferred
  path: tools/check.d/40-ports.sh
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "what the specification declares for them". Design:
`docs/platform-ports.md` § 9, spec change S4. Depends on none.

## Context

- Every pattern-B branch (design § 4) starts its `external:` text with `port <name>: `:
  - `ess/5-runtime/secrets.yaml:410`, `:553`, `:884` (`secret-store`);
  - `ess/3-resources/resources.yaml:829` (`objects`);
  - `ess/4-applications/formations.yaml:1526` and `ess/5-runtime/claims.yaml:393` (`dns`);
  - `ess/5-runtime/claims.yaml:390` (`network`);
  - `ess/5-runtime/node.yaml:571` (`endpoints`).
- A check, `tools/check-ports.sh` in `task check` (a hook under `tools/check.d/`), fails on a
  `port <name>:` whose name is not one of the 16 ports of design § 2. POSIX sh; no Python.
- The plan of a synthesis then names the port beside each obligation
  (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:963-966`). This is the workaround for ESS gap
  27 (`story:ports-ess-gaps`).
- The change passes `ess specify validate --path ess --strict-requires` and is acknowledged with
  `tools/check-spec-diff.sh --acknowledge`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None: the change edits free texts of `external:` branches, no entity or relation.

## Scope

- `ess/5-runtime/secrets.yaml`, `ess/3-resources/resources.yaml`,
  `ess/4-applications/formations.yaml`, `ess/5-runtime/claims.yaml`, `ess/5-runtime/node.yaml`
  (the eight `external:` lines)
- `tools/check-ports.sh`, a hook in `tools/check.d/`, a fixture directory

## Acceptance

`tools/check-ports.sh` passes on the tree and turns to failing on a fixture whose `external:` names
a port outside the 16 of design § 2.

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
