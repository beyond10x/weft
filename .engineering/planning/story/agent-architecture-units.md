---
format: aep.planning-md/3
id: story:agent-architecture-units
kind: story
status: draft
title: 'Spec: an agent architecture arrives as a cloudspec architecture; weft keeps each component origin and the revision architecture'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-package-settings
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`, outcome 2 (weft runs the agent units of a cloudspec
architecture). The specification change only; `ess/` and `docs/domain-sketch.md` are written when
this story is implemented, not by the plan. Running is `story:agent-architecture-run`.

## Context

- The owner, 2026-11-28 about 02:08 CET: "an architecture from agenticspec is just a workload
  kind". About 02:14 CET (D5): every source translates into cloudspec; weft runs only cloudspec
  specifications (epic § Order).
- So weft gets no package kind and no source kind for the agenticspec bundle: cloudspec translates
  `agenticspec-deployment/1` into a cloudspec architecture (cloudspec epic `translation`), and weft
  runs that. Until cloudspec has an architecture format, a weft formation document stands in for
  it (`docs/domain-sketch.md` § 3).
- agenticspec's unit kinds and their mapping: agenticspec `design/deployment.md` K3, K7 (branch
  `design/ingress` at `7d489f0`).

## What weft holds of an agent architecture

The unit-to-component rule is cloudspec's translator's; weft relies on its result:

| unit kind | component in weft |
|---|---|
| `agent` | package `Agent`, shape `Service`, replicas = the unit's (`story:agent-package-settings`) |
| `job` | package `Agent`, shape `Job` |
| `scheduled` | package `Agent`, shape `Schedule` starting a `Job` |
| `service`, `core` | the unit's runtime artifact, shape `Service` |
| `model` | package `Model`, shape `Service`, with accelerator claims |
| `store` | a catalog managed-service binding or a volume claim |
| `external` | no component: an egress destination |

The specification change weft needs for it:

1. `ess/0-foundation/shared.yaml`: a value type `weft.shared.Origin`, struct `{source_digest:
   String, source_format: String, object: String}`: the digest and format of the document a
   component came from (for an agenticspec bundle: its digest and `agenticspec-deployment/1`) and
   the object path in it (`units.<unit>`, `parts.<part>`). The integrator adds it (repository rule:
   shared types go to `shared.yaml`).
2. `ess/4-applications/formations.yaml`, entity `weft.formations.Component` (line 233): a field
   `origin: Optional<weft.shared.Origin>`, set at creation and never changed; the view
   `weft.formations.FormationComponents` shows it. This is cloudspec's "source field on every
   object, kept into target objects and observations" (agenticspec design V3) on weft's side.
3. `ess/4-applications/formations.yaml`, entity `weft.formations.FormationRevision`: a field
   `architecture: Optional<weft.shared.ArtifactDigest>`, the agenticspec architecture document of
   the revision, stored as an artifact, the input of the console's drawing
   (`story:console-agent-architecture`).
4. `docs/domain-sketch.md` § 4 "Package: spec kinds" (lines 235-247): one sentence that an agent
   architecture arrives as a cloudspec architecture whose agent units are `Agent` packages, and
   that weft keeps each component's origin.

No change to `weft.workloads.PackageKind` or `weft.deployments.SourceKind`.

## Scope

- `ess/0-foundation/shared.yaml`, `ess/4-applications/formations.yaml`, `docs/domain-sketch.md`

## Acceptance

`ess specify validate --path ess --strict-requires` passes with `origin` on
`weft.formations.Component` and `architecture` on `weft.formations.FormationRevision`, and
`weft.formations.FormationComponents` returns a component's `origin` as it was created.
