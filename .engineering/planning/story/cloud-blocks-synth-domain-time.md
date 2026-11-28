---
format: aep.planning-md/3
id: story:cloud-blocks-synth-domain-time
kind: story
status: active
title: tools/synth-domain.sh builds a valid subset of the vendored spec and synthesizes it within 10 minutes
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
scope:
- confidence: cited
  path: tools/synth-domain.sh
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T10:10:15Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T10:10:15Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: a test-budget defect found by `story:cloud-blocks-policies`.

## Found

- After cloudspec's blocks were vendored (`ess/cloud/`, a485b7f), `tools/synth-domain.sh accounts`
  ran into the 30-minute pool limit (exit 124 at 1801 s) on a subset of 12 domains and on one of 24;
  every subset that holds `weft.workloads` does. Before, AGENTS.md gives about 80 s per domain
  and 451 s for the whole system (not measured again).
- The script as committed builds a subset that does not validate (exit 1, 18 s): it leaves out
  `weft.cluster`, `weft.nodes` and `weft.pipelines`, which weft's run-time guards read, and the
  StampedAt conversions of `components.yaml`. Two patched versions are in the session scratch
  (`policies/synth-domain.patch`, `synth-domain-narrow.patch`).
- Code synthesis is unaffected: `tools/synth.sh` takes 11 s on the vendored tree and 10 s on
  main 5f034cc. No gate step runs conformance synthesis; only `tools/synth-domain.sh` does.

## Outcome

1. `tools/synth-domain.sh <name>` builds a subset that validates for every weft domain.
2. Its synthesis of the `accounts`, `scheduling`, `access` and `infrastructure` subsets ends within
   10 minutes each, or the cause is named (the domain, command or guard whose scenarios grow) and
   handed to the owner (cloudspec for a vendored block, ess for the synthesizer).
3. The scenario checks of `story:cloud-blocks-policies` (its former acceptance 4) run.

## Scope

`tools/synth-domain.sh`.

## Acceptance

1. One at a time, outside the test step, `slot --timeout 30m tools/synth-domain.sh <name>` exits 0
   within 10 minutes and reports 0 failed scenarios for `accounts`, `scheduling`, `access` and
   `infrastructure`; the `scheduling` suite holds a scenario in which `PlaceInstance` on a node
   outside the formation's cluster ends in that refusal; the `access` suite a passing
   `ConfigureAccess` scenario that sets `access_mode`; the `infrastructure` suite a passing
   `ScaleControlPlane` scenario that sets `control_plane`.
2. The report names the synthesis time of each of the four runs.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
