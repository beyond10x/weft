---
format: aep.planning-md/3
id: story:deployments-admit-serve
kind: story
status: draft
title: 'The control host answers a dry-run admit: Admitted with a plan or Refused with every missing item, writing nothing'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:deployments-admit
revision: 2
---
## Epic

Decomposes `epic:agent-workloads`. Serves the dry-run admission specified by
`story:deployments-admit` on the control host, so a consumer asks before anything starts.

## Context

- A consumer's deployment plan (relayed by the agentic operator, 2026-11-28, row R36) asks for a
  dry-run admission that answers Admitted with a plan or Refused with the missing items, before
  anything starts.
- `story:deployments-admit` adds the command to `ess/6-delivery/deployments.yaml`; the control
  host's routes are generated per command from the specification (`weft-bin/src/lib.rs`), so this
  story writes the handler behind the generated route and the answer it computes.
- Admission reads the formation, the binding and the placement's served ports
  (`cloudspec@2ed72c2:docs/platform-ports.md`), and writes nothing.

## Scope

- `product/cloud/crates/weft-control/` (the admit handler), `product/cloud/crates/weft-testkit/`

## Acceptance

On a control host with one local node, the admit route answers Admitted with a plan for a formation
that fits, and Refused naming `NoMatchingResource` and each missing port for one that claims an
accelerator no node has; after both calls the event log has no new record.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed. No network request beyond
  loopback in the test step.
