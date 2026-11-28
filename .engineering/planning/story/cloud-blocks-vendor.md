---
format: aep.planning-md/3
id: story:cloud-blocks-vendor
kind: story
status: draft
title: ess/cloud holds cloudspec's renamed blocks at the pin, and weft's files keep only what weft keeps
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: the copy goes in and weft's duplicates come out.

## Outcome

1. `ess/cloud/` is the output of cloudspec's `scripts/vendor.sh` at the pin, renamed to `weft`,
   without `13-actors/actors.yaml` and `components.yaml` (D-a), with `wire: weft` (D-c).
2. `ess/ess-inputs.yaml` lists every file of `ess/cloud/`.
3. The four weft files whose domains moved whole are deleted; the 15 split files and
   `components.yaml` keep only what weft keeps under D7 A (its run-time records, actors, engines,
   bindings and run-time admission) and refer to the vendored names.
4. The caller context carries `scope_id`.

## Scope

`ess/cloud/**`, `ess/ess-inputs.yaml`, `ess/system.yaml`, `ess/0-foundation/`..`ess/9-actors/`
(the trimmed files), `ess/components.yaml`.

## Acceptance

1. `vendor.sh --check ess/cloud` run from cloudspec at the pin exits 0.
2. `ess specify validate --path ess --strict-requires` reports no `duplicate_declaration` and no
   unknown name; what it still reports is the Account/Scope, cluster, runtime-field and policy
   causes that `story:cloud-blocks-account-scope`, `-cluster-pinning`, `-runtime-fields` and
   `-policies` remove.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
