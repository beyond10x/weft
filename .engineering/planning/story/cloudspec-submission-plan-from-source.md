---
format: aep.planning-md/3
id: story:cloudspec-submission-plan-from-source
kind: story
status: active
title: PlanFromSource copies a submitted cloudspec architecture into a formation revision and a plan naming the source, each refusal tested
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-submit
- depends_on: story:cloudspec-submission-pin
scope:
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/plan_from_source.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/plan_from_source.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T14:03:29Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T14:03:29Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloudspec-submission`, the first half of outcome 3: an accepted submission becomes a
plan through `weft.deployments.PlanFromSource`. Takes outcome 2 of `story:cloudspec-submission-plan`
without its workloads per part (behind `decision-blocker:cloudspec-snippet-workload-definition`);
that story is not edited. Depends on `story:cloudspec-submission-submit` (the source row and the
stored file) and `story:cloudspec-submission-pin` (the command at the pin).

## Context

Read on integration/2026-11-28-6 (fee8313, pin f57337466c7c):

- `PlanFromSource {source_id, scope_id, environment_id}`
  (`ess/cloud/11-translation/deployments.yaml:1413-1480`) is granted and accepted by `weft-control`
  and answers 501: no behaviour file. Its `planned` outcome creates the `Plan`; the
  `FormationRevision` is minted by the implementation (UNMAPPED, lines 1406-1411), as
  `CommitRevision` mints its number and document digest
  (`product/cloud/crates/weft-impl/src/behaviours/commit_revision.rs`).
- A plan of a cloudspec source names the source and no translation (`Plan.source_id`, lines
  520-524; D-R85-1 = B).

## Outcome

1. `product/cloud/crates/weft-impl/src/behaviours/plan_from_source.rs`: the outcomes in the order of
   the contract: `not-callers-scope`, `no-such-source`, `no-such-environment`,
   `source-of-other-scope`, `source-not-cloudspec`, `source-withdrawn`, `environment-foreign`,
   `planned`.
2. `planned`: reads the stored file by the source digest (`weft_impl::documents::get`) and checks it
   with `weft_cloudspec::Submission::read`; its recomputed architecture digest equals the source row
   `architecture_digest`. Then, in the one command, a `FormationRevision` of the source formation
   (state `Recorded`, `number` one more than the revisions of the formation so far,
   `document_digest` the architecture digest) and a `Plan` (state `Simulating`, `scope_id` and
   `formation_id` of the source, `environment_id` of the input, `formation_revision_id` the new
   revision, `source_id` set, `translation_id` absent, `created_by` the caller), and `PlanCreated`
   with the revision id. A missing or different stored file answers the `UnmetObligation` (501,
   `committed: false`) and writes nothing.

## Domain relations

- Plan → DeploymentSource, many-to-one, references, no translation: `ess/cloud/11-translation/deployments.yaml`,
  entity `weft.deployments.Plan`, relation `source`.
- Plan → FormationRevision, references one: entity `weft.deployments.Plan`, relation `formation_revision`.
- Plan → Environment, references one, the environment one of the source formation:
  entity `weft.deployments.Plan`, relation `environment`, with refusal `environment-foreign` of
  `PlanFromSource`.
- Formation → FormationRevision, owns many: `ess/cloud/9-composition/architecture.yaml`, entity
  `weft.architecture.Formation`, relation `revisions`.
- FormationRevision → document, references by `document_digest`, the document outlives the revision:
  `ess/cloud/9-composition/architecture.yaml`, entity `weft.architecture.FormationRevision`, relation
  `document`; the digest is the source `architecture_digest` per the comment of `PlanFromSource`
  (`deployments.yaml:1402-1405`).

## Scope

`product/cloud/crates/weft-impl/src/behaviours/plan_from_source.rs` (new),
`product/cloud/crates/weft-testkit/tests/plan_from_source.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line). Reads, does not change,
`product/cloud/crates/weft-impl/src/documents.rs` of `story:cloudspec-submission-submit`.

Shared: `weft-testkit/tests/main.rs` with `story:cloudspec-submission-submit` (one `mod` line each);
`story:cloudspec-submission-plan` holds both paths under its own scope. No file of
`story:cloudspec-submission-docs`.

## Acceptance

With the snippet golden submitted to a formation that has an environment, `PlanFromSource` answers
`planned`, and each of its seven refusals is answered with its error and writes no `Plan` and no
`FormationRevision`.

Checked by:

1. planned: the `Plan` row has `source_id` set, `translation_id` absent, state `Simulating`; its
   revision has `document_digest`
   `sha256:3ce513003eec18301050ec62932fdfdc45a7b4193d5e3b43c82a5a1cfa5c948f` and the next `number`
   of its formation; the log holds a `PlanCreated` event line naming that revision.
2. Refusals, each with no new row: another `scope_id` → `NotFound`; an unknown source → `NotFound`;
   an unknown environment → `NotFound`; a source of another scope → `NotFound`; a seeded `Compose`
   source → `SourceNotCloudspec`; a seeded `Withdrawn` cloudspec source → `SourceIsWithdrawn`; an
   environment of another formation → `NotFound`.
3. A seeded cloudspec source with no stored file answers 501 and writes nothing.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step. No Python in anything committed.
