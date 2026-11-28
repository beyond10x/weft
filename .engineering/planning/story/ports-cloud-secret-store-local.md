---
format: aep.planning-md/3
id: story:ports-cloud-secret-store-local
kind: story
status: draft
title: 'The `secret-store` port: `ClusterStore` and signing keys'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-spec-port-names
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/cloud.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local
revision: 3
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`cloudspec@2ed72c2:docs/platform-ports.md` § 6 C6 (`secret-store`), § 8 (`local`: `ClusterStore`, an encrypted file
in the state directory of the control host). Depends on `story:ports-spec-port-names` (the
`port secret-store:` texts of the three branches).

## Context

- Operations: `probe(backend)`, `put(value) -> value_ref` (written once), `get(value_ref)` (only
  for a delivery), `disable(value_ref)`; keys: `create_key() -> key_ref`, `sign(key_ref, bytes)`,
  `public_key(key_ref)`.
- Backends: `SecretBackendKind` `ClusterStore | ExternalVault` (`ess/5-runtime/secrets.yaml:72-87`).
  The model holds references only; a value is written once and never returned by the API, and never
  appears in logs or events (`docs/domain-sketch.md:421-427`).
- Refusals: `BackendUnavailable` (the `external:` branches `ess/5-runtime/secrets.yaml:408-411`,
  `:551-554`, `:882-885`), `ValueDisabled`, `NotServed`. `put` keyed by `(secret_id, version_id)`;
  `create_key` by the trust domain.
- Serves: `RegisterSecretBackend`, `AddSecretVersion`, `DeliverSecret` (pattern B),
  `identity.CreateTrustDomain`, `RotateSigningKey`, `IssueIdentityDocument` (key custody).
- Delivery of a value to one run at its start goes through N3 and waits for substrate gap 4
  (design § 10).
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.secrets.Secret` -> `weft.secrets.SecretVersion`, one-to-many, the secret owns its versions —
  inferable: `ess/5-runtime/secrets.yaml`, entity `weft.secrets.Secret`, relation `versions`.
- `weft.secrets.Secret` -> `weft.secrets.SecretBackend`, many-to-one, references — inferable:
  `ess/5-runtime/secrets.yaml`, entity `weft.secrets.Secret`, relation `backend`.
- `weft.secrets.SecretDelivery` -> `weft.workloads.Instance`, many-to-one, references — inferable:
  `ess/5-runtime/secrets.yaml`, entity `weft.secrets.SecretDelivery`, relation `instance_id`.

## Scope

- `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/`

## Acceptance

A value added with `AddSecretVersion` (version `Enabled`) is delivered once at start, one
`SecretDelivery` being recorded, and appears in no log line.

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
