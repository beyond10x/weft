---
format: aep.planning-md/3
id: epic:cloud-blocks
kind: epic
status: draft
title: weft's spec takes cloudspec's blocks as a pinned, renamed copy and keeps only its run-time records and its example
relations:
- serves: vision:weft
revision: 3
---
## Order

The owner, 2026-11-28: cloudspec is the lowest common representation; "the idea was to move the
stuff from weft [...] to Cloud spec" (relayed by the host operator about 03:3x CET). D2 A: a
consumer vendors cloudspec's blocks as a renamed copy at a pinned commit. D7 A: cloudspec holds the
design-time pieces; weft keeps its run-time records (`Instance`, `PlacementDecision`,
`SecretDelivery`, `Release`, `ReleaseStep`, `Drift`, `Alert`, `Engine`, `OfferingPlan`, the actors
`Scheduler`, `NodeAgent`, `ControlPlane`). The cloudspec session sent removal request R3 with the
pin 7f22943; its `vendor.sh` gained `wire` renaming and `--exclude` in e817a3b (cloudspec
integration PR 7), and the pin is the cloudspec `main` commit that carries it.

## Outcome

- `ess/cloud/` holds cloudspec's blocks renamed `cloud.` to `weft.` by cloudspec's
  `scripts/vendor.sh`, with `cloud.lock`; weft's own files hold only what weft keeps.
- `ess specify validate --path ess --strict-requires` passes on the combined system.
- The gate checks the lock offline; a slow lane and a CI job re-run the copy from source.
- The hand-written Rust, the consoles, the simulator inputs and the docs follow the renamed
  domains; the walking skeleton's end-to-end test stays green.

## Decisions taken on default (reversible on the branch)

Assessment: `move/vendor-cloud-blocks` trial, 179 validation errors to 30 after the renames.
- D-a: the copy leaves out `13-actors/actors.yaml` and `components.yaml` (`vendor.sh --exclude`,
  recorded in the lock; cloudspec adds the flag in its PR 7); weft's actors keep
  every grant, including the 80 commands whose only actor stayed with weft.
- D-b: weft's kept records stay in the vendored domains (a domain may span files).
- D-c: cloudspec is asked to have `vendor.sh` rename `wire: cloud` too, so paths stay `/weft/`.
- D-d: `weft.accounts.Account` keeps `AccountId` and gains a unique `scope_id`; rows the blocks key
  by scope relate to Scope; account-only rows keep `account_id`.
- D-e: weft records which cluster a formation runs in; `PlaceInstance` keeps its three isolation
  refusals (default deny).
- D-f: one pin, `CLOUDSPEC_REV`, for cloudspec-sim and the blocks.

## Scope

`ess/`, `tools/` (lock gate, layers, domain map, docs pages), `Taskfile.yml`, `.github/workflows/`,
`AGENTS.md`, `docs/layers.md`, `product/cloud/crates/` (weft-impl,
weft-controllers, weft-testkit, weft-client, weft-bin, weft-agent), `product/console/`, `sim/`, `website/docs/`,
`quality/spec-diff-acknowledgements.json`, `quality/ratchet.yaml`. The integration branch starts
from weft#33, where `tools/sim` is gone and `CLOUDSPEC_REV` exists.

Out of scope: entities open under D7 in cloudspec; the history files under `docs/` (hardening
passes, open questions) keep the old names they were written with.

## Acceptance

- `slot task check` passes on one integration branch with `ess/cloud/cloud.lock` at the pin.
- The skeleton test `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running`
  passes on that branch.
