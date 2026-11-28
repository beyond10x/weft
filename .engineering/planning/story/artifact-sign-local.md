---
format: aep.planning-md/3
id: story:artifact-sign-local
kind: story
status: draft
title: A local run signs its artifacts with a key held on this machine and registers them
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:ports-cloud-secret-store-local
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`. A local run registers its own artifacts: weft signs them with a
key held on this machine and trusts that key for the run's account.

## Context

- A consumer's deployment plan (relayed by the agentic operator, 2026-11-28, row R39) asks for
  signing an artifact with a key held on one machine, so a local run can register it.
- `weft.artifacts.RegisterArtifact` refuses an artifact with no signature and one whose signature
  does not verify against a key the account trusts (`ess/3-resources/artifacts.yaml`, errors
  `unsigned` and `bad-signature`).
- The `secret-store` port designs `create_key() -> key_ref` and `sign(key_ref, bytes)`
  (`story:ports-cloud-secret-store-local`); its local adapter keeps the key in the state directory.
- `weft artifact sign <file>` prints the digest, signature and key id; `weft run` signs and
  registers what it runs with the same key.

## Scope

- `product/cloud/crates/weft-platform/src/local/` (key custody), `product/cloud/crates/weft-bin/`
  (`artifact sign`), `product/cloud/crates/weft-testkit/`

## Acceptance

On placement local, a file signed by `weft artifact sign` registers through `RegisterArtifact`;
the same file with one byte changed is refused with `bad-signature`; the private key appears in no
log line and in no file outside the state directory.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed. No network request beyond
  loopback in the test step.
