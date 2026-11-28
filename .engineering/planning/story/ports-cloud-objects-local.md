---
format: aep.planning-md/3
id: story:ports-cloud-objects-local
kind: story
status: draft
title: 'The `objects` port: buckets and the artifact store in a directory'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-step-block
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/cloud.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`cloudspec@2ed72c2:docs/platform-ports.md` § 6 C5 (`objects`), § 8 (`local`: a directory under the state directory
of the control host). Depends on `story:ports-spec-step-block` (the C5 step line and the grant of
`ReportBucketProvisioned` to ControlPlane).

## Context

- Operations: `create_bucket(bucket_id)`, `delete_bucket(bucket_id)` (refused while not empty),
  `is_empty(bucket_id)`, `put(bucket_id, key, bytes)`, `get(bucket_id, key)`, `list(bucket_id,
  prefix)`. The artifact store is a bucket of weft's own, keyed by digest; an artifact `put` of an
  existing digest is a no-op.
- Refusals: `NotEmpty`, `NameTaken`, `NotFound`, `QuotaExceeded`, `NotServed`, `Unavailable`.
- Serves: `resources.CreateBucket` (recorded by `ReportBucketProvisioned`),
  `DeleteBucket/not-empty` (pattern B, `ess/3-resources/resources.yaml:827-829`),
  `artifacts.RegisterArtifact`, `observability.ConfigureBucketSink`,
  `catalog.BackupServiceInstance`.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.resources.Bucket` -> `weft.accounts.Account`, many-to-one, references — inferable:
  `ess/3-resources/resources.yaml`, entity `weft.resources.Bucket`, relation `account`. The
  artifact store bucket is weft's own and no `ess/1` entity.

## Scope

- `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/`

## Acceptance

`DeleteBucket` of a bucket that holds an object answers `not-empty` and the bucket stays
`Available`, while an empty one moves `Available` -> `Deleted`.

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
