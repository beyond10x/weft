---
format: aep.planning-md/3
id: story:skeleton-generated-code
kind: story
status: active
title: Generated domain code from ess/ committed under generated/ with a source digest, checked fast and regenerated in the slow lane
relations:
- decomposes: epic:walking-skeleton
- serves: vision:weft
- depends_on: story:skeleton-workspace-gate
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/weft-testkit/Cargo.toml
- confidence: cited
  path: crates/weft-testkit/tests/main.rs
- confidence: cited
  path: generated
- confidence: cited
  path: tools/check.d/05-generated-selftest.sh
- confidence: cited
  path: tools/check.d/10-generated-digest.sh
- confidence: cited
  path: tools/slow.d/10-generated-regenerate.sh
- confidence: cited
  path: tools/synth-input.sh
- confidence: cited
  path: tools/synth.sh
- confidence: cited
  path: tools/test-generated.sh
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T10:36:58Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T10:36:58Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 2. Depends on `story:skeleton-workspace-gate` (root
workspace with `exclude = ["generated"]`, the `check.d` and `slow.d` hook directories).

ESS gaps E1-E3 (`docs/implementation.md` § Gaps): ESS 0.99.2 refuses the unmodified `ess/` for the
Rust target with 51 refusals of 3 kinds. The host operator decided at 09:28 CET that the skeleton
does not wait for them: this story derives the synthesis input from the unmodified `ess/` with
the transformations listed below (the three of the scratch sample, and the removal of seven more
bindings of the skeleton path that weft delivers by hand), and the hand-written pieces
that restore what a transformation removed land in the stories whose path needs them. `ess/` itself
is never edited for this. Not blocked.

## Context

Decision A2 (`docs/implementation.md`): weft runs its domains as the Rust `ess generate synthesize
--target rust` writes. The output is committed under `generated/` with the digest of its sources; a
fast check compares the digest; the `slow` lane regenerates and compares bytes
(`~/beyond10x/AGENTS.md` § Tests stay fast: an expensive product is made once, never per test).
Measured on the scratch copy: synthesis exits 0 in 15 s, 3019 capabilities (2735 generated, 284
obligations), 7 crates, about 383,000 lines of Rust, with only `r1-create-replica` removed. The
story reports the same counts for its own input with eight bindings removed.

- `tools/synth-input.sh` (POSIX sh, `yq` v4, pinned to v4.30.8 as on this host): copies the
  unmodified `ess/` to `target/synth-input/` and applies exactly these transformations, each
  listed in the script with its ESS story id (ESS store, commit `bfb32ee3d6`):
  - E1: delete every `one_time_response:` line (ESS
    `story:the-rust-target-writes-a-one-time-response-outcome-as-an-obligation-that-carries-its-contract-instead-of-refusing-the-workspace`);
  - removal of the eight bindings of the skeleton path that carry a per-refusal policy, which weft
    delivers by hand (`story:skeleton-replica-instances` owns the runner): `r1-create-replica` (ESS
    `story:rust-binding-wraps-a-plain-copy-into-an-optional-member`), and
    `r1-submit-scheduling-request`, `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`,
    `r4-record-placement-assigned`, `edge-backend-ready`, `edge-backend-restore` (each ESS
    `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`).
    The generated System has no seam for a per-refusal policy (`deliver_*` private, a declared
    refusal held in a private `held_*` list, `run_*` dropping `take_invocations()`), so these
    bindings cannot keep their policies inside it;
  - E2 for every other binding: `explode(.)`, then set every map-valued `on_failure` still present
    to `retry` (ESS `story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts`).
  The scratch sample of 2026-11-27 was made with only `r1-create-replica` dropped; it stays the
  reference for everything else.
- `tools/synth.sh` (POSIX sh): runs `tools/synth-input.sh`, then `ess generate synthesize --path
  target/synth-input --target rust` into a scratch directory under `target/`, and replaces
  `generated/` with it; writes `generated/SOURCES.sha256`, the digest of every file of `ess/`, of
  `tools/synth-input.sh` and of the `ess` version. Synthesis runs through `slot` with a named
  timeout.
- Check hook `tools/check.d/10-generated-digest.sh` (POSIX sh, fast, no `ess` call): recomputes the
  digest and fails when it differs from `generated/SOURCES.sha256`.
- Slow hook `tools/slow.d/10-generated-regenerate.sh`: runs the synthesis into `target/` and
  compares bytes with `generated/`; any difference fails.
- Self-test `tools/test-generated.sh` (POSIX sh) on the fixture specification
  `tools/sim/tests/fixture/` (small, synthesizes in seconds): both sides of the digest check (equal
  digest passes; a changed fixture source fails) and of the regeneration compare (a clean output
  passes; an edited generated file fails). It runs in `check` as hook
  `tools/check.d/05-generated-selftest.sh`.
- Count on the path of `sim/experiments/full-setup.yaml:38-70`, for the stories that restore by
  hand what a transformation removed (each piece with a comment naming its ESS story id; when ESS
  ships a fix, the transformation and its hand-written piece are removed together, in one change):
  - E1: 1 of the 8 one-time outcomes, `weft.infrastructure.ProvisionPoolNode/provisioned` (the join
    token), restored in `story:skeleton-control-host`; `IssueJoinToken` is not on the path.
  - E2: ESS counts 41 refusals; `ess/components.yaml` holds 33 bindings with a map-valued
    `on_failure` (read with `yq`, 2026-11-27; a binding can carry more than one refusal). 8 of
    them run on the skeleton path: 6 that invoke a command of the chain (7 do; the skeleton deploys
    into the default cluster, so `cluster-create-node-pool` is not on it) and the 2 edge bindings
    on `InstanceReady`. Rule (coordinator, 2026-11-27): every path binding with a per-refusal
    policy is hand-run. So the 8 are removed and delivered by hand with their declared policies
    (`story:skeleton-replica-instances`: the runner and the two `r1-*`;
    `story:skeleton-controllers`: the four `r4-*`; `story:skeleton-node-run-assignment`:
    `edge-backend-ready` and `edge-backend-restore`); the other 25 are flattened to `retry`.
  - E3: `r1-create-replica` is on the path, one of the 8 removed and delivered by hand.
- Build cost: the generated crates compile in the root `target/` (the first build of about
  380,000 lines). The story times a clean `cargo build` of `weft-server` through `slot` and reports
  it; a build that does not fit the 10-minute pool job of `slot task check` beside the tests is a
  defect reported with its time.
- CI: the Gate job already installs `ess` 0.99.2; the slow hook needs `yq` too, installed pinned by
  the hook where missing.

## Domain relations

None. The synthesis reads `ess/` as written; this story writes no entity.

## Scope

- `generated/` (the synthesis output, `SOURCES.sha256`)
- `tools/synth-input.sh`, `tools/synth.sh`, `tools/test-generated.sh`
- `tools/check.d/05-generated-selftest.sh`, `tools/check.d/10-generated-digest.sh`
- `tools/slow.d/10-generated-regenerate.sh`
- `crates/weft-testkit/` (`Cargo.toml` path dependency on `generated/crates/weft-server`;
  `tests/main.rs` module `generated`)

## Acceptance

1. `slot task check` fails when a file of `ess/` or `tools/synth-input.sh` changed and
   `generated/SOURCES.sha256` was not regenerated, and passes on the committed tree.
2. `tools/test-generated.sh`, run by `slot task check`, passes only when, on the fixture
   specification, an equal digest passes, a changed source fails the digest check, a clean
   regeneration compares equal, and an edited generated file fails the compare.
3. After `tools/synth-input.sh`, a case of `tools/test-generated.sh` reads the difference between
   `ess/` and `target/synth-input/`, compared as parsed YAML through `yq` (`explode(.)` and the yq
   writer change the layout, so the text is not compared), and finds each transformation at its
   count and nothing else:
   all 8 `one_time_response:` lines of `ess/` deleted (counted 2026-11-27: 4 in
   `2-tenancy/access.yaml`, 3 in `1-infrastructure/infrastructure.yaml`, 1 in
   `5-runtime/identity.yaml`) and none left; exactly eight bindings removed, `r1-create-replica`,
   `r1-submit-scheduling-request`, `r4-bind-placement`, `r4-assign`, `r4-schedule-instance`,
   `r4-record-placement-assigned`, `edge-backend-ready` and `edge-backend-restore`; all 25
   map-valued `on_failure` of the remaining bindings of `ess/components.yaml` replaced by `retry`
   and none left; no other value changed. The counts are read from `ess/` by the case, not written
   into it, so a later change of `ess/` moves them with it.
4. `task slow` regenerates `generated/` from the committed sources into `target/` and finds it
   byte-equal to the committed `generated/`.
5. `slot task check` builds `generated/crates/weft-server` as a path dependency of
   `crates/weft-testkit` and runs its module `generated`, which maps `weft_server::entry::Refused::Unmet`
   to status 501 through the generated `status()`.
## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh (`yq` v4 may be called from sh).
- No load tests, no VMs, no GPU workloads.
- The test step makes no request that leaves the host: the control host, the node agent and
  substrate-daemon run on loopback and a Unix socket; seeded rows or fixtures stand in for
  anything else.
