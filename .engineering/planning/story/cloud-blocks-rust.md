---
format: aep.planning-md/3
id: story:cloud-blocks-rust
kind: story
status: active
title: The walking skeleton places and runs an instance on the vendored spec
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
- depends_on: story:cloud-blocks-cluster-pinning
- depends_on: story:cloud-blocks-runtime-fields
scope:
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: product/cloud/crates/weft-agent/tests
- confidence: cited
  path: product/cloud/crates/weft-bin/src
- confidence: cited
  path: product/cloud/crates/weft-client
- confidence: cited
  path: product/cloud/crates/weft-controllers
- confidence: cited
  path: product/cloud/crates/weft-impl
- confidence: cited
  path: product/cloud/crates/weft-testkit
- confidence: cited
  path: quality/ratchet.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:22Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T10:10:15Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the hand-written Rust follows the vendored spec, and the walking
skeleton runs on it.

## Outcome

1. weft-impl's behaviours follow the vendored guards (the blocks cut 9 checks and changed the
   guards of CreateFormation, CreateNodePool, SubmitSchedulingRequest and the instance commands);
   the scope checks the blocks dropped move to weft's run-time admission.
2. weft-controllers, weft-testkit, weft-client, weft-bin and the weft-agent tests use the renamed
   domains and `scope_id` where the spec moved to it (weft-bin's `CallerAttributes` reads
   `caller.scope_id`); the skeleton's path opens a scope. Under D-c
   A the `/weft/` paths stay.
3. `quality/ratchet.yaml` and any held suite counts are re-baselined to the renamed scenario ids,
   with the old and new counts in the commit body.
4. `tools/synth.sh` is timed before and after; a slowdown over 1.2 times is fixed in this story.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/`, `src/context/mod.rs`, `src/quota.rs`,
`src/bindings/r1_submit_scheduling_request.rs`; `weft-controllers/src/placement.rs`,
`tests/main.rs`; `weft-testkit/src/steps.rs`, `tests/main.rs`; `weft-client/`;
`weft-bin/src/node_channel.rs`, `weft-bin/src/lib.rs` (the caller-attribute read); `weft-agent/tests/`; `quality/ratchet.yaml`.

## Acceptance

1. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` passes on the
   vendored spec.
2. `slot --here task test` exits 0 within 120 s.
3. `tools/synth.sh` run cold (its cache under the build directory removed) after the change takes
   at most 1.2 times its cold time before; the report names
   both times.
4. `ratchet::the_ratchet_file_is_consistent` passes with the new counts.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
