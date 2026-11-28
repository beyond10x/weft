---
format: aep.planning-md/3
id: story:cloudspec-submission-release-steps
kind: story
status: implemented
title: A planned release gets its rollout steps, starts, and starts its first step; r1-set-workload-replicas runs by hand and drops not-a-service
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-plan-apply
scope:
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/add_release_step.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/set_workload_replicas.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/start_release.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/start_release_step.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings/mod.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/bindings/r1_set_workload_replicas.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/release_steps.rs
- confidence: cited
  path: tools/synth-input.sh
- confidence: cited
  path: tools/test-generated.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T14:06:06Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T14:06:06Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T15:24:39Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 3, second half, second step: a planned release gets
its rollout steps and starts them. Depends on `story:cloudspec-submission-plan-apply` (the release
comes from `ApplyPlan` in its test).

## Context

Read on integration/2026-11-28-6 (db91440):

- `AddReleaseStep` (`ess/6-delivery/deployments.yaml:616`), `StartRelease` (line 553),
  `StartReleaseStep` (line 680) and `weft.workloads.SetWorkloadReplicas`
  (`ess/cloud/5-workloads/workloads.yaml:2331`) answer 501: no behaviour file.
- The three release commands are granted to `weft.actors.ReleaseController`
  (`ess/9-actors/actors.yaml:943-981`); "The control plane adds the steps of a planned release"
  (deployments.yaml:614) and "starts a planned release once its steps are added" (line 543).
- `r1-set-workload-replicas` (`ess/components.yaml:1223`): `ReleaseStepStarted` (Rollout, workload
  and replicas defined) -> `SetWorkloadReplicas`, drop `workload-retired`, `not-a-service`. It is
  delivered by the generated System today, and `tools/synth-input.sh` sets every `on_failure` it
  does not remove to `retry` (E2), so a `not-a-service` of a Job workload would be retried, not
  dropped. weft delivers such a binding by hand (`product/cloud/crates/weft-impl/src/bindings/mod.rs:1-15`).
- `StartRelease` has no synthesized scenario (deployments.yaml:544-552); the behaviour follows the
  contract text.

## Outcome

1. Behaviours, each with every outcome of its contract in its order:
   `add_release_step.rs` (`invalid-position`, `rollout-needs-replicas`, `invalid-replicas`,
   `no-such-release`, `release-not-planned`, `position-taken`, `added`), `start_release.rs`
   (`no-such-release`, `already-applied`, `release-in-progress`, `gate-not-passed`, `started`,
   `wrong-state`), `start_release_step.rs` (`no-such-step`, `no-such-release`,
   `release-not-applying`, `rollout-needs-workload`, `previous-step-incomplete`,
   `job-outside-release`, `workload-outside-release`, `started`, emitting `ReleaseStepStarted` with
   the release scope), `set_workload_replicas.rs` (every outcome of line 2331, `not-a-service` for
   a workload of another shape than Service).
2. `r1-set-workload-replicas` delivered by hand: `bindings/r1_set_workload_replicas.rs`, its entry
   in `bindings/mod.rs` (`ENTRIES` and the list of the module comment), its id in
   `removed_bindings` of `tools/synth-input.sh` with the E2 story line. `ess/components.yaml` is not
   changed.

## Domain relations

- Release -> ReleaseStep, owns many: `ess/6-delivery/deployments.yaml`, entity
  `weft.deployments.Release`, relation `steps`.
- ReleaseStep -> Workload, references one, set when a rollout step starts: entity
  `weft.deployments.ReleaseStep`, relation `workload`.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/{add_release_step,start_release,start_release_step,set_workload_replicas}.rs`
(new), `product/cloud/crates/weft-impl/src/bindings/r1_set_workload_replicas.rs` (new),
`product/cloud/crates/weft-impl/src/bindings/mod.rs`, `tools/synth-input.sh`,
`product/cloud/crates/weft-testkit/tests/release_steps.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line).

Shared: `bindings/mod.rs` and `tools/synth-input.sh` with `story:cloudspec-submission-job-run` and
`story:cloudspec-submission-job-exit` (each adds entries; ordered by depends_on).
`weft-testkit/tests/main.rs`, one `mod` line, with every story of this set and with
`story:cloudspec-submission-submit` and `-plan-from-source`.

## Acceptance

On a release made by `ApplyPlan`, a rollout step is added, the release starts and the step starts
naming a Service workload, whose desired replica count then reads the step replicas through an
applied `r1-set-workload-replicas`; for a Job workload that binding line reads `not-a-service`,
`dropped`, and every refusal of the four commands writes no row.

Checked by:

1. AddReleaseStep Rollout, position 1, replicas 2 -> a `Pending` step, `ReleaseStepAdded`;
   StartRelease -> `Applying`, `ReleaseStarted`; StartReleaseStep with the workload -> step
   `Running`, `ReleaseStepStarted` carrying `scope_id`, `workload_id`, `replicas`.
2. Service workload (the skeleton definitions): a binding line `r1-set-workload-replicas` `applied`;
   the service desired replicas 2. Job workload: the line has refusal `not-a-service`, `dropped`, and
   nothing is held.
3. Refusals, each with no new row and no move: position 0, Rollout without replicas, replicas -1,
   unknown release, a release not `Planned`, a taken position (AddReleaseStep); a second release of
   the same plan started, an environment with an `Applying` release, a seeded active gate with no
   passed check (StartRelease); a release not `Applying`, Rollout without workload, step 2 before
   step 1 succeeded, a workload of another environment (StartReleaseStep); each SetWorkloadReplicas
   refusal sent directly.
4. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies under
  `tests/fixtures/cloudspec/`. No Python in anything committed.
