---
format: aep.planning-md/3
id: story:cloudspec-submission-reader
kind: story
status: draft
title: 'weft reads a cloudspec-weft-submission/1 offline: format, both member digests and the binding, checked on cloudspec''s three goldens'
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: product/cloud/crates/weft-cloudspec
- confidence: cited
  path: tests/fixtures/cloudspec
- confidence: cited
  path: tools/slow.d/30-cloudspec-goldens.sh
revision: 3
---
## Epic

Decomposes `epic:cloudspec-submission`: outcome 4 (the three goldens in weft's fixtures with their
digests, checked in the test step with no network and no cloudspec binary) and the offline checks
of outcome 2 (format, both member digests, the binding). Submitting the checked document through
`weft.deployments.SubmitDeploymentSource` waits for `decision-blocker:cloudspec-source-translator`
and is not this story. The library depends on no generated crate, so it builds at either pin.

## Context

- Format `cloudspec-weft-submission/1`: `{format, architecture, binding (null when none), coverage,
  digests{architecture, binding}}`; each digest is `sha256:<hex>` of the canonical JSON of its member,
  `null` included (cloudspec `fdcf7e8:docs/layers.md` § 11 Weft submission).
- Canonical JSON: object keys sorted by their UTF-8 bytes, no white space, strings and numbers as
  serde_json writes them (`fdcf7e8:crates/cloudspec/src/generate/weft.rs:32-52`). The keys are sorted
  explicitly, as cloudspec does, so a `preserve_order` feature unified in by another crate of the
  workspace does not change a digest.
- Goldens: `fdcf7e8:fixtures/weft/{snippet,workstation,host-daemon}.submission.json` (file sha256
  `d58f6b6c…`, `ee10ee24…`, `25303268…`). snippet: binding null, 4 mapped, 0 refused. workstation:
  binding `split` for architecture `workstation`, role `agent-runtime` named by parts `operator` and
  `worker`. host-daemon: binding null, 28 refused in the weft step. All three pass the checks below.

## Outcome

1. A crate `product/cloud/crates/weft-cloudspec` (library; dependencies serde_json and sha2, no
   generated crate): `Submission::read(bytes)` answers the accepted submission or one named refusal
   (`enum Refusal`), checked in this order, stopping at the first:
   - `NotASubmission`: not a JSON object with the five members;
   - `UnknownFormat { found }`: `format` other than `cloudspec-weft-submission/1` (another version
     included);
   - `DigestMismatch { member, declared, computed }`: the recomputed digest of `architecture`, then
     of `binding`, differs from `digests`;
   - `BindingForOtherArchitecture { binding, architecture }`: a binding whose `architecture` is not
     the architecture's `architecture`;
   - `BindingRoleNamesNoPart { role }`: a key of `binding.roles` that no part names as
     `package.runtime.role`.
2. The accepted `Submission` answers the architecture, the binding and the coverage as JSON values,
   the two digests, and the sha256 of the file's bytes (the source digest a later
   `SubmitDeploymentSource` names, cloudspec `fdcf7e8:docs/layers.md:634`).
3. `tests/fixtures/cloudspec/`: byte copies of the three goldens and `goldens.lock` (one line
   `commit fdcf7e8286a79bc3bbcdb54bf9fe1e332d17884c`, one line `<sha256> <file>` per golden).
4. `tools/slow.d/30-cloudspec-goldens.sh` (POSIX sh, slow lane only): fetches cloudspec at the lock's
   commit into `target/cloudspec-src` with git and compares each golden's bytes; it builds nothing.

## Domain relations

- Binding → Architecture: many-to-one by name (`binding.architecture` = `architecture.architecture`);
  the architecture does not own the binding and either changes without the other. Inferable
  (inferred from cloudspec `fdcf7e8:crates/cloudspec/src/generate/weft.rs:300-307`, the refusal "the
  binding is for architecture {want}, the input is {got}"; no ess/1 document declares this
  relation).
- Binding role → Part: one role of `binding.roles` is named by one or more parts
  (`package.runtime.role`); a role no part names is refused. Inferable (inferred from cloudspec
  `fdcf7e8:crates/cloudspec/src/main.rs:465-469`, the roles the generator binds are those the parts
  name; no ess/1 document declares this relation).
- DeploymentSource → submission file: one-to-one by `source_digest`, the content digest of the
  source's bytes. Inferable from `ess/cloud/11-translation/deployments.yaml:41-46`
  (`weft.deployments.SourceDigest`); this story only computes it.

## Scope

`product/cloud/crates/weft-cloudspec/` (new), `Cargo.lock`, `tests/fixtures/cloudspec/` (new),
`tools/slow.d/30-cloudspec-goldens.sh` (new). `story:cloudspec-submission-pin` edits
`tests/fixtures/cloudspec/` after this story.

## Acceptance

`cargo nextest run -p weft-cloudspec` (through the slot shim) passes with no network and no
cloudspec binary: the three goldens match `goldens.lock` and are accepted, and one-place changes
of them are refused with each of the five refusal names.

Checked by:

1. Each golden's sha256 equals its lock line; a test that changes one byte of a copy in
   `target/` sees the mismatch.
2. snippet is accepted with recomputed digests
   `sha256:3ce513003eec18301050ec62932fdfdc45a7b4193d5e3b43c82a5a1cfa5c948f` and
   `sha256:74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b`; workstation and
   host-daemon are accepted.
3. Copies changed in memory are refused: `[]` → `NotASubmission`; `format`
   `cloudspec-weft-submission/2` → `UnknownFormat`; one string of `architecture` changed →
   `DigestMismatch` (architecture); workstation's `binding.binding` changed → `DigestMismatch`
   (binding); workstation's `binding.architecture` renamed with its digest recomputed →
   `BindingForOtherArchitecture`; a role `nobody` added with its digest recomputed →
   `BindingRoleNamesNoPart`.
4. `tools/slow.d/30-cloudspec-goldens.sh` exits 0 at the lock's commit, and 1 after a byte of a
   golden changes (slow lane; it fetches).

## Rules for this work

- Builds and tests run through slot (`cargo` through its shim). One integration-test binary
  (`tests/main.rs`). No test over 60 s (the runner kills it); the test step ends within 120 s; a
  test over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time
  with `slot --timeout` (at most 30m).
- No network, no process spawn and no cloudspec binary in the test step; the goldens are committed
  with their digests. The slow-lane hook fetches and runs without the `slot` prefix (it builds
  nothing).
- No Python in anything committed: Rust and POSIX sh.
