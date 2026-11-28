---
format: aep.planning-md/3
id: story:ports-node-images-local
kind: story
status: draft
title: 'The `images` port: artifact bytes to a root filesystem'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-cloud-objects-local
scope:
- confidence: cited
  path: product/cloud/crates/weft-agent
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local/substrate.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/node.rs
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`cloudspec@2ed72c2:docs/platform-ports.md` § 5 N6 (`images`), § 8, § 10 gap 5. Depends on
`story:ports-cloud-objects-local` (the bytes come from the `objects` port). Outside the first
build: it starts when substrate makes a root filesystem from a content-addressed archive without a
Docker daemon (gap 5).

## Context

- Operations: `prepare(digest, kind) -> root ref` (bytes from C5 `objects`, digest checked on the
  node), `release(root ref)`, `observe(digest)`. Kinds: the package kinds of
  `weft.artifacts.ArtifactKind` (`ess/3-resources/artifacts.yaml:14-30`); an adapter names the kinds
  it serves. The first build serves kind `Snippet` only (design § 8).
- Refusals: `DigestMismatch` (recorded as `workloads.RecordArtifactRejected`), `KindNotServed`,
  `NotFound`, `Unavailable`. Keyed by digest.
- Serves: `workloads.RecordArtifactVerified` (`ess/4-applications/workloads.yaml:2483`).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.workloads.Instance` -> `weft.artifacts.Artifact`, many-to-one, references — inferable:
  `ess/4-applications/workloads.yaml`, entity `weft.workloads.Instance`, relation `artifact`.

## Scope

- `product/cloud/crates/weft-platform/src/node.rs`, `src/local/substrate.rs`
- `product/cloud/crates/weft-agent/`

## Acceptance

An `Image` artifact whose bytes do not match its digest is recorded with `RecordArtifactRejected`,
and its instance never moves `Scheduled` -> `Starting`.

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
