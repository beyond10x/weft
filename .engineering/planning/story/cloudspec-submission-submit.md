---
format: aep.planning-md/3
id: story:cloudspec-submission-submit
kind: story
status: active
title: The control host takes a cloudspec-weft-submission/1 file, checks it with weft-cloudspec and records it through SubmitDeploymentSource with kind Cloudspec
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-reader
- depends_on: story:cloudspec-submission-pin
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: product/cloud/crates/weft-bin/Cargo.toml
- confidence: inferred
  path: product/cloud/crates/weft-bin/src/cloudspec.rs
- confidence: cited
  path: product/cloud/crates/weft-bin/src/main.rs
- confidence: cited
  path: product/cloud/crates/weft-client/src/lib.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours/submit_deployment_source.rs
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/documents.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/lib.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/storage/mod.rs
- confidence: inferred
  path: product/cloud/crates/weft-testkit/tests/cloudspec_submit.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/tests/main.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T13:44:17Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T13:44:17Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 2 beyond the offline checks: a checked
`cloudspec-weft-submission/1` is recorded through `weft.deployments.SubmitDeploymentSource` with kind
`Cloudspec`, no translator and both member digests. Takes outcome 1 of
`story:cloudspec-submission-plan` (submit), which drafts outcomes 2 and 3 as one story and is split
here; that story is not edited. Depends on `story:cloudspec-submission-reader` (the reader) and
`story:cloudspec-submission-pin` (the command at the pin).

## Context

Read on integration/2026-11-28-6 (fee8313, pin f57337466c7c):

- `SubmitDeploymentSource` (`ess/cloud/11-translation/deployments.yaml:827-970`) carries digests,
  not the document. It answers 501 today: no file of `product/cloud/crates/weft-impl/src/behaviours/`
  names it, so `weft-impl/build.rs` writes the `UnmetObligation` stub.
- `PlanFromSource` (line 1413) later copies the submitted architecture (comment lines 1402-1405), so
  the control host keeps the file; the command cannot carry it.
- The control host sends `/node/` to `node_channel` and every other path to `Host::handle`
  (`product/cloud/crates/weft-bin/src/main.rs:143`); `Host::unit` runs several commands in one
  transaction (`weft-bin/src/lib.rs:165`).

## Outcome

1. `product/cloud/crates/weft-impl/src/behaviours/submit_deployment_source.rs`: every outcome of the
   contract, in its order, for every kind (`not-callers-scope`, `values-not-helm`,
   `not-a-spec-source`, `translator-required`, `translator-not-allowed`, `digests-required`,
   `digests-not-cloudspec`, `formation-missing`, `no-translator`, `formation-foreign`,
   `translator-not-for-source`, `values-environment-foreign`, `submitted`). `submitted` writes the
   `DeploymentSource` (state `Submitted`, both digests, `submitted_by` the caller) and emits
   `SourceSubmitted`.
2. `product/cloud/crates/weft-impl/src/documents.rs` (new, `pub mod documents` in `lib.rs`): one
   table of the same state file, the submission bytes keyed by `source_digest`; `put` and `get`.
3. Route `POST /cloudspec/submissions?formation_id=<id>` (module
   `product/cloud/crates/weft-bin/src/cloudspec.rs`, one branch in `main.rs` beside `node_channel`),
   the body the file bytes, the caller as for every request:
   - `weft_cloudspec::Submission::read` refuses: 422 with `{"refusal": "<variant>"}`, a request
     line in the log, nothing written;
   - accepted: in one `Host::unit`, the bytes are stored by their sha256 and
     `SubmitDeploymentSource` runs with kind `Cloudspec`, `format_version`
     `cloudspec-weft-submission/1`, `source_digest` `sha256:<hex of the file>`, the two digests of
     the file, no translator, no values, no `embedded_in`. A refusal of the command stores no bytes.
     The route answers what the command answers.
4. `weft-client`: `Client::post_bytes(path, bytes)`, used by the tests.

## Domain relations

- DeploymentSource → Scope, references one: `ess/cloud/11-translation/deployments.yaml`, entity
  `weft.deployments.DeploymentSource`, relation `scope`.
- DeploymentSource → Formation, references one, a foreign formation answers `NotFound`: same entity,
  relation `formation`.
- DeploymentSource → stored submission file, one-to-one by `source_digest`, kept as long as the
  state file (no command deletes a source; a withdrawn one keeps its records): inferable (inferred
  from `ess/cloud/11-translation/deployments.yaml:1402-1405`, the comment of `PlanFromSource`, and
  type `weft.deployments.SourceDigest` line 45; no ess/1 document declares where the bytes live).

## Scope

`product/cloud/crates/weft-impl/src/behaviours/submit_deployment_source.rs` (new),
`product/cloud/crates/weft-impl/src/documents.rs` (new), `product/cloud/crates/weft-impl/src/lib.rs`
(one `pub mod` line), `product/cloud/crates/weft-bin/src/cloudspec.rs` (new),
`product/cloud/crates/weft-bin/src/main.rs` (one branch), `product/cloud/crates/weft-bin/Cargo.toml`,
`Cargo.lock`, `product/cloud/crates/weft-client/src/lib.rs` (`post_bytes`),
`product/cloud/crates/weft-testkit/tests/cloudspec_submit.rs` (new),
`product/cloud/crates/weft-testkit/tests/main.rs` (one `mod` line).

Shared: `story:cloudspec-submission-plan-from-source` reads `documents.rs` and adds its own `mod`
line to `weft-testkit/tests/main.rs`. `story:cloudspec-submission-docs` shares no file; its
`:::caution[Planned]` section names no command, so this route does not make it false.
`story:cloudspec-submission-plan` (scope `weft-impl/src/behaviours`, `weft-testkit`) holds every
path above under its own; a wave runs this split or that story, not both.

## Acceptance

A control-host test posts `tests/fixtures/cloudspec/snippet.submission.json` and reads back one
`DeploymentSource` of kind `Cloudspec` with the three digests of the golden, and changed copies are
answered 422 with their `weft-cloudspec` refusal name and leave no row.

Checked by:

1. snippet: the row has `source_digest`
   `sha256:d58f6b6cf43d2f0bb672a11c68c7bb51c0b848dbddf2b9ff999ee08567dfaead`, `architecture_digest`
   `sha256:3ce513003eec18301050ec62932fdfdc45a7b4193d5e3b43c82a5a1cfa5c948f`, `binding_digest`
   `sha256:74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b`, state `Submitted`; the
   log holds a `SourceSubmitted` event line; `documents::get` answers the posted bytes.
2. `format` `cloudspec-weft-submission/2` → 422 `UnknownFormat`; one string of `architecture`
   changed → 422 `DigestMismatch`; neither leaves a `DeploymentSource` or a stored document.
3. The command, sent directly: `Cloudspec` with a `translator_id` → `TranslatorNotAllowed`;
   `Cloudspec` without `binding_digest` → `DigestsRequired`; `Compose` with an
   `architecture_digest` → `DigestsOnlyForCloudspec`; `Compose` without a translator →
   `TranslatorRequired`; a formation of another scope → `NotFound`; none writes a row.
4. A post naming a formation of another scope answers `NotFound` and stores no bytes.
5. `place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running` still passes.

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary per
  crate. No test over 60 s (the runner kills it); the test step ends within 120 s; a test over 5 s
  is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m).
- No network and no cloudspec binary in the test step: the goldens are the committed copies. No
  Python in anything committed.
