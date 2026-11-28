---
format: aep.planning-md/3
id: story:cloudspec-submission-plan
kind: story
status: archived
title: SubmitDeploymentSource accepts a cloudspec submission, PlanFromSource plans its snippet workloads, and the snippet golden runs to a finished job
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-reader
- depends_on: story:cloudspec-submission-pin
scope:
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
- confidence: inferred
  path: product/cloud/crates/weft-testkit
revision: 4
transitions:
- {from: "draft", to: "archived", at: "2026-11-28T13:44:04Z", actor: "human:operator", revision: 4}
---
## Epic

Decomposes `epic:cloudspec-submission`, outcomes 2 (submit) and 3 (plan and run), which no story
covered: `story:cloudspec-submission-reader` checks a submission offline, `story:cloudspec-submission-pin`
brings `SourceKind::Cloudspec` and `weft.deployments.PlanFromSource` into `ess/cloud/`, and no
behaviour of weft implements either command (`product/cloud/crates/weft-impl/src/behaviours/` on
integration/2026-11-28-6, fee8313). Drafted for `epic:slot-node-daemon`: every slot-node story
reaches weft through this path. Depends on `story:cloudspec-submission-reader` and
`story:cloudspec-submission-pin`.

## Outcome

1. `weft.deployments.SubmitDeploymentSource` with kind `Cloudspec` and no translator: the behaviour
   runs `Submission::read` of `weft-cloudspec` on the bytes and answers each of its five refusals
   by name with nothing changed; an accepted one stores the source with its file digest and the
   two member digests (`ess/cloud/11-translation/deployments.yaml`, command at line 827).
2. `weft.deployments.PlanFromSource {source_id, scope_id, environment_id}` (line 1413) in its
   contract order: weft builds the formation revision from the snippet parts of the architecture
   (one component and one `Snippet` workload per part, `command` and `environment` copied) and the
   plan names the source and no translation (D-R85-1 = B, `Plan.source_id`, line 524).
3. The walking skeleton follows the plan to an instance on the local node; the `snippet` golden
   runs to a finished job.
4. A part the coverage marks refused is not planned: the plan holds only what cloudspec mapped.

## Domain relations

- `weft.deployments.Plan` → `weft.deployments.DeploymentSource`, many-to-one, a Cloudspec plan
  names its source and no translation: inferable from `ess/cloud/11-translation/deployments.yaml`,
  entity `weft.deployments.Plan`, field `source_id` (comment lines 524-527), and the cleared
  `decision-blocker:cloudspec-source-plan-path`.
- snippet part → `weft.workloads.Workload` (package kind `Snippet`), one-to-one, the architecture
  owns the part: inferable (inferred from cloudspec
  `59129eb:crates/cloudspec/src/generate/weft/runs.rs`, row `SNIPPET`, "cloud.workloads.Workload
  package_kind Snippet"; no ess/1 document declares this relation).

## Scope

Worked out, not read in a diff: `product/cloud/crates/weft-impl/src/behaviours/` (two new
behaviours and their registration), `product/cloud/crates/weft-cloudspec/src/` (snippet parts to
formation inputs), `product/cloud/crates/weft-testkit/`.

## Acceptance

A test of the control host submits `tests/fixtures/cloudspec/snippet.submission.json`, calls
`PlanFromSource`, and sees the snippet instance of the plan finish its job on the local node, with
no network and no cloudspec binary.

Checked by: a changed golden is refused at submit with its `weft-cloudspec` refusal name and no
`DeploymentSource` row; `PlanFromSource` on a source of another kind answers
`SourceNotCloudspec`; the plan row has `source_id` set and `translation_id` absent.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- No network, no cloudspec binary in the test step. No Python in anything committed.
