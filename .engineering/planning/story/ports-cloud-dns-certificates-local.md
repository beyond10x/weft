---
format: aep.planning-md/3
id: story:ports-cloud-dns-certificates-local
kind: story
status: draft
title: '`dns` for `.localhost` names and `certificates` from a local CA'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-cloud-edge-local
scope:
- confidence: cited
  path: product/cloud/crates/weft-platform/src/cloud.rs
- confidence: cited
  path: product/cloud/crates/weft-platform/src/local
revision: 2
---
## Epic

Decomposes `epic:platform-ports`, epic scope "other ports designed, not built". Design:
`docs/platform-ports.md` § 6 C3 (`dns`) and C4 (`certificates`), § 8 (`local`: names under
`.localhost` only, proved by their form; a CA weft makes in the state directory of the control
host). Depends on `story:ports-cloud-edge-local`.

## Context

- `dns`: `challenge(host, account)`, `verify(host, account) -> Proved | NotProved`,
  `upsert(host, address)`, `remove(host)`, `observe(host)`; refusals `NotProved`,
  `ZoneNotServed`, `NotServed`, `Unavailable`; keyed by host. Serves
  `formations.AddEndpoint/host-not-verified` (`ess/4-applications/formations.yaml:1524-1527`) and
  `claims.DeclareClaim/domain-not-verified` (`ess/5-runtime/claims.yaml:392-394`).
- `certificates`: `issue(certificate_id, host)`, `observe(certificate_id) -> Pending | Issued
  {not_after} | Failed {reason}`, `retire(certificate_id)`; refusals `HostNotProved`,
  `IssuerRefused { reason }`, `NotServed`, `Unavailable`; keyed by `certificate_id`. The end of
  validity is the issuer's (`ess/5-runtime/edge.yaml:595-596`). Serves `edge.RequestCertificate`,
  `RetryCertificate`, `RetireEndpointCertificates`; recorded by `RecordCertificateIssued` and
  `RecordCertificateFailed`.
- How an account proves it controls a host is OPEN in the specification
  (`ess/4-applications/formations.yaml:1504-1505`), and so is the issuer
  (`ess/5-runtime/edge.yaml:153-154`); on `local` the design answers both by the `.localhost` form
  and a local CA only.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- `weft.edge.EdgeCertificate` -> `weft.formations.Endpoint`, many-to-one, references — inferable:
  `ess/5-runtime/edge.yaml`, entity `weft.edge.EdgeCertificate`, relation `endpoint`.

## Scope

- `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/`

## Acceptance

`AddEndpoint` of a `.localhost` host is accepted and another host refused `host-not-verified`, and
the endpoint's certificate moves `Requested` -> `Issued` through `RecordCertificateIssued`.

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
