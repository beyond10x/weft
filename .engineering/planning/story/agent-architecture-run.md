---
format: aep.planning-md/3
id: story:agent-architecture-run
kind: story
status: draft
title: The agent units of a vendored agenticspec fixture run on local from a formation standing in for its cloudspec translation
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-architecture-units
- depends_on: story:agent-run-local-job
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`, outcome 2 and acceptance 3: weft runs the agent units of a
cloudspec architecture. Depends on `story:agent-architecture-units` (the component origin and the
revision's architecture) and `story:agent-run-local-job` (an `Agent` Job and Schedule run on
`local`).

This story replaces the translator story first drafted here: after the owner's decision D5
(2026-11-28 about 02:14 CET, epic § Order) weft has no translator. The translation of an
`agenticspec-deployment/1` bundle into a cloudspec architecture is cloudspec's (cloudspec epic
`translation`); weft's side is running the result.

## Context

- Until cloudspec has an architecture format, a weft formation document stands in for cloudspec's
  translation of a bundle. The stand-in is written by hand per fixture, follows the unit table of
  `story:agent-architecture-units`, and sets each component's `origin` and the revision's
  `architecture`; when cloudspec's translator exists, its output replaces the hand-written file
  and this story's test reads that instead.
- Fixture: one bundle from agenticspec, vendored under `tests/fixtures/agenticspec/` with a lock
  file naming the agenticspec source commit and the bundle's digest; a fast test recomputes the
  digest and fails on a mismatch. First fixture: `scheduled-loops` packed `by_part` (two agent
  parts: `sweeper` as a `scheduled` unit, `triager` as an `agent` unit with 2 replicas; agenticspec
  `architectures/scheduled-loops.yaml`). Until agenticspec publishes bundles, the fixture is the
  architecture document with a packing written beside it, and the lock names the architecture's
  commit and digest.
- On `local`, Job and Schedule components run (`story:agent-run-local-job`); a Service component
  (the `triager` unit) is defined and its instances wait in `Scheduled` until
  `story:ports-node-workloads-local` lets the node run a process until stopped.

## Scope

- `tests/fixtures/agenticspec/` (bundle or architecture, packing, lock, stand-in formation)
- `product/cloud/crates/weft-testkit/` (deploy a formation document with origins)
- `product/cloud/crates/weft-agent/tests/` (the run of the fixture's units)

## Acceptance

1. The fixture's stand-in formation deploys on `local`; `weft.formations.FormationComponents` lists
   one component per unit, each with `origin.object` naming its unit.
2. After the control host's clock is moved past the `sweeper` schedule's interval, the `sweeper`
   component has one `Succeeded` Job in `weft.workloads.Jobs`.
3. The lock test fails when one byte of the vendored fixture changes.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No Python in anything committed. No model call and no network request in the test step: the
  fixture is vendored, never fetched by a test.
