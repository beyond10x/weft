---
format: aep.planning-md/3
id: story:cloud-blocks-rust
kind: story
status: draft
title: The walking skeleton places and runs an instance on the vendored spec
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
- depends_on: story:cloud-blocks-cluster-pinning
- depends_on: story:cloud-blocks-runtime-fields
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: the hand-written Rust follows the vendored spec, and the walking
skeleton runs on it.

## Outcome

1. weft-impl's behaviours follow the vendored guards (the blocks cut 9 checks and changed the
   guards of CreateFormation, CreateNodePool, SubmitSchedulingRequest and the instance commands);
   the scope checks the blocks dropped move to weft's run-time admission.
2. weft-controllers, weft-testkit, weft-client and weft-bin use the renamed domains and a scope
   on the skeleton's path.
3. `tools/synth.sh` is timed before and after; a jump is fixed in this story.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/`, `src/context/mod.rs`, `src/quota.rs`,
`src/bindings/r1_submit_scheduling_request.rs`; `weft-controllers/src/placement.rs`,
`tests/main.rs`; `weft-testkit/src/steps.rs`, `tests/main.rs`; `weft-client/`;
`weft-bin/src/node_channel.rs`.

## Acceptance

1. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` passes on the
   vendored spec.
2. `slot --here task test` ends within 120 s; the report names both `tools/synth.sh` times.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
