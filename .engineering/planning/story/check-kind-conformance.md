---
format: aep.planning-md/3
id: story:check-kind-conformance
kind: story
status: draft
title: A health check kind Conformance runs a suite against a workload and gates Ready on it
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`. A health check kind that runs a conformance suite against a
workload, so a workload is Ready only when it answers its own specification.

## Context

- A consumer's deployment plan (relayed by the agentic operator, 2026-11-28, row R40) asks for a
  health check kind that runs a conformance suite against a workload.
- `weft.workloads.CheckKind` has `Http`, `Tcp`, `Command` and `Grpc`
  (`ess/4-applications/workloads.yaml:166`); a check names what its kind needs.
- The new kind `Conformance` names a suite artifact by digest and the workload port it runs
  against; the node runs the suite as a confined exec beside the instance and reads its report
  (`ess-conformance-report`); the check passes when the report has no failed scenario.
- Role `Ready` with this kind keeps an instance out of its backends until the suite passes.

## Scope

- `ess/4-applications/workloads.yaml` (the kind and its fields), `product/cloud/crates/weft-agent/`
  (running the suite), `product/cloud/crates/weft-testkit/`

## Acceptance

`ess specify validate --path ess --strict-requires` passes with `CheckKind` `Conformance`; on
placement local, an instance whose suite passes reads Ready, and one whose suite has a failed
scenario stays not Ready with the failed scenario's name in its check status.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed. No network request beyond
  loopback in the test step.
