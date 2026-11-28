---
format: aep.planning-md/3
id: story:cloud-blocks-vendor
kind: story
status: active
title: ess/cloud holds cloudspec's renamed blocks at the pin, and weft's files keep only what weft keeps
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: ess/0-foundation/shared.yaml
- confidence: cited
  path: ess/1-infrastructure/infrastructure.yaml
- confidence: cited
  path: ess/1-infrastructure/inventory.yaml
- confidence: cited
  path: ess/2-tenancy/access.yaml
- confidence: cited
  path: ess/2-tenancy/accounts.yaml
- confidence: cited
  path: ess/3-resources/artifacts.yaml
- confidence: cited
  path: ess/3-resources/cluster.yaml
- confidence: cited
  path: ess/3-resources/resources.yaml
- confidence: cited
  path: ess/4-applications/formations.yaml
- confidence: cited
  path: ess/4-applications/workloads.yaml
- confidence: cited
  path: ess/5-runtime/claims.yaml
- confidence: cited
  path: ess/5-runtime/edge.yaml
- confidence: cited
  path: ess/5-runtime/identity.yaml
- confidence: cited
  path: ess/5-runtime/nodes.yaml
- confidence: cited
  path: ess/5-runtime/observability.yaml
- confidence: cited
  path: ess/5-runtime/scheduling.yaml
- confidence: cited
  path: ess/5-runtime/secrets.yaml
- confidence: cited
  path: ess/6-delivery/deployments.yaml
- confidence: cited
  path: ess/6-delivery/pipelines.yaml
- confidence: cited
  path: ess/7-services/catalog.yaml
- confidence: cited
  path: ess/8-economy/metering.yaml
- confidence: cited
  path: ess/9-actors/actors.yaml
- confidence: cited
  path: ess/cloud
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: tools/synth-domain.sh
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:22Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T08:38:23Z", actor: "human:operator", revision: 9, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the copy goes in, the pin moves with it, and weft's duplicates
come out.

## Outcome

1. `ess/cloud/` is the output of cloudspec's `scripts/vendor.sh` at the pin (cloudspec a485b7f, the merge of
   its integration PR 7, which carries e817a3b), renamed to `weft`,
   without `13-actors/actors.yaml` and `components.yaml` (D-a, `--exclude`), with `wire: weft`
   (D-c), and `ess/cloud/cloud.lock`.
2. `CLOUDSPEC_REV` in `Taskfile.yml` changes from 2ed72c2 to the same pin in the same commit (D-f), so the lock
   and the pin never differ on the branch.
3. `ess/ess-inputs.yaml` lists every file of `ess/cloud/`; `ess/system.yaml` lists the 5 new
   domains (architecture, topology, scope, limits, services).
4. Deleted, their domains moved whole: `1-infrastructure/inventory.yaml`,
   `3-resources/artifacts.yaml`, `3-resources/resources.yaml`, `5-runtime/claims.yaml`.
5. Trimmed to what weft keeps under D7 A and renamed to the vendored names: `0-foundation/shared.yaml`,
   `1-infrastructure/infrastructure.yaml`, `2-tenancy/access.yaml`, `2-tenancy/accounts.yaml`,
   `3-resources/cluster.yaml`, `4-applications/formations.yaml`, `4-applications/workloads.yaml`,
   `5-runtime/edge.yaml`, `5-runtime/identity.yaml`, `5-runtime/nodes.yaml`,
   `5-runtime/observability.yaml`, `5-runtime/scheduling.yaml`, `5-runtime/secrets.yaml`,
   `6-delivery/deployments.yaml`, `6-delivery/pipelines.yaml`, `7-services/catalog.yaml`,
   `8-economy/metering.yaml`, `9-actors/actors.yaml`, `components.yaml`.
6. The mechanical renames the blocks force, in those files: `caller.account_id` to
   `caller.scope_id` (46 places in 10 files), the actor attribute `account_id` to `scope_id`
   (9 actors), the field `account_id` to `scope_id` where a weft row joins a scope-keyed block
   row, refusals the blocks cut dropped from binding policies, and the 4 Endpoint/Route commands
   moved from the `weft-api` to the `weft-edge` accepts.
7. `tools/synth-domain.sh` copies every file under `ess/cloud/` into its subset tree (the
   vendored layer is the lowest, so every domain may need it); the later stories run it.

## Scope

`ess/cloud/**`, `ess/ess-inputs.yaml`, `ess/system.yaml`, the 4 deleted and 19 trimmed files of
outcomes 4 and 5, `Taskfile.yml` (`CLOUDSPEC_REV` only), `tools/synth-domain.sh`. This story runs first; the later stories
edit these files after it.

## Acceptance

1. `vendor.sh --check ess/cloud`, run from cloudspec at `CLOUDSPEC_REV`, exits 0, and
   `ess/cloud/cloud.lock` reads `format cloudspec-vendor/2` with two `exclude` lines.
2. The 4 files of outcome 4 are absent, and every `output` path of `cloud.lock` appears in
   `ess/ess-inputs.yaml` (`grep -c` per path is 1).
3. `ess specify validate --path ess --strict-requires` reports no `duplicate_declaration` and no
   unknown name, and at most 30 errors, all of the kinds `unobservable_fact`,
   `conflicting_declaration`, `missing_declaration` and `undeclared_reference`, each on one of
   the four causes of the trial (Account and Scope share `ScopeId`, 8; tenant-cluster pinning, 7;
   `access_mode` and `control_plane`, 7; empty policies and `rollback_of`, 6), which
   `story:cloud-blocks-account-scope`, `-cluster-pinning`, `-runtime-fields` and `-policies`
   remove.
4. After `tools/synth-domain.sh accounts`, `.check/synth-accounts/ess/cloud/` holds every `output`
   of `cloud.lock` (the run may stop at its compile step while the errors of acceptance 3 remain).

## Dependencies

- cloudspec's integration PR 7 (`vendor.sh` writes `wire: <system>` and records `--exclude` in
  the lock): merged as a485b7f459abd66261af25b558fb92df90570545.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
