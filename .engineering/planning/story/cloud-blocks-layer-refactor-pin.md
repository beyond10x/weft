---
format: aep.planning-md/3
id: story:cloud-blocks-layer-refactor-pin
kind: story
status: implemented
title: The pin moves to cloudspec's layer refactor and scope-isolation guard (59129eb)
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloudspec-submission-pin
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: ess
- confidence: cited
  path: product
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
- confidence: cited
  path: sim
- confidence: cited
  path: tests/fixtures
- confidence: cited
  path: website
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T11:56:16Z", actor: "human:operator", revision: 9, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T11:56:16Z", actor: "human:operator", revision: 10, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T14:44:24Z", actor: "human:operator", revision: 11, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the pin moves to cloudspec's layer refactor and scope-isolation
guard (cloudspec#14, main 59129ebfcdbef65bbde498b749fb91c6ff5563b2).

## What changes in the blocks (cloudspec's own summary)

- Renames: `docs/renames/2026-11-28-layers.json`, 17 old -> new under `cloud.` (edge Route,
  AddRoute, RemoveRoute, ComponentRoutes, FormationBorder and others -> architecture; edge
  NoSuchFormation -> NoSuchScope, FormationStateConflict -> ScopeStateConflict; identity
  FormationNotInScope -> IdentityOutsideScope; services FormationServiceBindings ->
  WorkloadServiceBindings). Removed with no successor: services FormationNotFound.
- Blocks below layer 9 name a formation, environment or component by workload-identity path
  (`shared.WorkloadIdentity`); new architecture ComponentWorkload, AssignWorkload,
  FormationWorkloads; inventory NodeCapacity split out of infrastructure Node.
- New `scope_id` input: architecture AddLink, AddToBorder, CreateConfig, MountConfig,
  SetConfigOverride; deployments RecordCoverageEntry; workloads SetWorkloadReplicas, WakeService.
- SubmitSchedulingRequest: guards no-such-instance and instance-of-other-scope (NotFound),
  identity-not-instances (RequestMismatch) with Optional input `identity`; two hand-written
  scenarios in `blocks/scenarios/`.
- Behaviour: endpoint removal no longer refused by routes; service binding per environment; box
  created before its endpoint; service instances have no system formation; telemetry policy unique
  per path.

## Outcome

1. `task vendor-update` at 59129eb; `cloud.lock` and `CLOUDSPEC_REV` name it.
2. weft's own files, Rust, consoles, sim inputs and docs apply the rename file with a script
   (`cloud.` -> `weft.`), and follow the signature and behaviour changes.
3. FormationBorder and the routes now live in weft.architecture: the account network console page
   gets its border-route section back if its reads pass `ess ui check` (the endpoint section stays
   removed while Endpoints stay in weft.edge).
4. weft keeps its own PlaceInstance guards instance-of-other-scope and instance-of-other-formation.

## Scope

`Taskfile.yml`, `ess/cloud/`, `ess/`, `product/`, `sim/`, `website/`, `tests/fixtures/`,
`quality/spec-diff-acknowledgements.json`.

## Acceptance

1. `slot --here --timeout 30m task check` exits 0 with `cloud.lock` at 59129eb.
2. `task sim-experiments` and `task sim-relations` exit 0.
3. `tools/check-vendor-lock.sh` names 59129eb.
