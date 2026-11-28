---
format: aep.planning-md/3
id: story:cloudspec-submission-pin
kind: story
status: draft
title: CLOUDSPEC_REV moves to a cloudspec revision with SourceKind::Cloudspec and the cloudspec-sim view-name fix, and weft follows the new block fields
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: dependency-blocker:cloudspec-sim-renamed-views
- depends_on: story:cloudspec-submission-reader
- depends_on: story:cloud-blocks-vendor
- depends_on: story:cloud-blocks-lock-gate
- depends_on: story:cloud-blocks-layers
- depends_on: story:cloud-blocks-rust
- depends_on: story:cloud-blocks-consoles
- depends_on: story:cloud-blocks-docs
- depends_on: story:cloud-blocks-sim-pin
scope:
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: ess/cloud
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: product/cloud/crates/weft-cloudspec/tests
- confidence: inferred
  path: product/cloud/crates/weft-controllers/src/placement.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/create_formation.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/define_workload.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/place_instance.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/src/steps.rs
- confidence: inferred
  path: product/console/ui/fixtures
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
- confidence: inferred
  path: sim/experiments
- confidence: cited
  path: tests/fixtures/cloudspec
revision: 4
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 1: weft's vendored blocks move to a cloudspec
revision that holds `SourceKind::Cloudspec` (fdcf7e8 or later) and the cloudspec-sim view-name fix,
in one pin move (decision D-f of `epic:cloud-blocks`: one pin, `CLOUDSPEC_REV`, for cloudspec-sim and
the blocks).

## Dependencies

- `dependency-blocker:cloudspec-sim-renamed-views` cleared: cloudspec-sim at a485b7f reads view names
  from before the rename, so the closed-link experiment exits 2. The revision this story pins is the
  one that clears it, and it is named there; this story names no sha of its own.
- The revision must not carry cloudspec's layer refactor (out of the epic's scope: a later pin
  move with its rename file). If the first revision with both pieces also carries the refactor, the
  story stops and says so.
- `story:cloudspec-submission-reader`: the goldens and their lock exist.
- The `epic:cloud-blocks` stories that own the same files: `cloud-blocks-vendor`,
  `cloud-blocks-lock-gate`, `cloud-blocks-layers` (`Taskfile.yml`, `ess/cloud/`), `cloud-blocks-rust`
  (the crates), `cloud-blocks-consoles` (console fixtures), `cloud-blocks-docs`
  (`quality/spec-diff-acknowledgements.json`), `cloud-blocks-sim-pin` (`sim/experiments/`).

## What changes in the blocks

Read from `git diff a485b7f fdcf7e8 -- blocks` in cloudspec (7 files); the pinned revision may add
more, which this story follows the same way:

- `0-protocols/shared.yaml`: `IsolationLevel` gains `None`, ordered first (weakest).
- `2-infrastructure/infrastructure.yaml`: the isolation policy refusals take `None`; new refusal
  `weakened-from-process-sandbox`.
- `3-scope/scope.yaml`: `Scope.placement`, input `placement` of `OpenScope`, refusal `placement-form`.
- `4-resources/artifacts.yaml`, `5-workloads/workloads.yaml`: package kinds `WebBundle`,
  `BuildContext`; `DefineWorkload` inputs `run_as`, `upgrade`, `volumes`, `ports`; refusal
  `run-as-missing`.
- `9-composition/architecture.yaml`: `LinkProtocol::Ssh`, `Link.data`, `Formation.parity`; input
  `parity` (a required list) of `CreateFormation` with refusal `parity-form`; input `data` of `AddLink`.
- `11-translation/deployments.yaml`: `SourceKind` gains `Cloudspec`.

## Outcome

1. `task vendor-update REV=<the revision>`: `ess/cloud/` and `ess/cloud/cloud.lock` at it, and
   `CLOUDSPEC_REV` in `Taskfile.yml` equal to it in the same commit. A file the new lock adds is
   listed in `ess/ess-inputs.yaml` by hand. No file under `ess/cloud/` is edited by hand.
2. weft follows the block changes above wherever the gate needs it: the hand-written behaviours
   answer the new refusals in their contract's order (`define_workload.rs`: `run-as-missing`;
   `create_formation.rs`: `parity-form`), the placement code treats `None` as the weakest level
   (default deny), the skeleton's steps send `parity: []`, and the simulator inputs and console
   fixtures carry the new required fields.
3. `tests/fixtures/cloudspec/`: the three goldens copied again at the new revision, the lock's
   `commit` set to `CLOUDSPEC_REV`, and the lock test of `weft-cloudspec` fails when the two differ.
4. `quality/spec-diff-acknowledgements.json` acknowledged again (`tools/check-spec-diff.sh
   --acknowledge`) after the change is reviewed.

## Scope

`Taskfile.yml` (`CLOUDSPEC_REV` only), `ess/cloud/`, `ess/ess-inputs.yaml`,
`quality/spec-diff-acknowledgements.json`, `tests/fixtures/cloudspec/`,
`product/cloud/crates/weft-cloudspec/tests/`; worked out, not read in a diff:
`product/cloud/crates/weft-impl/src/behaviours/define_workload.rs`,
`product/cloud/crates/weft-impl/src/behaviours/create_formation.rs`,
`product/cloud/crates/weft-impl/src/behaviours/place_instance.rs`,
`product/cloud/crates/weft-controllers/src/placement.rs`, `product/cloud/crates/weft-testkit/src/steps.rs`,
`sim/experiments/`, `product/console/ui/fixtures/`.

## Acceptance

With `CLOUDSPEC_REV` at the revision that cleared `dependency-blocker:cloudspec-sim-renamed-views`,
`slot task check` passes and `task sim-experiments` exits 0.

Checked by:

1. `tools/check-vendor-lock.sh` exits 0 and `ess/cloud/cloud.lock` names `CLOUDSPEC_REV`;
   `weft.deployments.SourceKind` in `ess/cloud/11-translation/deployments.yaml` has the variant
   `Cloudspec`.
2. `ess specify validate --path ess --strict-requires` passes.
3. `slot task check` passes; its test step ends within 120 s, and
   `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` passes.
4. `task sim-experiments` and `task sim-relations` exit 0 (slow lane; they fetch), the acceptance
   `story:cloud-blocks-sim-pin` could not meet at a485b7f.
5. The goldens lock names `CLOUDSPEC_REV`, and the lock test fails on a lock naming another commit.

## Rules for this work

- Builds and tests run through slot. No test over 60 s; the test step ends within 120 s. A pool job
  ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). `task
  vendor-update`, `task sim-*` and the slow lane fetch and never run in `task check`.
- No network request, no cloudspec binary in the test step. No Python in anything committed.
- One owner per domain file; nothing under `ess/cloud/` is edited by hand.
